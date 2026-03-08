use crate::models::recording::{
    ActionType, RecordedAction, RecordingSession, RecordActionRequest,
    StartRecordingResponse, RecordActionResponse, StopRecordingResponse,
    StopRecordingRequest, ListRecordingsResponse, RecordingWithActions,
    EditActionRequest, RecordingStatusResponse,
};
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Recording state management for active recording sessions.
/// Maps device_udid -> recording_id
#[derive(Default)]
pub struct RecordingStateInner {
    active_recordings: HashMap<String, i64>,
    paused_recordings: HashSet<String>,
}

/// Thread-safe wrapper for recording state.
#[derive(Clone)]
pub struct RecordingState {
    inner: Arc<RwLock<RecordingStateInner>>,
}

impl RecordingState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RecordingStateInner::default())),
        }
    }

    /// Check if a device has an active recording session.
    pub async fn is_recording(&self, device_udid: &str) -> bool {
        self.inner.read().await.active_recordings.contains_key(device_udid)
    }

    /// Get the active recording ID for a device.
    pub async fn get_active_recording(&self, device_udid: &str) -> Option<i64> {
        self.inner.read().await.active_recordings.get(device_udid).copied()
    }

    /// Start a recording session for a device.
    pub async fn start_recording(&self, device_udid: &str, recording_id: i64) -> Result<(), String> {
        let mut state = self.inner.write().await;
        if state.active_recordings.contains_key(device_udid) {
            return Err(format!(
                "Device {} already has an active recording session",
                device_udid
            ));
        }
        state.active_recordings.insert(device_udid.to_string(), recording_id);
        Ok(())
    }

    /// Stop a recording session for a device.
    pub async fn stop_recording(&self, device_udid: &str) -> Option<i64> {
        let mut state = self.inner.write().await;
        state.paused_recordings.remove(device_udid);
        state.active_recordings.remove(device_udid)
    }

    /// Pause a recording session for a device.
    pub async fn pause_recording(&self, device_udid: &str) -> Result<(), String> {
        let mut state = self.inner.write().await;
        if !state.active_recordings.contains_key(device_udid) {
            return Err("ERR_NO_ACTIVE_RECORDING".to_string());
        }
        if state.paused_recordings.contains(device_udid) {
            return Err("ERR_RECORDING_ALREADY_PAUSED".to_string());
        }
        state.paused_recordings.insert(device_udid.to_string());
        Ok(())
    }

    /// Resume a paused recording session for a device.
    pub async fn resume_recording(&self, device_udid: &str) -> Result<(), String> {
        let mut state = self.inner.write().await;
        if !state.active_recordings.contains_key(device_udid) {
            return Err("ERR_NO_ACTIVE_RECORDING".to_string());
        }
        if !state.paused_recordings.contains(device_udid) {
            return Err("ERR_RECORDING_NOT_PAUSED".to_string());
        }
        state.paused_recordings.remove(device_udid);
        Ok(())
    }

    /// Check if a recording is paused.
    pub async fn is_paused(&self, device_udid: &str) -> bool {
        self.inner.read().await.paused_recordings.contains(device_udid)
    }

    /// Cancel a recording without saving (removes from active state).
    pub async fn cancel_recording(&self, device_udid: &str) -> Option<i64> {
        let mut state = self.inner.write().await;
        state.paused_recordings.remove(device_udid);
        state.active_recordings.remove(device_udid)
    }
}

/// Service for managing recording sessions.
#[derive(Clone)]
pub struct RecordingService {
    pool: sqlx::SqlitePool,
    recording_state: RecordingState,
}

