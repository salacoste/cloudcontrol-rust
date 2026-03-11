use crate::db::Database;
use chrono::Utc;
use dashmap::DashMap;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};

/// Directory for video recordings (shared with scrcpy recordings).
const RECORDINGS_DIR: &str = "recordings";

/// Metadata for a video recording session.
#[derive(Debug, Clone, Serialize)]
pub struct VideoRecordingInfo {
    pub id: String,
    pub udid: String,
    pub file_path: String,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<String>,
    pub frame_count: u64,
    pub fps: u32,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

/// Internal state for an active recording (not serializable due to Child handle).
struct ActiveRecording {
    ffmpeg_child: Child,
    /// Stdin held separately behind an async mutex so we can write without holding the DashMap guard.
    stdin: Arc<tokio::sync::Mutex<tokio::process::ChildStdin>>,
    frame_counter: Arc<AtomicU64>,
}

/// VideoService manages JPEG-to-MP4 video recording via FFmpeg child processes.
#[derive(Clone)]
pub struct VideoService {
    /// recording_id → active metadata (only in-progress recordings kept in memory)
    recordings: Arc<DashMap<String, VideoRecordingInfo>>,
    /// recording_id → active FFmpeg child (only in-progress recordings)
    active: Arc<DashMap<String, ActiveRecording>>,
    /// SQLite database for persistent storage of completed/failed recordings
    db: Database,
}

impl VideoService {
    pub fn new(db: Database) -> Self {
        // Ensure recordings directory exists
        let _ = std::fs::create_dir_all(RECORDINGS_DIR);
        Self {
            recordings: Arc::new(DashMap::new()),
            active: Arc::new(DashMap::new()),
            db,
        }
    }

