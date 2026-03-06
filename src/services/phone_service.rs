use crate::db::Database;
use serde_json::Value;

/// Device lifecycle management — replaces Python `phone_service_impl.py`.
#[derive(Clone)]
pub struct PhoneService {
    db: Database,
}

#[allow(dead_code)]
impl PhoneService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Called when a device first connects via heartbeat.
    /// Fetches info from atx-agent and upserts to DB.
    pub async fn on_connected(&self, identifier: &str, host: &str) -> Result<(), String> {
        tracing::info!("[PhoneService] on_connected: {} from {}", identifier, host);

        // Try to fetch device info from atx-agent
        let url = format!("http://{}:9008/info", host);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .no_proxy()
            .build()
            .unwrap_or_default();

        let device_data = match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(info) = resp.json::<Value>().await {
                    let mut data = serde_json::json!({
                        "udid": identifier,
                        "ip": host,
                        "port": 9008,
                        "present": true,
                        "ready": true,
                    });
                    // Merge atx-agent info fields
                    if let Some(obj) = info.as_object() {
                        let m = data.as_object_mut().unwrap();
                        if let Some(v) = obj.get("serial") {
                            m.insert("serial".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("brand") {
                            m.insert("brand".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("model") {
                            let model = v.as_str().unwrap_or("");
                            m.insert("model".to_string(), Value::String(model.to_string()));
                        }
                        if let Some(v) = obj.get("productName") {
                            m.insert("model".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("version") {
                            m.insert("version".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("sdk") {
                            m.insert("sdk".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("hwaddr") {
                            m.insert("hwaddr".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("agentVersion") {
                            m.insert("agentVersion".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("display") {
                            m.insert("display".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("battery") {
                            m.insert("battery".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("memory") {
                            m.insert("memory".to_string(), v.clone());
                        }
                        if let Some(v) = obj.get("cpu") {
                            m.insert("cpu".to_string(), v.clone());
                        }
                    }
                    data
                } else {
                    serde_json::json!({
                        "udid": identifier,
                        "ip": host,
                        "port": 9008,
                        "present": true,
                        "ready": false,
                    })
                }
            }
            Err(e) => {
                tracing::warn!("[PhoneService] Failed to fetch device info: {}", e);
                serde_json::json!({
                    "udid": identifier,
                    "ip": host,
                    "port": 9008,
                    "present": true,
                    "ready": false,
                })
            }
        };

        self.db
            .upsert(identifier, &device_data)
            .await
            .map_err(|e| format!("DB upsert failed: {}", e))?;

        Ok(())
    }

    /// Called when a device reconnects from a different IP.
    pub async fn re_connected(&self, identifier: &str, host: &str) -> Result<(), String> {
        tracing::info!("[PhoneService] re_connected: {} from {}", identifier, host);

        let data = serde_json::json!({
            "ip": host,
            "present": true,
        });

        self.db
            .update(identifier, &data)
            .await
            .map_err(|e| format!("DB update failed: {}", e))?;

        Ok(())
    }

    /// Called when a device goes offline (heartbeat timeout).
    pub async fn offline_connected(&self, identifier: &str) -> Result<(), String> {
        tracing::info!("[PhoneService] offline_connected: {}", identifier);

        let data = serde_json::json!({
            "present": false,
        });

        self.db
            .update(identifier, &data)
            .await
            .map_err(|e| format!("DB update failed: {}", e))?;

        Ok(())
    }

    /// Generic field update.
    pub async fn update_field(&self, identifier: &str, item: &Value) -> Result<(), String> {
        self.db
            .upsert(identifier, item)
            .await
            .map_err(|e| format!("DB upsert failed: {}", e))
    }

    /// Query device info by udid.
    pub async fn query_info_by_udid(&self, udid: &str) -> Result<Option<Value>, String> {
        self.db
            .find_by_udid(udid)
            .await
            .map_err(|e| format!("DB query failed: {}", e))
    }

    /// Get all devices.
    pub async fn query_device_list(&self) -> Result<Vec<Value>, String> {
        self.db
            .find_device_list()
            .await
            .map_err(|e| format!("DB query failed: {}", e))
    }

    /// Get online devices only.
    pub async fn query_device_list_by_present(&self) -> Result<Vec<Value>, String> {
        self.db
            .query_device_list_by_present()
            .await
            .map_err(|e| format!("DB query failed: {}", e))
    }

    /// Delete all devices (legacy - use restore_devices for persistence).
    pub async fn delete_devices(&self) -> Result<(), String> {
        self.db
            .delete_all_devices()
            .await
            .map_err(|e| format!("DB delete failed: {}", e))
    }

    /// Restore persisted devices on startup.
    /// Marks ALL devices as offline initially - discovery services will update status.
    /// This enables device state to persist across server restarts.
    pub async fn restore_devices(&self) -> Result<(), String> {
        // Load ALL devices, not just present=true ones
        let devices = self
            .db
            .find_device_list()
            .await
            .map_err(|e| format!("DB query failed: {}", e))?;

        tracing::info!(
            "[PhoneService] Restoring {} persisted devices...",
            devices.len()
        );

        for device in devices {
            if let Some(udid) = device.get("udid").and_then(|v| v.as_str()) {
                // Mark as offline initially - discovery services will reconnect
                let update = serde_json::json!({"present": false});
                self.db
                    .update(udid, &update)
                    .await
                    .map_err(|e| format!("DB update failed for {}: {}", udid, e))?;
            }
        }

        Ok(())
    }

    // ─── Tag Management ───

    /// Add tags to a device. Returns the updated tags list.
    pub async fn add_tags(&self, udid: &str, tags: &[String]) -> Result<Vec<String>, String> {
        // Verify device exists
        self.query_info_by_udid(udid).await?
            .ok_or_else(|| format!("Device not found: {}", udid))?;

        self.db.add_tags(udid, tags).await
    }

    /// Remove a tag from a device. Returns the updated tags list.
    pub async fn remove_tag(&self, udid: &str, tag: &str) -> Result<Vec<String>, String> {
        // Verify device exists
        self.query_info_by_udid(udid).await?
            .ok_or_else(|| format!("Device not found: {}", udid))?;

        self.db.remove_tag(udid, tag).await
    }

    /// Query devices filtered by tag.
    pub async fn query_devices_by_tag(&self, tag: &str) -> Result<Vec<Value>, String> {
        self.db
            .find_devices_by_tag(tag)
            .await
            .map_err(|e| format!("DB query failed: {}", e))
    }
}
