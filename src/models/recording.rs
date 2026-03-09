use serde::{Deserialize, Serialize};

/// Action types that can be recorded and replayed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ActionType {
    Tap,
    Swipe,
    Input,
    KeyEvent,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionType::Tap => "tap",
            ActionType::Swipe => "swipe",
            ActionType::Input => "input",
            ActionType::KeyEvent => "keyevent",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "tap" => Some(ActionType::Tap),
            "swipe" => Some(ActionType::Swipe),
            "input" => Some(ActionType::Input),
            "keyevent" => Some(ActionType::KeyEvent),
            _ => None,
        }
    }
}

/// A single recorded action within a recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedAction {
    /// Unique ID of the action
    pub id: i64,
    /// ID of the parent recording session
    pub recording_id: i64,
    /// Type of action (tap, swipe, input, keyevent)
    pub action_type: ActionType,
    /// X coordinate for tap/swipe start
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    /// Y coordinate for tap/swipe start
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    /// X2 coordinate for swipe end
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x2: Option<i32>,
    /// Y2 coordinate for swipe end
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y2: Option<i32>,
    /// Duration in milliseconds for swipe
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i32>,
    /// Text input content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Key code for key events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_code: Option<i32>,
    /// Order in the recording sequence (0-indexed)
    pub sequence_order: i32,
    /// Unix timestamp when action was recorded
    pub created_at: i64,
}

/// A recording session containing multiple recorded actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSession {
    /// Unique ID of the recording
    pub id: i64,
    /// Human-readable name for the recording
    pub name: String,
    /// Device UDID where recording was made
    pub device_udid: String,
    /// Number of actions in this recording
    pub action_count: i32,
    /// Unix timestamp when recording was created
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    /// Unix timestamp when recording was last updated
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
}

/// Request to start a new recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRecordingRequest {
    pub device_udid: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Request to record an action in an active session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordActionRequest {
    pub action_type: ActionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x2: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y2: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_code: Option<i32>,
}

/// Request to stop and save a recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopRecordingRequest {
    pub name: String,
}

/// Response for starting a recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRecordingResponse {
    pub status: String,
    pub recording_id: i64,
    pub message: String,
}

/// Response for recording an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordActionResponse {
    pub status: String,
    pub action_id: i64,
    pub sequence_order: i32,
}

/// Response for stopping a recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopRecordingResponse {
    pub status: String,
    pub recording: RecordingSession,
}

/// Response for listing recordings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRecordingsResponse {
    pub status: String,
    pub recordings: Vec<RecordingSession>,
}

/// Full recording with all actions for playback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingWithActions {
    pub id: i64,
    pub name: String,
    pub device_udid: String,
    pub action_count: i32,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    pub actions: Vec<RecordedAction>,
}

/// Request to edit an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditActionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x2: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y2: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_code: Option<i32>,
}

/// Response for recording status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStatusResponse {
    pub status: String,
    pub recording_id: i64,
    pub recording_status: String,
    pub action_count: i32,
}

/// Request to start playback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartPlaybackRequest {
    /// Target device UDID for playback
    pub target_device_udid: String,
    /// Playback speed multiplier (1.0 = normal, 2.0 = double speed, 0.5 = half speed)
    #[serde(default)]
    pub speed: f32,
}

/// Response for starting playback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartPlaybackResponse {
    pub status: String,
    pub recording_id: i64,
    pub target_device_udid: String,
    pub total_actions: i32,
    pub message: String,
}

/// Playback status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStatusResponse {
    pub status: String,
    pub recording_id: i64,
    pub target_device_udid: String,
    pub playback_status: String, // "playing", "paused", "stopped", "completed"
    pub current_action_index: i32,
    pub total_actions: i32,
    pub progress_percent: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_type_serde() {
        let tap = ActionType::Tap;
        let json = serde_json::to_string(&tap).unwrap();
        assert_eq!(json, "\"tap\"");

        let parsed: ActionType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ActionType::Tap);
    }

    #[test]
    fn test_action_type_from_str() {
        assert_eq!(ActionType::from_str("tap"), Some(ActionType::Tap));
        assert_eq!(ActionType::from_str("SWIPE"), Some(ActionType::Swipe));
        assert_eq!(ActionType::from_str("Input"), Some(ActionType::Input));
        assert_eq!(ActionType::from_str("keyevent"), Some(ActionType::KeyEvent));
        assert_eq!(ActionType::from_str("unknown"), None);
    }

    #[test]
    fn test_recorded_action_serialization() {
        let action = RecordedAction {
            id: 1,
            recording_id: 100,
            action_type: ActionType::Tap,
            x: Some(100),
            y: Some(200),
            x2: None,
            y2: None,
            duration_ms: None,
            text: None,
            key_code: None,
            sequence_order: 0,
            created_at: 1709827200,
        };

        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["action_type"], "tap");
        assert_eq!(json["x"], 100);
        assert_eq!(json["y"], 200);
        assert!(json.get("x2").is_none()); // skip_serializing_if
    }

    #[test]
    fn test_swipe_action_serialization() {
        let action = RecordedAction {
            id: 2,
            recording_id: 100,
            action_type: ActionType::Swipe,
            x: Some(100),
            y: Some(500),
            x2: Some(100),
            y2: Some(200),
            duration_ms: Some(300),
            text: None,
            key_code: None,
            sequence_order: 1,
            created_at: 1709827201,
        };

        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["action_type"], "swipe");
        assert_eq!(json["x"], 100);
        assert_eq!(json["y"], 500);
        assert_eq!(json["x2"], 100);
        assert_eq!(json["y2"], 200);
        assert_eq!(json["duration_ms"], 300);
    }

    #[test]
    fn test_input_action_serialization() {
        let action = RecordedAction {
            id: 3,
            recording_id: 100,
            action_type: ActionType::Input,
            x: None,
            y: None,
            x2: None,
            y2: None,
            duration_ms: None,
            text: Some("test@example.com".to_string()),
            key_code: None,
            sequence_order: 2,
            created_at: 1709827202,
        };

        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["action_type"], "input");
        assert_eq!(json["text"], "test@example.com");
        assert!(json.get("x").is_none());
    }

    #[test]
    fn test_recording_session_serialization() {
        let session = RecordingSession {
            id: 1,
            name: "Login Test Flow".to_string(),
            device_udid: "device-1".to_string(),
            action_count: 5,
            created_at: 1709827200,
            updated_at: 1709827300,
        };

        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["name"], "Login Test Flow");
        assert_eq!(json["createdAt"], 1709827200);
        assert_eq!(json["updatedAt"], 1709827300);
    }

    #[test]
    fn test_record_action_request() {
        let req: RecordActionRequest = serde_json::from_str(
            r#"{"action_type":"tap","x":100,"y":200}"#
        ).unwrap();
        assert_eq!(req.action_type, ActionType::Tap);
        assert_eq!(req.x, Some(100));
        assert_eq!(req.y, Some(200));
    }
}