    /// Start a new video recording for the given device.
    /// Spawns an FFmpeg child process that reads JPEG frames from stdin.
    pub async fn start_recording(
        &self,
        udid: &str,
        fps: u32,
        device_name: Option<String>,
    ) -> Result<VideoRecordingInfo, String> {
        let recording_id = uuid::Uuid::new_v4().to_string();
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S").to_string();
        let file_name = format!("video_{}_{}.mp4", udid, timestamp);
        let file_path = PathBuf::from(RECORDINGS_DIR).join(&file_name);

        let mut child = Command::new("ffmpeg")
            .args([
                "-f",
                "image2pipe",
                "-framerate",
                &fps.to_string(),
                "-i",
                "pipe:0",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-preset",
                "fast",
                "-crf",
                "23",
                "-movflags",
                "+faststart",
                "-y", // overwrite if exists
                file_path.to_str().unwrap_or("output.mp4"),
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn FFmpeg: {}", e))?;

        // Extract stdin from child so it can be held behind an async mutex
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture FFmpeg stdin".to_string())?;
        let stdin = Arc::new(tokio::sync::Mutex::new(stdin));
        let frame_counter = Arc::new(AtomicU64::new(0));

        let info = VideoRecordingInfo {
            id: recording_id.clone(),
            udid: udid.to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            started_at: Utc::now().to_rfc3339(),
            stopped_at: None,
            frame_count: 0,
            fps,
            status: "recording".to_string(),
            duration_ms: None,
            file_size: None,
            device_name,
        };

        self.recordings.insert(recording_id.clone(), info.clone());
        self.active.insert(
            recording_id,
            ActiveRecording {
                ffmpeg_child: child,
                stdin,
                frame_counter,
            },
        );

        // Best-effort SQLite persistence — don't fail the recording on DB error
        if let Err(e) = self.db.insert_video(&info).await {
            tracing::warn!("[VideoService] Failed to persist video to SQLite: {}", e);
        }

        tracing::info!(
            "[VideoService] Started recording for device {} → {}",
            udid,
            file_path.display()
        );
        Ok(info)
    }

    /// Feed a JPEG frame to an active recording.
    pub async fn feed_frame(&self, id: &str, jpeg_data: &[u8]) -> Result<(), String> {
        // Clone Arc handles out of DashMap before awaiting to avoid holding sync lock across .await
        let (stdin, counter) = {
            let entry = self
                .active
                .get(id)
                .ok_or_else(|| "ERR_RECORDING_NOT_FOUND".to_string())?;
            (entry.stdin.clone(), entry.frame_counter.clone())
        }; // DashMap guard dropped here

        let mut stdin_lock = stdin.lock().await;
        stdin_lock
            .write_all(jpeg_data)
            .await
            .map_err(|e| format!("Failed to write frame to FFmpeg: {}", e))?;
        drop(stdin_lock);

        counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Stop an active recording. Closes FFmpeg stdin and waits for the process to finish.
    pub async fn stop_recording(&self, id: &str) -> Result<VideoRecordingInfo, String> {
        // Remove from active map
        let (_, mut active) = self
            .active
            .remove(id)
            .ok_or_else(|| "ERR_RECORDING_NOT_FOUND".to_string())?;

        // Update status to finalizing
        if let Some(mut rec) = self.recordings.get_mut(id) {
            rec.status = "finalizing".to_string();
        }

        let final_frame_count = active.frame_counter.load(Ordering::Relaxed);

        // Close stdin to signal FFmpeg to finish encoding
        drop(active.stdin);

        // Wait for FFmpeg to finish (with timeout)
        let wait_result =
            tokio::time::timeout(std::time::Duration::from_secs(30), active.ffmpeg_child.wait())
                .await;

        let success = match wait_result {
            Ok(Ok(status)) => {
                if !status.success() {
                    // Read stderr for diagnostics on failure
                    if let Some(mut stderr) = active.ffmpeg_child.stderr.take() {
                        let mut stderr_buf = String::new();
                        if let Ok(_) =
                            tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut stderr_buf)
                                .await
                        {
                            if !stderr_buf.is_empty() {
                                tracing::warn!(
                                    "[VideoService] FFmpeg stderr for {}: {}",
                                    id,
                                    stderr_buf.lines().last().unwrap_or(&stderr_buf)
                                );
                            }
                        }
                    }
                }
                status.success()
            }
            Ok(Err(e)) => {
                tracing::warn!("[VideoService] FFmpeg wait error for {}: {}", id, e);
                false
            }
            Err(_) => {
                tracing::warn!("[VideoService] FFmpeg timeout for {}, killing process", id);
                let _ = active.ffmpeg_child.kill().await;
                false
            }
        };

        // Update metadata
        if let Some(mut rec) = self.recordings.get_mut(id) {
            let now = Utc::now();
            rec.stopped_at = Some(now.to_rfc3339());
            rec.frame_count = final_frame_count;
            rec.status = if success {
                "completed".to_string()
            } else {
                "failed".to_string()
            };

            // Calculate duration
            if let Ok(started) = chrono::DateTime::parse_from_rfc3339(&rec.started_at) {
                rec.duration_ms = Some((now - started.with_timezone(&Utc)).num_milliseconds().max(0) as u64);
            }

            // Get file size
            if success {
                if let Ok(meta) = std::fs::metadata(&rec.file_path) {
                    rec.file_size = Some(meta.len());
                }
            }

            tracing::info!(
                "[VideoService] Stopped recording {} — {} frames, status: {}",
                id,
                final_frame_count,
                rec.status
            );
        }

        // Clone info out before potential DashMap removal
        let info = self
            .recordings
            .get(id)
            .map(|r| r.value().clone())
            .ok_or_else(|| "ERR_RECORDING_NOT_FOUND".to_string())?;

        // Persist final state to SQLite
        if let Err(e) = self.db.update_video(&info).await {
            tracing::warn!("[VideoService] Failed to update video in SQLite: {}", e);
        }

        // Remove from in-memory DashMap — completed recordings live in SQLite only
        self.recordings.remove(id);

        Ok(info)
    }

    /// Get a specific recording by ID. Checks in-memory DashMap first, then SQLite.
    pub async fn get_recording(&self, id: &str) -> Option<VideoRecordingInfo> {
        // Check DashMap first for active recordings
        if let Some(entry) = self.recordings.get(id) {
            return Some(entry.value().clone());
        }
        // Fall back to SQLite for completed/failed recordings
        match self.db.get_video(id).await {
            Ok(info) => info,
            Err(e) => {
                tracing::warn!("[VideoService] Failed to query video from SQLite: {}", e);
                None
            }
        }
    }

    /// List all recordings (active from DashMap + completed from SQLite), with optional filters.
    pub async fn list_recordings(
        &self,
        udid: Option<&str>,
        status: Option<&str>,
    ) -> Vec<VideoRecordingInfo> {
        let mut results = Vec::new();

        // Get active recordings from DashMap (status: recording/finalizing)
        for entry in self.recordings.iter() {
            let info = entry.value();
            let udid_match = udid.map_or(true, |u| info.udid == u);
            let status_match = status.map_or(true, |s| info.status == s);
            if udid_match && status_match {
                results.push(info.clone());
            }
        }

        // Get completed/failed recordings from SQLite
        match self.db.list_videos(udid, status).await {
            Ok(db_recordings) => {
                // Avoid duplicates — DashMap IDs take precedence
                let active_ids: std::collections::HashSet<String> =
                    self.recordings.iter().map(|e| e.key().clone()).collect();
                for rec in db_recordings {
                    if !active_ids.contains(&rec.id) {
                        results.push(rec);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("[VideoService] Failed to list videos from SQLite: {}", e);
            }
        }

        results
    }

    /// Delete a completed recording and its file.
    pub async fn delete_recording(&self, id: &str) -> Result<(), String> {
        // Don't allow deleting active recordings
        if self.active.contains_key(id) {
            return Err("ERR_RECORDING_ACTIVE".to_string());
        }

        // Try to remove from DashMap first (in case it's still there)
        let info = if let Some((_, info)) = self.recordings.remove(id) {
            Some(info)
        } else {
            // Fall back to SQLite lookup
            self.db
                .get_video(id)
                .await
                .map_err(|e| format!("Database error: {}", e))?
        };

        let info = info.ok_or_else(|| "ERR_RECORDING_NOT_FOUND".to_string())?;

        // Delete from SQLite
        if let Err(e) = self.db.delete_video(id).await {
            tracing::warn!("[VideoService] Failed to delete video from SQLite: {}", e);
        }

        // Best-effort file deletion
        if let Err(e) = std::fs::remove_file(&info.file_path) {
            tracing::warn!(
                "[VideoService] Failed to delete recording file {}: {}",
                info.file_path,
                e
            );
        }

        Ok(())
    }

    /// Stop all active video recordings. Used during graceful shutdown.
    pub async fn stop_all_active(&self) {
        let ids: Vec<String> = self.active.iter().map(|e| e.key().clone()).collect();
        let count = ids.len();
        for id in ids {
            if let Err(e) = self.stop_recording(&id).await {
                tracing::warn!("[VideoService] Failed to stop recording {}: {}", id, e);
            }
        }
        tracing::info!("[VideoService] Stopped {} active recordings during shutdown", count);
    }

    /// Reconcile database with filesystem on startup.
    /// - Marks stale active recordings (from previous run) as failed
    /// - Registers orphaned MP4 files not in the database
    /// - Marks DB records with missing files as failed
    pub async fn recover_on_startup(&self) -> Result<(), String> {
        let mut recovered = 0u64;
        let mut cleaned = 0u64;
        let mut registered = 0u64;

        // Step 1: Mark stale active recordings as failed
        for from_status in &["recording", "finalizing"] {
            match self.db.update_videos_status(from_status, "failed").await {
                Ok(count) => cleaned += count,
                Err(e) => {
                    tracing::warn!(
                        "[VideoService] Failed to mark stale '{}' recordings: {}",
                        from_status,
                        e
                    );
                }
            }
        }

        // Step 2: Scan recordings/ directory for orphaned MP4 files
        let recordings_dir = std::path::Path::new(RECORDINGS_DIR);
        if recordings_dir.exists() {
            // Query DB once to get all known file paths — avoid N+1 queries
            let known_paths: std::collections::HashSet<String> =
                match self.db.list_videos(None, None).await {
                    Ok(db_videos) => db_videos.into_iter().map(|v| v.file_path).collect(),
                    Err(e) => {
                        tracing::warn!(
                            "[VideoService] Failed to load DB videos for orphan check: {}",
                            e
                        );
                        std::collections::HashSet::new()
                    }
                };

            if let Ok(entries) = std::fs::read_dir(recordings_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let file_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();

                    // Only process video_*.mp4 files
                    if !file_name.starts_with("video_") || !file_name.ends_with(".mp4") {
                        continue;
                    }

                    let file_path_str = path.to_string_lossy().to_string();

                    if !known_paths.contains(&file_path_str) {
                        // Parse udid and timestamp from filename: video_{udid}_{timestamp}.mp4
                        let stem = file_name.trim_end_matches(".mp4");
                        let parts: Vec<&str> =
                            stem.trim_start_matches("video_").rsplitn(2, '_').collect();
                        let (udid, _timestamp) = if parts.len() == 2 {
                            (parts[1].to_string(), parts[0])
                        } else {
                            ("unknown".to_string(), "")
                        };

                        let file_size = std::fs::metadata(&path).ok().map(|m| m.len());

                        let info = VideoRecordingInfo {
                            id: uuid::Uuid::new_v4().to_string(),
                            udid,
                            file_path: file_path_str,
                            started_at: Utc::now().to_rfc3339(),
                            stopped_at: Some(Utc::now().to_rfc3339()),
                            frame_count: 0,
                            fps: 2,
                            status: "recovered".to_string(),
                            duration_ms: None,
                            file_size,
                            device_name: None,
                        };

                        if let Err(e) = self.db.insert_video(&info).await {
                            tracing::warn!(
                                "[VideoService] Failed to register orphaned file {}: {}",
                                file_name,
                                e
                            );
                        } else {
                            registered += 1;
                        }
                    }
                }
            }
        }

        // Step 3: Check DB records for missing files
        match self.db.list_videos(None, None).await {
            Ok(db_videos) => {
                for video in db_videos {
                    if video.status == "failed" {
                        continue; // Already marked as failed
                    }
                    if !std::path::Path::new(&video.file_path).exists() {
                        let mut updated = video.clone();
                        updated.status = "failed".to_string();
                        updated.file_size = None;
                        if let Err(e) = self.db.update_video(&updated).await {
                            tracing::warn!(
                                "[VideoService] Failed to mark missing-file video {}: {}",
                                video.id,
                                e
                            );
                        } else {
                            recovered += 1;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "[VideoService] Failed to check DB records for missing files: {}",
                    e
                );
            }
        }

        tracing::info!(
            "[VideoService] Startup recovery: cleaned {} stale records, registered {} orphaned files, recovered {} missing-file records",
            cleaned,
            registered,
            recovered
        );

        Ok(())
    }
}

/// Check if FFmpeg is available on the system.
pub async fn check_ffmpeg_available() -> bool {
    match Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
    {
        Ok(status) => {
            if status.success() {
                tracing::info!("[VideoService] FFmpeg is available");
                true
            } else {
                tracing::warn!("[VideoService] FFmpeg exited with non-zero status");
                false
            }
        }
        Err(e) => {
            tracing::warn!("[VideoService] FFmpeg not found: {}", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn test_db() -> (tempfile::TempDir, Database) {
        let tmp = tempdir().unwrap();
        let db = Database::new(tmp.path().to_str().unwrap(), "test.db")
            .await
            .unwrap();
        (tmp, db)
    }

    #[test]
    fn test_video_recording_info_serialization() {
        let info = VideoRecordingInfo {
            id: "test-id".to_string(),
            udid: "device-123".to_string(),
            file_path: "recordings/video_device-123_20260311T120000.mp4".to_string(),
            started_at: "2026-03-11T12:00:00Z".to_string(),
            stopped_at: None,
            frame_count: 0,
            fps: 2,
            status: "recording".to_string(),
            duration_ms: None,
            file_size: None,
            device_name: Some("Pixel 6".to_string()),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["id"], "test-id");
        assert_eq!(json["fps"], 2);
        assert_eq!(json["device_name"], "Pixel 6");
        // Optional None fields should be absent
        assert!(json.get("stopped_at").is_none());
        assert!(json.get("duration_ms").is_none());
        assert!(json.get("file_size").is_none());
    }

    #[tokio::test]
    async fn test_video_service_new() {
        let (_tmp, db) = test_db().await;
        let service = VideoService::new(db);
        assert_eq!(service.list_recordings(None, None).await.len(), 0);
    }

    #[tokio::test]
    async fn test_video_service_delete_not_found() {
        let (_tmp, db) = test_db().await;
        let service = VideoService::new(db);
        let result = service.delete_recording("nonexistent").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "ERR_RECORDING_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_video_service_get_not_found() {
        let (_tmp, db) = test_db().await;
        let service = VideoService::new(db);
        assert!(service.get_recording("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_stop_all_active_empty() {
        let (_tmp, db) = test_db().await;
        let service = VideoService::new(db);
        // Should complete without error on empty state
        service.stop_all_active().await;
        assert_eq!(service.list_recordings(None, None).await.len(), 0);
    }
}
