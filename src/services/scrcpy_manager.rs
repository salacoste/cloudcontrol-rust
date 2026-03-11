use crate::device::adb::Adb;
use crate::device::scrcpy::ScrcpySession;
use bytes::Bytes;
use dashmap::DashMap;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{broadcast, Mutex};

const SCRCPY_SERVER_LOCAL: &str = "resources/scrcpy/scrcpy-server.jar";
const SCRCPY_SERVER_REMOTE: &str = "/data/local/tmp/scrcpy-server.jar";

/// Broadcast channel capacity: ~833ms buffer at 60fps.
const BROADCAST_CHANNEL_CAPACITY: usize = 50;

/// Directory for scrcpy session recordings.
const RECORDINGS_DIR: &str = "recordings";

/// H.264 Annex B start code prefix.
const NAL_START_CODE: &[u8] = &[0x00, 0x00, 0x00, 0x01];

/// Pre-serialized video frame for zero-copy broadcasting to multiple viewers.
/// Format: flags(1) + size(4 BE) + H.264 NAL data.
#[derive(Debug, Clone)]
pub struct BroadcastFrame {
    pub data: Bytes,
}

impl BroadcastFrame {
    /// Create a BroadcastFrame from a ScrcpyFrame, serializing to the WS binary format.
    pub fn from_scrcpy_frame(frame: &crate::device::scrcpy::ScrcpyFrame) -> Self {
        let flags: u8 =
            (if frame.is_config { 1 } else { 0 }) | (if frame.is_key { 2 } else { 0 });
        let size = (frame.data.len() as u32).to_be_bytes();

        let mut buf = Vec::with_capacity(5 + frame.data.len());
        buf.push(flags);
        buf.extend_from_slice(&size);
        buf.extend_from_slice(&frame.data);

        Self {
            data: Bytes::from(buf),
        }
    }
}

/// Serializable session info returned by REST endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct ScrcpySessionInfo {
    pub session_id: String,
    pub udid: String,
    pub serial: String,
    pub status: String,
    pub width: u32,
    pub height: u32,
    pub device_name: String,
    pub started_at: String,
}

/// Metadata for a scrcpy session recording.
#[derive(Debug, Clone, Serialize)]
pub struct ScrcpyRecordingInfo {
    pub id: String,
    pub udid: String,
    pub file_path: String,
    pub started_at: String,
    pub stopped_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub file_size: Option<u64>,
    pub frame_count: u64,
    pub status: String,
}

/// Internal entry holding session info + the live scrcpy session handle.
pub struct ScrcpySessionEntry {
    pub info: ScrcpySessionInfo,
    pub session: Arc<Mutex<ScrcpySession>>,
    /// Broadcast sender for video frames — viewers subscribe via `sender.subscribe()`.
    pub video_tx: broadcast::Sender<BroadcastFrame>,
    /// Handle to the video producer task (reads frames → broadcasts).
    pub producer_task: Option<tokio::task::JoinHandle<()>>,
    /// Active recording task handle (if recording).
    pub recording_task: Option<tokio::task::JoinHandle<()>>,
    /// Active recording ID (if recording).
    pub recording_id: Option<String>,
    /// Oneshot sender to signal graceful recording stop (allows BufWriter flush).
    pub recording_stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Shared atomic frame counter for the active recording.
    pub frame_counter: Option<Arc<AtomicU64>>,
}

/// Manages scrcpy JAR push tracking and active session lifecycle.
#[derive(Clone)]
pub struct ScrcpyManager {
    /// serial → true if JAR has been pushed in this session
    pushed: Arc<DashMap<String, bool>>,
    /// udid → active session entry (one session per device)
    sessions: Arc<DashMap<String, ScrcpySessionEntry>>,
    /// udid → true while a session is being started (prevents TOCTOU race)
    starting: Arc<DashMap<String, bool>>,
    /// recording_id → recording metadata
    recordings: Arc<DashMap<String, ScrcpyRecordingInfo>>,
}

