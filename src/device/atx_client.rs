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
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(3))
            .no_proxy()
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
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(3))
            .no_proxy()
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
                // u2 returns MIME-style base64 with \n line breaks — strip them
                let b64_clean = b64_data.replace('\n', "").replace('\r', "");
                return base64::engine::general_purpose::STANDARD
                    .decode(&b64_clean)
                    .map_err(|e| format!("Failed to decode screenshot base64: {}", e));
            }
        }

        // Fallback: try GET /screenshot/0 (old atx-agent)
        let url = format!("{}/screenshot/0", self.base_url);
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Screenshot request failed: {}", e))?;

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read screenshot bytes: {}", e))?;

        Ok(bytes.to_vec())
    }

    /// Take screenshot with custom scale and quality — device does all processing.
    /// Params: scale (0.0-1.0), quality (0-100).
    /// Returns raw JPEG bytes (base64-decoded from u2 response).
    /// This is the FASTEST path: no server-side PNG decode / resize / JPEG encode.
    pub async fn screenshot_scaled(&self, scale: f64, quality: u8) -> Result<Vec<u8>, String> {
        let t0 = std::time::Instant::now();

        let result = self
            .jsonrpc(
                "takeScreenshot",
                vec![serde_json::json!(scale), serde_json::json!(quality)],
            )
            .await;

        let t_rpc = t0.elapsed();

        if let Ok(Value::String(b64_data)) = result {
            if !b64_data.is_empty() {
                let t1 = std::time::Instant::now();
                // u2 returns MIME-style base64 with \n line breaks — strip them
                let b64_clean = b64_data.replace('\n', "").replace('\r', "");
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&b64_clean)
                    .map_err(|e| format!("Failed to decode screenshot base64: {}", e))?;
                let t_decode = t1.elapsed();

                tracing::info!(
                    "[Screenshot] u2 s={:.1} q={} | rpc={:.0}ms | b64_decode={:.0}ms ({}KB b64→{}KB jpg) | total={:.0}ms",
                    scale, quality,
                    t_rpc.as_secs_f64() * 1000.0,
                    t_decode.as_secs_f64() * 1000.0,
                    b64_data.len() / 1024,
                    bytes.len() / 1024,
                    t0.elapsed().as_secs_f64() * 1000.0,
                );

                return Ok(bytes);
            }
        }

        Err("takeScreenshot returned no data".to_string())
    }

    /// Take screenshot and return base64 string directly from u2 server.
    /// Avoids the decode→recompress→re-encode cycle — much faster for streaming.
    pub async fn screenshot_base64_direct(&self) -> Result<String, String> {
        let result = self
            .jsonrpc("takeScreenshot", vec![serde_json::json!(1), serde_json::json!(80)])
            .await;

        if let Ok(Value::String(b64_data)) = result {
            if !b64_data.is_empty() {
                return Ok(b64_data);
            }
        }

        Err("takeScreenshot returned no data".to_string())
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

    /// Device info — tries GET /info first, falls back to JSON-RPC deviceInfo.
    pub async fn device_info(&self) -> Result<Value, String> {
        // Try GET /info (old atx-agent)
        let url = format!("{}/info", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<Value>().await {
                    Ok(json) => return Ok(json),
                    Err(e) => tracing::debug!("[ATX] GET /info JSON parse failed for {}: {}", self.udid, e),
                }
            }
            Ok(resp) => tracing::debug!("[ATX] GET /info returned {} for {}", resp.status(), self.udid),
            Err(e) => tracing::debug!("[ATX] GET /info request failed for {}: {}", self.udid, e),
        }

        // Fallback: JSON-RPC deviceInfo (new u2.jar)
        let result = self.jsonrpc("deviceInfo", vec![]).await?;
        Ok(result)
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
