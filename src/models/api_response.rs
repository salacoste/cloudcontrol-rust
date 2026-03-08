use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Standardized API response wrapper for all /api/v1/* endpoints
/// Follows NFR20 error response standardization

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            status: "success".to_string(),
            data: Some(data),
            error: None,
            message: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(error_code: &str, message: &str) -> Self {
        Self {
            status: "error".to_string(),
            data: None,
            error: Some(error_code.to_string()),
            message: Some(message.to_string()),
            timestamp: Utc::now(),
        }
    }
}

/// Error codes for API responses (NFR20)
pub const ERR_DEVICE_NOT_FOUND: &str = "ERR_DEVICE_NOT_FOUND";
pub const ERR_DEVICE_DISCONNECTED: &str = "ERR_DEVICE_DISCONNECTED";
pub const ERR_INVALID_REQUEST: &str = "ERR_INVALID_REQUEST";
pub const ERR_OPERATION_FAILED: &str = "ERR_OPERATION_FAILED";
pub const ERR_NO_DEVICES_SELECTED: &str = "ERR_NO_DEVICES_SELECTED";
pub const ERR_BATCH_PARTIAL_FAILURE: &str = "ERR_BATCH_PARTIAL_FAILURE";

/// Request/Response types for API v1

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapRequest {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwipeRequest {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
    #[serde(default = "default_duration")]
    pub duration: f64,
}

fn default_duration() -> f64 {
    0.3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRequest {
    pub text: String,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEventRequest {
    pub key: String,
}

/// Alias for backward compatibility
pub type KeyeventRequest = KeyEventRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchTapRequest {
    pub udids: Vec<String>,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSwipeRequest {
    pub udids: Vec<String>,
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
    #[serde(default = "default_duration")]
    pub duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInputRequest {
    pub udids: Vec<String>,
    pub text: String,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub udid: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse {
    pub status: String,
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub results: Vec<BatchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotQuery {
    #[serde(default)]
    pub format: Option<String>,  // "jpeg" or "png"
    #[serde(default)]
    pub quality: Option<u8>,  // 1-100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub udid: String,
    pub model: String,
    #[serde(rename = "androidVersion")]
    pub android_version: String,
    pub battery: i32,
    pub display: DisplayInfo,
    pub serial: String,
    pub ip: String,
    pub port: i64,
    pub status: String,
    #[serde(rename = "lastSeen")]
    pub last_seen: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
}

/// Response type for single device info endpoint
pub type DeviceInfoResponse = DeviceInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotResponse {
    #[serde(rename = "type")]
    pub image_type: String,
    pub encoding: String,
    pub data: String,
}
