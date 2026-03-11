use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Provider {
    pub id: i64,
    pub ip: String,
    pub notes: Option<String>,
    pub present: bool,
    #[serde(rename = "presenceChangedAt")]
    pub presence_changed_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderWithDevices {
    #[serde(flatten)]
    pub provider: Provider,
    pub device_count: i64,
    pub devices: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub ip: String,
    pub notes: Option<String>,
}
