use crate::device::adb::Adb;
use std::time::Duration;
use tokio::process::Command;
use std::process::Stdio;

/// Initialize UiAutomator2 server on a device.
/// Replaces the old atx-agent approach with the new u2.jar Java server.
pub struct AtxInit;

#[allow(dead_code)]
impl AtxInit {
    /// Full device initialization:
    /// 1. Check if u2.jar exists on device (push if not)
    /// 2. Start the UiAutomator2 Java server in background
    /// 3. Verify readiness on port 9008
    pub async fn init_device(serial: &str) -> Result<(), String> {
        tracing::info!("[ATX] Initializing UiAutomator2 server on {}...", serial);

        // Set up a single forward for checking and communication
        let local_port = Adb::forward(serial, 9008).await?;

        // Check if u2 server is already running
        if Self::check_port_alive(local_port).await {
            tracing::info!("[ATX] UiAutomator2 server already running on {}", serial);
            return Ok(());
        }

        // Check if u2.jar exists on device
        let check_jar = Adb::shell(serial, "ls /data/local/tmp/u2.jar")
            .await
            .unwrap_or_default();
        if !check_jar.contains("u2.jar") || check_jar.contains("No such file") {
            tracing::warn!(
                "[ATX] u2.jar not found on {}. Run 'python3 -m uiautomator2 init --serial {}' first.",
                serial, serial
            );
            return Err(format!("u2.jar not found on device {}. Please run 'python3 -m uiautomator2 init' first.", serial));
        }

        // Start UiAutomator2 Java server in background
        // Command: CLASSPATH=/data/local/tmp/u2.jar app_process / com.wetest.uia2.Main
        tracing::info!("[ATX] Starting UiAutomator2 server on {}...", serial);

        let _child = Command::new("adb")
            .args([
                "-s",
                serial,
                "shell",
                "CLASSPATH=/data/local/tmp/u2.jar app_process / com.wetest.uia2.Main",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start u2 server: {}", e))?;

        // Wait for server to be ready (up to 15 seconds), reusing the same forwarded port
        for i in 0..30 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if Self::check_port_alive(local_port).await {
                tracing::info!(
                    "[ATX] UiAutomator2 server ready on {} (took ~{}ms, forward port {})",
                    serial,
                    (i + 1) * 500,
                    local_port
                );
                return Ok(());
            }
        }

        tracing::warn!("[ATX] UiAutomator2 server did not become ready on {} within 15s", serial);
        Err("UiAutomator2 server did not start in time".to_string())
    }

    /// Check if the u2 server is reachable on a specific local forwarded port.
    async fn check_port_alive(local_port: u16) -> bool {
        let url = format!("http://127.0.0.1:{}/jsonrpc/0", local_port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(800))
            .connect_timeout(Duration::from_millis(500))
            .no_proxy()
            .build()
            .unwrap_or_default();

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "deviceInfo",
            "params": [],
        });

        match client.post(&url).json(&body).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Verify server readiness via a direct URL (for already-forwarded connections).
    pub async fn verify_ready(_ip: &str, port: i64) -> bool {
        Self::check_port_alive(port as u16).await
    }
}