impl ScrcpyManager {
    pub fn new() -> Self {
        // Ensure recordings directory exists
        let _ = std::fs::create_dir_all(RECORDINGS_DIR);

        Self {
            pushed: Arc::new(DashMap::new()),
            sessions: Arc::new(DashMap::new()),
            starting: Arc::new(DashMap::new()),
            recordings: Arc::new(DashMap::new()),
        }
    }

    /// Start a scrcpy session for the given device.
    /// Returns session info on success, error string on failure.
    /// Uses a `starting` sentinel to prevent concurrent starts for the same device.
    pub async fn start_session(
        &self,
        udid: &str,
        serial: &str,
    ) -> Result<ScrcpySessionInfo, String> {
        // Atomic check: reject if already active or already starting
        if self.sessions.contains_key(udid) || self.starting.contains_key(udid) {
            return Err("ERR_SESSION_ALREADY_ACTIVE".to_string());
        }

        // Insert sentinel to block concurrent starts (removed on success or failure)
        self.starting.insert(udid.to_string(), true);

        tracing::info!(
            "[ScrcpyManager] Starting session for {} (serial: {})",
            udid,
            serial
        );

        let scrcpy = match ScrcpySession::start(serial).await {
            Ok(s) => s,
            Err(e) => {
                self.starting.remove(udid);
                return Err(e);
            }
        };

        let session_id = uuid::Uuid::new_v4().to_string();
        let info = ScrcpySessionInfo {
            session_id,
            udid: udid.to_string(),
            serial: serial.to_string(),
            status: "active".to_string(),
            width: scrcpy.meta.width,
            height: scrcpy.meta.height,
            device_name: scrcpy.meta.device_name.clone(),
            started_at: chrono::Utc::now().to_rfc3339(),
        };

        // Create broadcast channel for video frame distribution
        let (video_tx, _) = broadcast::channel::<BroadcastFrame>(BROADCAST_CHANNEL_CAPACITY);

        let session_handle = Arc::new(Mutex::new(scrcpy));

        // Spawn video producer task: reads frames from scrcpy → broadcasts to all viewers
        let producer_session = session_handle.clone();
        let producer_tx = video_tx.clone();
        let producer_udid = udid.to_string();
        let producer_task = tokio::spawn(async move {
            loop {
                let frame = {
                    let mut s = producer_session.lock().await;
                    s.read_frame().await
                };

                match frame {
                    Ok(frame) => {
                        let broadcast_frame = BroadcastFrame::from_scrcpy_frame(&frame);
                        // Send to all subscribers; ignore error if no receivers
                        let _ = producer_tx.send(broadcast_frame);
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[ScrcpyManager] Video producer error for {}: {}",
                            producer_udid,
                            e
                        );
                        break;
                    }
                }
            }
            tracing::info!(
                "[ScrcpyManager] Video producer stopped for {}",
                producer_udid
            );
        });

        let entry = ScrcpySessionEntry {
            info: info.clone(),
            session: session_handle,
            video_tx,
            producer_task: Some(producer_task),
            recording_task: None,
            recording_id: None,
            recording_stop_tx: None,
            frame_counter: None,
        };

        self.sessions.insert(udid.to_string(), entry);
        self.starting.remove(udid);
        tracing::info!(
            "[ScrcpyManager] Session started for {} ({}x{})",
            udid,
            info.width,
            info.height
        );

