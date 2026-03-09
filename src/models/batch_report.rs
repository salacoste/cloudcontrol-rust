use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a single device in a batch operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceOperationStatus {
    Success,
    Failed,
    Skipped,
    Timeout,
}

/// Result of a single device in a batch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeviceOperationResult {
    pub udid: String,
    pub status: DeviceOperationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>, // Base64 encoded
}

/// Type of batch operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BatchOperationType {
    Tap,
    Swipe,
    Input,
    Screenshot,
}

/// A complete batch operation report
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BatchReport {
    pub id: i64,
    pub operation_type: BatchOperationType,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_devices: usize,
    pub successful: usize,
    pub failed: usize,
    pub results: Vec<DeviceOperationResult>,
}

impl BatchReport {
    /// Calculate overall status of the batch operation
    pub fn overall_status(&self) -> &'static str {
        if self.failed == 0 {
            "success"
        } else if self.successful == 0 {
            "failed"
        } else {
            "partial"
        }
    }

    /// Calculate total duration in milliseconds
    pub fn total_duration_ms(&self) -> i64 {
        if let Some(completed) = self.completed_at {
            (completed - self.created_at).num_milliseconds()
        } else {
            0
        }
    }
}