impl RecordingService {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            pool,
            recording_state: RecordingState::new(),
        }
    }

    /// Start a new recording session.
    pub async fn start_recording(
        &self,
        device_udid: &str,
        name: &str,
    ) -> Result<StartRecordingResponse, String> {
        // Check if device already has an active recording
        if self.recording_state.is_recording(device_udid).await {
            return Err(format!(
                "Device {} already has an active recording session",
                device_udid
            ));
        }

        // Create the recording in database
        let now = chrono::Utc::now().timestamp();
        let result = sqlx::query(
            "INSERT INTO recordings (name, device_udid, action_count, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?3)"
        )
        .bind(name)
        .bind(device_udid)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to create recording: {}", e))?;

        let recording_id = result.last_insert_rowid();

        // Track in memory state
        self.recording_state.start_recording(device_udid, recording_id).await?;

        Ok(StartRecordingResponse {
            status: "success".to_string(),
            recording_id,
            message: "Recording started".to_string(),
        })
    }

    /// Record an action in an active recording session.
    pub async fn record_action(
        &self,
        device_udid: &str,
        request: RecordActionRequest,
    ) -> Result<RecordActionResponse, String> {
        // Get active recording ID
        let recording_id = self
            .recording_state
            .get_active_recording(device_udid)
            .await
            .ok_or_else(|| "ERR_NO_ACTIVE_RECORDING".to_string())?;

        // Get next sequence order
        let sequence_order: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence_order), -1) + 1 FROM recorded_actions WHERE recording_id = ?1"
        )
        .bind(recording_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to get sequence order: {}", e))?;

        let now = chrono::Utc::now().timestamp();

        // Record the action
        let result = sqlx::query(
            r#"
            INSERT INTO recorded_actions
            (recording_id, action_type, x, y, x2, y2, duration_ms, text, key_code, sequence_order, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#
        )
        .bind(recording_id)
        .bind(request.action_type.as_str())
        .bind(request.x)
        .bind(request.y)
        .bind(request.x2)
        .bind(request.y2)
        .bind(request.duration_ms)
        .bind(request.text.as_deref())
        .bind(request.key_code)
        .bind(sequence_order)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to record action: {}", e))?;

        let action_id = result.last_insert_rowid();

        // Update recording action count
        sqlx::query(
            "UPDATE recordings SET action_count = (SELECT COUNT(*) FROM recorded_actions WHERE recording_id = ?1), updated_at = ?2 WHERE id = ?1"
        )
        .bind(recording_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update recording: {}", e))?;

        Ok(RecordActionResponse {
            status: "success".to_string(),
            action_id,
            sequence_order,
        })
    }

    /// Stop and save a recording session.
    pub async fn stop_recording(
        &self,
        device_udid: &str,
        request: StopRecordingRequest,
    ) -> Result<StopRecordingResponse, String> {
        // Get and remove active recording
        let recording_id = self
            .recording_state
            .stop_recording(device_udid)
            .await
            .ok_or_else(|| "ERR_NO_ACTIVE_RECORDING".to_string())?;

        // Update the recording name
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "UPDATE recordings SET name = ?1, updated_at = ?2 WHERE id = ?3"
        )
        .bind(&request.name)
        .bind(now)
        .bind(recording_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update recording: {}", e))?;

        // Get the updated recording
        let recording = self.get_recording(recording_id).await?;

        Ok(StopRecordingResponse {
            status: "success".to_string(),
            recording: RecordingSession {
                id: recording.id,
                name: recording.name,
                device_udid: recording.device_udid,
                action_count: recording.action_count,
                created_at: recording.created_at,
                updated_at: recording.updated_at,
            },
        })
    }

    /// List all recordings.
    pub async fn list_recordings(&self) -> Result<ListRecordingsResponse, String> {
        let rows = sqlx::query(
            "SELECT id, name, device_udid, action_count, created_at, updated_at FROM recordings ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to list recordings: {}", e))?;

        let recordings: Vec<RecordingSession> = rows
            .iter()
            .map(|row| RecordingSession {
                id: row.get("id"),
                name: row.get("name"),
                device_udid: row.get("device_udid"),
                action_count: row.get("action_count"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(ListRecordingsResponse {
            status: "success".to_string(),
            recordings,
        })
    }

    /// Get a recording with all its actions.
    pub async fn get_recording(&self, id: i64) -> Result<RecordingWithActions, String> {
        let row = sqlx::query(
            "SELECT id, name, device_udid, action_count, created_at, updated_at FROM recordings WHERE id = ?1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to get recording: {}", e))?
        .ok_or_else(|| format!("Recording {} not found", id))?;

        let session = RecordingSession {
            id: row.get("id"),
            name: row.get("name"),
            device_udid: row.get("device_udid"),
            action_count: row.get("action_count"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };

        // Get actions for this recording
        let action_rows = sqlx::query(
            r#"
            SELECT id, recording_id, action_type, x, y, x2, y2, duration_ms, text, key_code, sequence_order, created_at
            FROM recorded_actions
            WHERE recording_id = ?1
            ORDER BY sequence_order ASC
            "#
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to get recording actions: {}", e))?;

        let actions: Vec<RecordedAction> = action_rows
            .iter()
            .map(|row| {
                let action_type_str: String = row.get("action_type");
                RecordedAction {
                    id: row.get("id"),
                    recording_id: row.get("recording_id"),
                    action_type: ActionType::from_str(&action_type_str)
                        .unwrap_or(ActionType::Tap),
                    x: row.get("x"),
                    y: row.get("y"),
                    x2: row.get("x2"),
                    y2: row.get("y2"),
                    duration_ms: row.get("duration_ms"),
                    text: row.get("text"),
                    key_code: row.get("key_code"),
                    sequence_order: row.get("sequence_order"),
                    created_at: row.get("created_at"),
                }
            })
            .collect();

        Ok(RecordingWithActions {
            id: session.id,
            name: session.name,
            device_udid: session.device_udid,
            action_count: session.action_count,
            created_at: session.created_at,
            updated_at: session.updated_at,
            actions,
        })
    }

    /// Delete a recording and all its actions.
    pub async fn delete_recording(&self, id: i64) -> Result<(), String> {
        // Delete actions first (cascade should handle this, but be explicit)
        sqlx::query("DELETE FROM recorded_actions WHERE recording_id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to delete recording actions: {}", e))?;

        // Delete the recording
        let result = sqlx::query("DELETE FROM recordings WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to delete recording: {}", e))?;

        if result.rows_affected() == 0 {
            return Err(format!("Recording {} not found", id));
        }

        Ok(())
    }

    /// Check if a device has an active recording.
    pub async fn is_recording(&self, device_udid: &str) -> bool {
        self.recording_state.is_recording(device_udid).await
    }

    /// Get the active recording ID for a device (if any).
    pub async fn get_active_recording_id(&self, device_udid: &str) -> Option<i64> {
        self.recording_state.get_active_recording(device_udid).await
    }

    /// Pause an active recording session.
    pub async fn pause_recording(&self, device_udid: &str) -> Result<(), String> {
        self.recording_state.pause_recording(device_udid).await
    }

    /// Resume a paused recording session.
    pub async fn resume_recording(&self, device_udid: &str) -> Result<(), String> {
        self.recording_state.resume_recording(device_udid).await
    }

    /// Check if a recording is paused.
    pub async fn is_paused(&self, device_udid: &str) -> bool {
        self.recording_state.is_paused(device_udid).await
    }

    /// Cancel a recording without saving (deletes from database).
    pub async fn cancel_recording(&self, device_udid: &str) -> Result<(), String> {
        // Get and remove from state
        let recording_id = self
            .recording_state
            .cancel_recording(device_udid)
            .await
            .ok_or_else(|| "ERR_NO_ACTIVE_RECORDING".to_string())?;

        // Delete from database
        self.delete_recording(recording_id).await?;

        Ok(())
    }

    /// Get the status of a recording session for a device.
    pub async fn get_recording_status(&self, device_udid: &str) -> Result<RecordingStatusResponse, String> {
        let recording_id = self
            .recording_state
            .get_active_recording(device_udid)
            .await
            .ok_or_else(|| "ERR_NO_ACTIVE_RECORDING".to_string())?;

        let is_paused = self.recording_state.is_paused(device_udid).await;

        // Get action count
        let action_count: i32 = sqlx::query_scalar(
            "SELECT action_count FROM recordings WHERE id = ?1"
        )
        .bind(recording_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to get recording status: {}", e))?;

        Ok(RecordingStatusResponse {
            status: "success".to_string(),
            recording_id,
            recording_status: if is_paused { "paused" } else { "active" }.to_string(),
            action_count,
        })
    }

    /// Update an action in a recording.
    pub async fn update_action(
        &self,
        recording_id: i64,
        action_id: i64,
        request: EditActionRequest,
    ) -> Result<RecordedAction, String> {
        // Verify action belongs to this recording
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM recorded_actions WHERE id = ?1 AND recording_id = ?2)"
        )
        .bind(action_id)
        .bind(recording_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to verify action: {}", e))?;

        if !exists {
            return Err("ERR_ACTION_NOT_FOUND".to_string());
        }

        let now = chrono::Utc::now().timestamp();

        // Update the action with provided fields
        sqlx::query(
            r#"
            UPDATE recorded_actions
            SET x = COALESCE(?1, x),
                y = COALESCE(?2, y),
                x2 = COALESCE(?3, x2),
                y2 = COALESCE(?4, y2),
                duration_ms = COALESCE(?5, duration_ms),
                text = COALESCE(?6, text),
                key_code = COALESCE(?7, key_code)
            WHERE id = ?8
            "#
        )
        .bind(request.x)
        .bind(request.y)
        .bind(request.x2)
        .bind(request.y2)
        .bind(request.duration_ms)
        .bind(request.text.as_deref())
        .bind(request.key_code)
        .bind(action_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update action: {}", e))?;

        // Update recording updated_at
        sqlx::query("UPDATE recordings SET updated_at = ?1 WHERE id = ?2")
            .bind(now)
            .bind(recording_id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to update recording timestamp: {}", e))?;

        // Fetch and return the updated action
        let row = sqlx::query(
            "SELECT id, recording_id, action_type, x, y, x2, y2, duration_ms, text, key_code, sequence_order, created_at FROM recorded_actions WHERE id = ?1"
        )
        .bind(action_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch updated action: {}", e))?;

        let action_type_str: String = row.get("action_type");
        Ok(RecordedAction {
            id: row.get("id"),
            recording_id: row.get("recording_id"),
            action_type: ActionType::from_str(&action_type_str).unwrap_or(ActionType::Tap),
            x: row.get("x"),
            y: row.get("y"),
            x2: row.get("x2"),
            y2: row.get("y2"),
            duration_ms: row.get("duration_ms"),
            text: row.get("text"),
            key_code: row.get("key_code"),
            sequence_order: row.get("sequence_order"),
            created_at: row.get("created_at"),
        })
    }

    /// Delete an action from a recording and renumber remaining actions.
    pub async fn delete_action(&self, recording_id: i64, action_id: i64) -> Result<(), String> {
        // Verify action belongs to this recording and get its sequence order
        let sequence_order: i32 = sqlx::query_scalar(
            "SELECT sequence_order FROM recorded_actions WHERE id = ?1 AND recording_id = ?2"
        )
        .bind(action_id)
        .bind(recording_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to verify action: {}", e))?
        .ok_or_else(|| "ERR_ACTION_NOT_FOUND".to_string())?;

        let now = chrono::Utc::now().timestamp();

        // Delete the action
        sqlx::query("DELETE FROM recorded_actions WHERE id = ?1")
            .bind(action_id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to delete action: {}", e))?;

        // Renumber actions after the deleted one
        sqlx::query(
            "UPDATE recorded_actions SET sequence_order = sequence_order - 1 WHERE recording_id = ?1 AND sequence_order > ?2"
        )
        .bind(recording_id)
        .bind(sequence_order)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to renumber actions: {}", e))?;

        // Update recording action count and timestamp
        sqlx::query(
            "UPDATE recordings SET action_count = (SELECT COUNT(*) FROM recorded_actions WHERE recording_id = ?1), updated_at = ?2 WHERE id = ?1"
        )
        .bind(recording_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update recording: {}", e))?;

        Ok(())
    }

    /// Check if recording should capture actions (active and not paused).
    pub async fn should_capture(&self, device_udid: &str) -> bool {
        self.recording_state.is_recording(device_udid).await
            && !self.recording_state.is_paused(device_udid).await
    }
}
