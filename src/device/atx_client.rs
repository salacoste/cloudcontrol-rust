use base64::Engine;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

/// HTTP client for communicating with atx-agent on Android devices.
/// Replaces the entire uiautomator2 Python library.
#[derive(Clone)]
pub struct AtxClient {
    client: Client,
    base_url: String,
    pub udid: String,
}

impl AtxClient {
    pub fn new(ip: &str, port: i64, udid: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: format!("http://{}:{}", ip, port),
            udid: udid.to_string(),
        }
    }

    pub fn from_url(url: &str, udid: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: url.trim_end_matches('/').to_string(),
            udid: udid.to_string(),
        }
    }

    /// Take screenshot via JSON-RPC `takeScreenshot` → returns JPEG bytes.
    /// Compatible with both new uiautomator2 (port 9008) and old atx-agent (port 7912).
    pub async fn screenshot(&self) -> Result<Vec<u8>, String> {
        // Try JSON-RPC takeScreenshot first (new uiautomator2 on port 9008)
        let result = self
            .jsonrpc("takeScreenshot", vec![serde_json::json!(1), serde_json::json!(80)])
            .await;

        if let Ok(Value::String(b64_data)) = result {
            if !b64_data.is_empty() {
                return base64::engine::general_purpose::STANDARD
                    .decode(&b64_data)
                    .map_err(|e| format!("Failed to decode screenshot base64: {}", e));
            }
        }

        // Fallback: try GET /screenshot/0 (old atx-agent)
        let url = format!("{}/screenshot/0", self.base_url);
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("Screenshot request failed: {}", e))?;

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read screenshot bytes: {}", e))?;

        Ok(bytes.to_vec())
    }

    /// JSON-RPC call helper to POST /jsonrpc/0
    async fn jsonrpc(&self, method: &str, params: Vec<Value>) -> Result<Value, String> {
        let url = format!("{}/jsonrpc/0", self.base_url);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("JSON-RPC {} failed: {}", method, e))?;

        let json: Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse JSON-RPC response: {}", e))?;

        if let Some(err) = json.get("error") {
            return Err(format!("JSON-RPC error: {}", err));
        }

        Ok(json.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Click at (x, y)
    pub async fn click(&self, x: i32, y: i32) -> Result<(), String> {
        self.jsonrpc(
            "click",
            vec![serde_json::json!(x), serde_json::json!(y)],
        )
        .await?;
        Ok(())
    }

    /// Swipe from (x1,y1) to (x2,y2) with duration in seconds
    pub async fn swipe(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        duration: f64,
    ) -> Result<(), String> {
        self.jsonrpc(
            "swipe",
            vec![
                serde_json::json!(x1),
                serde_json::json!(y1),
                serde_json::json!(x2),
                serde_json::json!(y2),
                serde_json::json!(duration),
            ],
        )
        .await?;
        Ok(())
    }

    /// Press a key (e.g. "home", "back", "enter")
    pub async fn press_key(&self, key: &str) -> Result<(), String> {
        self.jsonrpc("pressKey", vec![serde_json::json!(key)])
            .await?;
        Ok(())
    }

    /// Set the FastInputIME and send text
    pub async fn input_text(&self, text: &str) -> Result<(), String> {
        // Enable fast input IME
        self.jsonrpc("setFastInputIME", vec![serde_json::json!(true)])
            .await
            .ok();
        // Send keys
        self.jsonrpc("sendKeys", vec![serde_json::json!(text), serde_json::json!(false)])
            .await?;
        Ok(())
    }

    /// Dump UI hierarchy XML via JSON-RPC
    pub async fn dump_hierarchy(&self) -> Result<String, String> {
        let result = self
            .jsonrpc(
                "dumpWindowHierarchy",
                vec![serde_json::json!(false), serde_json::json!(false)],
            )
            .await?;

        match result {
            Value::String(s) => Ok(s),
            _ => Ok(result.to_string()),
        }
    }

    /// GET /info → device info JSON
    pub async fn device_info(&self) -> Result<Value, String> {
        let url = format!("{}/info", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Device info request failed: {}", e))?;

        let json: Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse device info: {}", e))?;

        Ok(json)
    }

    /// GET /shell?command=<cmd> → execute shell on device
    pub async fn shell_cmd(&self, cmd: &str) -> Result<String, String> {
        let url = format!("{}/shell", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[("command", cmd)])
            .send()
            .await
            .map_err(|e| format!("Shell command failed: {}", e))?;

        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read shell response: {}", e))?;

        Ok(text)
    }

    /// Push file to device via POST /upload/{path} (multipart)
    pub async fn push_file(&self, remote_path: &str, data: Vec<u8>, filename: &str) -> Result<(), String> {
        let url = format!("{}/upload{}", self.base_url, remote_path);
        let part = reqwest::multipart::Part::bytes(data)
            .file_name(filename.to_string())
            .mime_str("application/octet-stream")
            .map_err(|e| format!("Failed to create multipart: {}", e))?;

        let form = reqwest::multipart::Form::new().part("file", part);

        self.client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Push file failed: {}", e))?;

        Ok(())
    }

    /// Window size via device info
    pub async fn window_size(&self) -> Result<(i64, i64), String> {
        let info = self.device_info().await?;
        let display = info.get("display");
        if let Some(d) = display {
            let w = d.get("width").and_then(|v| v.as_i64()).unwrap_or(1080);
            let h = d.get("height").and_then(|v| v.as_i64()).unwrap_or(1920);
            return Ok((w, h));
        }
        Ok((1080, 1920))
    }
}