        Ok(info)
    }

    /// Stop and clean up the scrcpy session for the given device.
    /// Cleanup is best-effort — errors are logged but don't prevent session removal.
    pub async fn stop_session(&self, udid: &str) -> Result<(), String> {
        let entry = self
            .sessions
            .remove(udid)
            .ok_or_else(|| "ERR_SESSION_NOT_FOUND".to_string())?;

        let (_key, mut entry) = entry;
        tracing::info!("[ScrcpyManager] Stopping session for {}", udid);

        // Auto-stop any active recording before cleanup.
        // Signal graceful stop first, then abort as fallback.
        if let Some(recording_id) = entry.recording_id.take() {
            // Signal graceful shutdown so BufWriter can flush
            if let Some(tx) = entry.recording_stop_tx.take() {
                let _ = tx.send(());
            }
            // Give the task a moment to flush, then abort as fallback
            if let Some(task) = entry.recording_task.take() {
                if tokio::time::timeout(std::time::Duration::from_secs(2), task)
                    .await
                    .is_err()
                {
                    tracing::warn!(
                        "[ScrcpyManager] Recording task for {} did not finish in time, aborting",
                        udid
                    );
                }
            }
            // Read frame count from atomic counter
            let final_frame_count = entry
                .frame_counter
                .as_ref()
                .map(|c| c.load(Ordering::Relaxed))
                .unwrap_or(0);
            // Update recording metadata to mark as completed
            if let Some(mut rec) = self.recordings.get_mut(&recording_id) {
                if rec.status == "recording" {
                    let now = chrono::Utc::now();
                    rec.status = "completed".to_string();
                    rec.stopped_at = Some(now.to_rfc3339());
                    rec.frame_count = final_frame_count;
                    if let Some(started) =
                        chrono::DateTime::parse_from_rfc3339(&rec.started_at).ok()
                    {
                        rec.duration_ms = Some(
                            (now - started.with_timezone(&chrono::Utc))
                                .num_milliseconds() as u64,
                        );
                    } else {
                        tracing::warn!(
                            "[ScrcpyManager] Failed to parse started_at for recording {}: '{}'",
                            recording_id,
                            rec.started_at
                        );
                    }
                    // Best-effort file size update
                    if let Ok(meta) = std::fs::metadata(&rec.file_path) {
                        rec.file_size = Some(meta.len());
                    }
                }
            }
            tracing::info!(
                "[ScrcpyManager] Auto-stopped recording {} for {}",
                recording_id,
                udid
            );
        }

        // Abort producer task — this stops frame reading and drops the sender,
        // which causes all broadcast receivers to get RecvError::Closed.
        if let Some(task) = entry.producer_task {
            task.abort();
        }
        // Sender is dropped here (entry moved out of DashMap), closing all receivers.

        // Best-effort cleanup: session is already removed from map, so even if
        // cleanup partially fails, we won't leak the session entry.
        let mut s = entry.session.lock().await;
        if let Err(e) = s.video_stream.shutdown().await {
            tracing::warn!("[ScrcpyManager] Video stream shutdown error for {}: {}", udid, e);
        }
        if let Err(e) = s.control_stream.shutdown().await {
            tracing::warn!("[ScrcpyManager] Control stream shutdown error for {}: {}", udid, e);
        }
        if let Some(mut proc) = s.server_process.take() {
            if let Err(e) = proc.kill().await {
                tracing::warn!("[ScrcpyManager] Process kill error for {}: {}", udid, e);
            }
        }
        if let Err(e) = Adb::forward_remove(&s.serial, s.local_port).await {
            tracing::warn!("[ScrcpyManager] Forward remove error for {}: {}", udid, e);
        }

        tracing::info!("[ScrcpyManager] Session stopped for {}", udid);
        Ok(())
    }

    /// List all active sessions.
    pub fn list_sessions(&self) -> Vec<ScrcpySessionInfo> {
        self.sessions
            .iter()
            .map(|entry| entry.value().info.clone())
            .collect()
    }

    /// Get info for a specific session by udid.
    pub fn get_session(&self, udid: &str) -> Option<ScrcpySessionInfo> {
        self.sessions.get(udid).map(|entry| entry.value().info.clone())
    }

    /// Get a cloned Arc handle to the session's ScrcpySession for direct control.
    /// Returns ERR_SESSION_NOT_FOUND if no active session exists for the udid.
    /// The DashMap ref is released immediately after cloning the Arc.
    pub fn get_session_handle(
        &self,
        udid: &str,
    ) -> Result<Arc<Mutex<ScrcpySession>>, String> {
        self.sessions
            .get(udid)
            .ok_or_else(|| "ERR_SESSION_NOT_FOUND".to_string())
            .map(|entry| entry.session.clone())
    }

    /// Get both session info and handle in a single DashMap access.
    /// Avoids TOCTOU race between separate get_session() and get_session_handle() calls.
    pub fn get_session_with_info(
        &self,
        udid: &str,
    ) -> Result<(ScrcpySessionInfo, Arc<Mutex<ScrcpySession>>), String> {
        self.sessions
            .get(udid)
            .ok_or_else(|| "ERR_SESSION_NOT_FOUND".to_string())
            .map(|entry| (entry.info.clone(), entry.session.clone()))
    }

    /// Subscribe to the video broadcast for a session.
    /// Returns a new Receiver that will get all future BroadcastFrames.
    /// The DashMap ref is released immediately after subscribing.
    pub fn subscribe_video(
        &self,
        udid: &str,
    ) -> Result<broadcast::Receiver<BroadcastFrame>, String> {
        self.sessions
            .get(udid)
            .ok_or_else(|| "ERR_SESSION_NOT_FOUND".to_string())
            .map(|entry| entry.video_tx.subscribe())
    }

    /// Get a cloned broadcast Sender for a session (e.g. for external producers).
    /// The DashMap ref is released immediately after cloning.
    pub fn get_video_producer(
        &self,
        udid: &str,
    ) -> Result<broadcast::Sender<BroadcastFrame>, String> {
        self.sessions
            .get(udid)
            .ok_or_else(|| "ERR_SESSION_NOT_FOUND".to_string())
            .map(|entry| entry.video_tx.clone())
    }

    // ─── Recording Methods ───

    /// Start recording the scrcpy session for the given device.
    /// Subscribes to the broadcast channel and writes H.264 Annex B to a file.
    pub fn start_recording(&self, udid: &str) -> Result<ScrcpyRecordingInfo, String> {
        // Check session exists and isn't already recording — single DashMap access
        let mut entry = self
            .sessions
            .get_mut(udid)
            .ok_or_else(|| "ERR_SESSION_NOT_FOUND".to_string())?;

        if entry.recording_id.is_some() {
            return Err("ERR_RECORDING_ALREADY_ACTIVE".to_string());
        }

        let recording_id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
        let file_name = format!("{}_{}.h264", udid, timestamp);
        let file_path = PathBuf::from(RECORDINGS_DIR).join(&file_name);

        let info = ScrcpyRecordingInfo {
            id: recording_id.clone(),
            udid: udid.to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
            stopped_at: None,
            duration_ms: None,
            file_size: None,
            frame_count: 0,
            status: "recording".to_string(),
        };

        self.recordings.insert(recording_id.clone(), info.clone());

        let video_rx = entry.video_tx.subscribe();
        entry.recording_id = Some(recording_id.clone());

        // Create oneshot channel for graceful stop signaling
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        entry.recording_stop_tx = Some(stop_tx);

        // Create shared atomic frame counter
        let frame_counter = Arc::new(AtomicU64::new(0));
        let task_frame_counter = frame_counter.clone();
        entry.frame_counter = Some(frame_counter);

        // Spawn recording consumer task
        let recordings = self.recordings.clone();
        let rec_id = recording_id;
        let rec_path = file_path;
        let rec_udid = udid.to_string();
        let task = tokio::spawn(async move {
            Self::recording_consumer_task(
                video_rx,
                rec_path,
                rec_id,
                rec_udid,
                recordings,
                stop_rx,
                task_frame_counter,
            )
            .await;
        });

        entry.recording_task = Some(task);

        tracing::info!(
            "[ScrcpyManager] Recording started for {} → {}",
            udid,
            file_name
        );

        Ok(info)
    }

    /// Recording consumer task: receives broadcast frames and writes H.264 Annex B to file.
    /// Uses a oneshot receiver to support graceful shutdown (allowing BufWriter flush).
    async fn recording_consumer_task(
        mut video_rx: broadcast::Receiver<BroadcastFrame>,
        file_path: PathBuf,
        recording_id: String,
        udid: String,
        recordings: Arc<DashMap<String, ScrcpyRecordingInfo>>,
        mut stop_rx: tokio::sync::oneshot::Receiver<()>,
        frame_counter: Arc<AtomicU64>,
    ) {
        let file = match tokio::fs::File::create(&file_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!(
                    "[ScrcpyManager] Failed to create recording file {:?}: {}",
                    file_path,
                    e
                );
                if let Some(mut rec) = recordings.get_mut(&recording_id) {
                    rec.status = "error".to_string();
                    rec.stopped_at = Some(chrono::Utc::now().to_rfc3339());
                }
                return;
            }
        };

        let mut writer = tokio::io::BufWriter::new(file);

        loop {
            tokio::select! {
                result = video_rx.recv() => {
                    match result {
                        Ok(frame) => {
                            // BroadcastFrame.data format: flags(1) + size(4 BE) + NAL_data(N)
                            if frame.data.len() <= 5 {
                                continue; // Skip empty frames
                            }
                            let nal_data = &frame.data[5..];

                            // Write Annex B: start code + NAL unit
                            if writer.write_all(NAL_START_CODE).await.is_err()
                                || writer.write_all(nal_data).await.is_err()
                            {
                                tracing::error!(
                                    "[ScrcpyManager] Recording write error for {}",
                                    udid
                                );
                                break;
                            }
                            frame_counter.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::debug!(
                                "[ScrcpyManager] Recording for {} lagged, skipped {} frames",
                                udid,
                                n
                            );
                            continue;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            tracing::info!(
                                "[ScrcpyManager] Recording broadcast closed for {} (session stopped)",
                                udid
                            );
                            break;
                        }
                    }
                }
                _ = &mut stop_rx => {
                    tracing::info!(
                        "[ScrcpyManager] Recording stop signal received for {}",
                        udid
                    );
                    break;
                }
            }
        }

        // Finalize: flush writer and update metadata
        let _ = writer.flush().await;

        let final_frame_count = frame_counter.load(Ordering::Relaxed);

        if let Some(mut rec) = recordings.get_mut(&recording_id) {
            rec.frame_count = final_frame_count;
            if rec.status == "recording" {
                let now = chrono::Utc::now();
                rec.status = "completed".to_string();
                rec.stopped_at = Some(now.to_rfc3339());
                if let Some(started) =
                    chrono::DateTime::parse_from_rfc3339(&rec.started_at).ok()
                {
                    rec.duration_ms = Some(
                        (now - started.with_timezone(&chrono::Utc))
                            .num_milliseconds() as u64,
                    );
                } else {
                    tracing::warn!(
                        "[ScrcpyManager] Failed to parse started_at for recording {}: '{}'",
                        recording_id,
                        rec.started_at
                    );
                }
            }
            if let Ok(meta) = std::fs::metadata(&file_path) {
                rec.file_size = Some(meta.len());
            }
        }

        tracing::info!(
            "[ScrcpyManager] Recording finalized for {} ({} frames)",
            udid,
            final_frame_count
        );
    }

    /// Stop recording for the given device. Returns updated recording info.
    /// This is async because it awaits the recording task to flush the BufWriter.
    pub async fn stop_recording(&self, udid: &str) -> Result<ScrcpyRecordingInfo, String> {
        let (recording_id, task, frame_counter) = {
            let mut entry = self
                .sessions
                .get_mut(udid)
                .ok_or_else(|| "ERR_SESSION_NOT_FOUND".to_string())?;

            let recording_id = entry
                .recording_id
                .take()
                .ok_or_else(|| "ERR_NO_ACTIVE_RECORDING".to_string())?;

            // Signal graceful shutdown so BufWriter can flush
            if let Some(tx) = entry.recording_stop_tx.take() {
                let _ = tx.send(());
            }

            let task = entry.recording_task.take();
            let frame_counter = entry.frame_counter.take();

            (recording_id, task, frame_counter)
            // DashMap ref dropped here
        };

        // Wait for the recording task to finish flushing, with a timeout fallback
        if let Some(task) = task {
            match tokio::time::timeout(std::time::Duration::from_secs(2), task).await {
                Ok(_) => {
                    tracing::debug!(
                        "[ScrcpyManager] Recording task for {} finished gracefully",
                        udid
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        "[ScrcpyManager] Recording task for {} did not finish in 2s, data may be truncated",
                        udid
                    );
                }
            }
        }

        // Read final frame count from atomic counter
        let final_frame_count = frame_counter
            .as_ref()
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0);

        // Update recording metadata
        if let Some(mut rec) = self.recordings.get_mut(&recording_id) {
            if rec.status == "recording" {
                let now = chrono::Utc::now();
                rec.status = "completed".to_string();
                rec.stopped_at = Some(now.to_rfc3339());
                rec.frame_count = final_frame_count;
                if let Some(started) =
                    chrono::DateTime::parse_from_rfc3339(&rec.started_at).ok()
                {
                    rec.duration_ms = Some(
                        (now - started.with_timezone(&chrono::Utc))
                            .num_milliseconds() as u64,
                    );
                } else {
                    tracing::warn!(
                        "[ScrcpyManager] Failed to parse started_at for recording {}: '{}'",
                        recording_id,
                        rec.started_at
                    );
                }
                if let Ok(meta) = std::fs::metadata(&rec.file_path) {
                    rec.file_size = Some(meta.len());
                }
            }
        }

        self.recordings
            .get(&recording_id)
            .map(|r| r.value().clone())
            .ok_or_else(|| "ERR_RECORDING_NOT_FOUND".to_string())
    }

    /// List all recordings (completed and in-progress).
    pub fn list_recordings(&self) -> Vec<ScrcpyRecordingInfo> {
        self.recordings
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get a specific recording by ID.
    pub fn get_recording(&self, id: &str) -> Option<ScrcpyRecordingInfo> {
        self.recordings.get(id).map(|r| r.value().clone())
    }

    /// Delete a recording by ID. Removes the file and tracking entry.
    /// Returns an error if the recording is still actively being recorded.
    pub fn delete_recording(&self, id: &str) -> Result<(), String> {
        // Check if the recording is still active before deleting
        if let Some(rec) = self.recordings.get(id) {
            if rec.status == "recording" {
                return Err("ERR_RECORDING_ACTIVE".to_string());
            }
        }

        let entry = self
            .recordings
            .remove(id)
            .ok_or_else(|| "ERR_RECORDING_NOT_FOUND".to_string())?;

        let (_key, info) = entry;

        // Best-effort file deletion
        if let Err(e) = std::fs::remove_file(&info.file_path) {
            tracing::warn!(
                "[ScrcpyManager] Failed to delete recording file {}: {}",
                info.file_path,
                e
            );
        }

        tracing::info!("[ScrcpyManager] Recording {} deleted", id);
        Ok(())
    }

    /// Get the file path for a recording (for download serving).
    pub fn get_recording_file_path(&self, id: &str) -> Option<PathBuf> {
        self.recordings
            .get(id)
            .map(|r| PathBuf::from(&r.file_path))
    }

    /// Ensure scrcpy-server.jar is pushed to the device. Skips if already done this session.
    pub async fn ensure_scrcpy_ready(&self, serial: &str) -> Result<(), String> {
        if self.pushed.contains_key(serial) {
            return Ok(());
        }

        // Check if JAR exists locally
        if !std::path::Path::new(SCRCPY_SERVER_LOCAL).exists() {
            return Err("scrcpy-server.jar not found locally".to_string());
        }

        tracing::info!("[ScrcpyManager] Pushing scrcpy-server.jar to {}", serial);
        Adb::push(serial, SCRCPY_SERVER_LOCAL, SCRCPY_SERVER_REMOTE).await?;
        self.pushed.insert(serial.to_string(), true);
        tracing::info!(
            "[ScrcpyManager] scrcpy-server.jar pushed to {}",
            serial
        );

        Ok(())
    }

    /// Stop all active scrcpy sessions. Used during graceful shutdown.
    pub async fn stop_all_sessions(&self) {
        let udids: Vec<String> = self.sessions.iter().map(|e| e.key().clone()).collect();
        let total = udids.len();
        let mut failed = 0usize;
        for udid in udids {
            if let Err(e) = self.stop_session(&udid).await {
                tracing::warn!("[ScrcpyManager] Failed to stop session for {}: {}", udid, e);
                failed += 1;
            }
        }
        if failed > 0 {
            tracing::warn!("[ScrcpyManager] Shutdown: {}/{} sessions stopped ({} failed)", total - failed, total, failed);
        } else {
            tracing::info!("[ScrcpyManager] Shutdown: {} sessions stopped", total);
        }
    }

    /// Remove tracking for a device (e.g. when it disconnects).
    pub fn remove_device(&self, serial: &str) {
        self.pushed.remove(serial);
    }

    /// Check if scrcpy-server.jar is available locally.
    pub fn jar_available() -> bool {
        std::path::Path::new(SCRCPY_SERVER_LOCAL).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrcpy_session_info_serialization() {
        let info = ScrcpySessionInfo {
            session_id: "test-uuid-1234".to_string(),
            udid: "abc123".to_string(),
            serial: "R5CT900ABCD".to_string(),
            status: "active".to_string(),
            width: 1080,
            height: 1920,
            device_name: "Pixel 6".to_string(),
            started_at: "2026-03-09T12:00:00+00:00".to_string(),
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["session_id"], "test-uuid-1234");
        assert_eq!(json["udid"], "abc123");
        assert_eq!(json["serial"], "R5CT900ABCD");
        assert_eq!(json["status"], "active");
        assert_eq!(json["width"], 1080);
        assert_eq!(json["height"], 1920);
        assert_eq!(json["device_name"], "Pixel 6");
        assert_eq!(json["started_at"], "2026-03-09T12:00:00+00:00");
    }

    #[test]
    fn test_scrcpy_manager_new() {
        let manager = ScrcpyManager::new();
        assert!(manager.list_sessions().is_empty());
        assert!(manager.get_session("nonexistent").is_none());
    }

    #[test]
    fn test_get_session_handle_not_found() {
        let manager = ScrcpyManager::new();
        let result = manager.get_session_handle("nonexistent");
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "ERR_SESSION_NOT_FOUND");
    }

    #[test]
    fn test_subscribe_video_not_found() {
        let manager = ScrcpyManager::new();
        let result = manager.subscribe_video("nonexistent");
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "ERR_SESSION_NOT_FOUND");
    }

    #[test]
    fn test_get_video_producer_not_found() {
        let manager = ScrcpyManager::new();
        let result = manager.get_video_producer("nonexistent");
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "ERR_SESSION_NOT_FOUND");
    }

    #[test]
    fn test_broadcast_frame_from_scrcpy_frame() {
        use crate::device::scrcpy::ScrcpyFrame;

        let frame = ScrcpyFrame {
            pts: 12345,
            is_config: true,
            is_key: false,
            data: vec![0xAA, 0xBB, 0xCC],
        };

        let bf = BroadcastFrame::from_scrcpy_frame(&frame);
        assert_eq!(bf.data.len(), 5 + 3); // 5-byte header + 3 bytes data

        // flags: config=1, key=0 → 0x01
        assert_eq!(bf.data[0], 0x01);

        // size: 3 in big-endian
        assert_eq!(&bf.data[1..5], &[0, 0, 0, 3]);

        // NAL data
        assert_eq!(&bf.data[5..], &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_broadcast_frame_keyframe() {
        use crate::device::scrcpy::ScrcpyFrame;

        let frame = ScrcpyFrame {
            pts: 0,
            is_config: false,
            is_key: true,
            data: vec![0x01],
        };

        let bf = BroadcastFrame::from_scrcpy_frame(&frame);
        // flags: config=0, key=1 → 0x02
        assert_eq!(bf.data[0], 0x02);
    }

    #[test]
    fn test_broadcast_frame_config_and_key() {
        use crate::device::scrcpy::ScrcpyFrame;

        let frame = ScrcpyFrame {
            pts: 0,
            is_config: true,
            is_key: true,
            data: vec![0x01, 0x02],
        };

        let bf = BroadcastFrame::from_scrcpy_frame(&frame);
        // flags: config=1 | key=2 → 0x03
        assert_eq!(bf.data[0], 0x03);
        assert_eq!(&bf.data[1..5], &[0, 0, 0, 2]); // size = 2
    }

    #[test]
    fn test_broadcast_channel_creation() {
        use tokio::sync::broadcast;
        let (tx, mut rx1) = broadcast::channel::<BroadcastFrame>(BROADCAST_CHANNEL_CAPACITY);
        let mut rx2 = tx.subscribe();

        let frame = BroadcastFrame {
            data: Bytes::from_static(b"\x01\x00\x00\x00\x03ABC"),
        };

        // Send one frame
        assert!(tx.send(frame.clone()).is_ok());

        // Both receivers should get it
        let r1 = rx1.try_recv();
        assert!(r1.is_ok());
        assert_eq!(r1.unwrap().data, frame.data);

        let r2 = rx2.try_recv();
        assert!(r2.is_ok());
        assert_eq!(r2.unwrap().data, frame.data);
    }

    // ─── Recording Tests ───

    #[test]
    fn test_scrcpy_recording_info_serialization() {
        let info = ScrcpyRecordingInfo {
            id: "rec-uuid-1234".to_string(),
            udid: "device-abc".to_string(),
            file_path: "recordings/device-abc_20260309T120000.h264".to_string(),
            started_at: "2026-03-09T12:00:00+00:00".to_string(),
            stopped_at: Some("2026-03-09T12:05:00+00:00".to_string()),
            duration_ms: Some(300000),
            file_size: Some(1048576),
            frame_count: 18000,
            status: "completed".to_string(),
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["id"], "rec-uuid-1234");
        assert_eq!(json["udid"], "device-abc");
        assert_eq!(json["file_path"], "recordings/device-abc_20260309T120000.h264");
        assert_eq!(json["started_at"], "2026-03-09T12:00:00+00:00");
        assert_eq!(json["stopped_at"], "2026-03-09T12:05:00+00:00");
        assert_eq!(json["duration_ms"], 300000);
        assert_eq!(json["file_size"], 1048576);
        assert_eq!(json["frame_count"], 18000);
        assert_eq!(json["status"], "completed");
    }

    #[test]
    fn test_start_recording_no_session() {
        let manager = ScrcpyManager::new();
        let result = manager.start_recording("nonexistent");
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "ERR_SESSION_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_stop_recording_no_session() {
        let manager = ScrcpyManager::new();
        let result = manager.stop_recording("nonexistent").await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "ERR_SESSION_NOT_FOUND");
    }

    #[test]
    fn test_delete_recording_not_found() {
        let manager = ScrcpyManager::new();
        let result = manager.delete_recording("nonexistent");
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "ERR_RECORDING_NOT_FOUND");
    }

    #[test]
    fn test_list_recordings_empty() {
        let manager = ScrcpyManager::new();
        let recordings = manager.list_recordings();
        assert!(recordings.is_empty());
    }

    #[test]
    fn test_get_recording_not_found() {
        let manager = ScrcpyManager::new();
        assert!(manager.get_recording("nonexistent").is_none());
    }

    #[test]
    fn test_h264_annex_b_start_code() {
        // Verify the NAL start code constant is correct (Annex B format)
        assert_eq!(NAL_START_CODE, &[0x00, 0x00, 0x00, 0x01]);
    }

    #[test]
    fn test_nal_extraction_from_broadcast_frame() {
        // BroadcastFrame format: flags(1) + size(4 BE) + NAL_data(N)
        let frame = BroadcastFrame {
            data: Bytes::from_static(b"\x01\x00\x00\x00\x03\xAA\xBB\xCC"),
        };

        // Extract NAL data by skipping 5-byte header
        let nal_data = &frame.data[5..];
        assert_eq!(nal_data, &[0xAA, 0xBB, 0xCC]);

        // Verify flags extraction
        let flags = frame.data[0];
        assert_eq!(flags & 0x01, 1); // is_config
        assert_eq!(flags & 0x02, 0); // not is_key
    }

    #[test]
    fn test_get_recording_file_path_not_found() {
        let manager = ScrcpyManager::new();
        assert!(manager.get_recording_file_path("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_stop_all_sessions_empty() {
        let manager = ScrcpyManager::new();
        // Should complete without error on empty state
        manager.stop_all_sessions().await;
        assert!(manager.list_sessions().is_empty());
    }
}
