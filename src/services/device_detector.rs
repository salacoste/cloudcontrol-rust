use crate::device::adb::Adb;
use crate::device::atx_init::AtxInit;
use crate::services::phone_service::PhoneService;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Automatic device detection — polls `adb devices` every 1 second.
/// Replaces Python `device_detector.py`.
pub struct DeviceDetector {
    phone_service: PhoneService,
    known_devices: Arc<Mutex<HashSet<String>>>,
    poll_handle: Mutex<Option<JoinHandle<()>>>,
}

#[allow(dead_code)]
impl DeviceDetector {
    pub fn new(phone_service: PhoneService) -> Self {
        Self {
            phone_service,
            known_devices: Arc::new(Mutex::new(HashSet::new())),
            poll_handle: Mutex::new(None),
        }
    }

    /// Get device info from ADB shell commands.
    async fn get_device_info(serial: &str) -> Option<Value> {
        let model = Adb::get_prop(serial, "ro.product.model")
            .await
            .unwrap_or_else(|_| "Unknown".to_string());
        let brand = Adb::get_prop(serial, "ro.product.brand")
            .await
            .unwrap_or_else(|_| "Unknown".to_string());
        let version = Adb::get_prop(serial, "ro.build.version.release")
            .await
            .unwrap_or_else(|_| "Unknown".to_string());
        let sdk_str = Adb::get_prop(serial, "ro.build.version.sdk")
            .await
            .unwrap_or_else(|_| "30".to_string());
        let sdk: i64 = sdk_str.trim().parse().unwrap_or(30);

        let (width, height) = Adb::get_screen_size(serial).await.unwrap_or((1080, 1920));

        // Generate UDID: serial-model (replacing spaces)
        let udid = format!(
            "{}-{}",
            serial,
            model.replace(' ', "_")
        );

        // Determine IP and port for atx-agent connection
        // New uiautomator2 uses port 9008, old atx-agent uses 7912
        let device_port: u16 = 9008;

        let (ip, agent_port): (String, i64) = if Adb::is_usb_serial(serial)
            || serial.starts_with("emulator-")
        {
            // USB or emulator: use adb forward to reach device server
            match Adb::forward(serial, device_port).await {
                Ok(local_port) => {
                    tracing::info!(
                        "[Detector] ADB forward established: 127.0.0.1:{} -> {}:{}",
                        local_port,
                        serial,
                        device_port
                    );
                    ("127.0.0.1".to_string(), local_port as i64)
                }
                Err(e) => {
                    tracing::warn!("[Detector] ADB forward failed for {}: {}", serial, e);
                    // Fallback: try to get WiFi IP
                    let ip = Adb::shell(serial, "ip route | grep 'src' | head -1 | awk '{print $NF}'")
                        .await
                        .unwrap_or_else(|_| "127.0.0.1".to_string());
                    (ip, device_port as i64)
                }
            }
        } else {
            // WiFi device: extract IP from serial (ip:port)
            let ip = serial
                .split(':')
                .next()
                .unwrap_or("127.0.0.1")
                .to_string();
            (ip, device_port as i64)
        };

        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let device_type = Adb::device_type(serial);

        Some(serde_json::json!({
            "udid": udid,
            "serial": serial,
            "ip": ip.trim(),
            "port": agent_port,
            "model": model.trim(),
            "brand": brand.trim(),
            "version": version.trim(),
            "sdk": sdk,
            "display": { "width": width, "height": height },
            "device_type": device_type,
            "present": true,
            "ready": true,
            "using": false,
            "is_server": false,
            "is_mock": false,
            "memory": { "total": 0 },
            "cpu": { "cores": 0 },
            "battery": { "level": 0 },
            "createdAt": now,
            "updatedAt": now,
        }))
    }

    /// Synchronize: detect new and disconnected devices.
    async fn sync_devices(
        phone_service: &PhoneService,
        known_devices: &Arc<Mutex<HashSet<String>>>,
    ) {
        let current = match Adb::list_devices().await {
            Ok(devices) => devices,
            Err(e) => {
                tracing::debug!("[Detector] Failed to list ADB devices: {}", e);
                return;
            }
        };

        let mut known = known_devices.lock().await;

        // Detect new devices
        for serial in &current {
            if !known.contains(serial) {
                tracing::info!("[Detector] New device found: {}", serial);

                // Get device info
                if let Some(info) = Self::get_device_info(serial).await {
                    let udid = info
                        .get("udid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Initialize atx-agent
                    if let Err(e) = AtxInit::init_device(serial).await {
                        tracing::warn!("[Detector] atx-agent init failed for {}: {}", serial, e);
                    }

                    // Register to DB
                    if let Err(e) = phone_service.update_field(&udid, &info).await {
                        tracing::error!("[Detector] Failed to register device {}: {}", serial, e);
                    } else {
                        tracing::info!("[Detector] Device registered: {}", udid);
                    }
                }

                known.insert(serial.clone());
            }
        }

        // Detect disconnected devices
        let disconnected: Vec<String> = known
            .iter()
            .filter(|s| !current.contains(*s))
            .cloned()
            .collect();

        for serial in &disconnected {
            tracing::info!("[Detector] Device disconnected: {}", serial);
            known.remove(serial);

            // We don't have a direct serial→udid mapping here, so we'll just log.
            // The heartbeat mechanism handles offline marking.
        }
    }

    /// Start the detection polling loop.
    pub async fn start(&self) {
        let phone_service = self.phone_service.clone();
        let known_devices = self.known_devices.clone();

        // Initial sync
        Self::sync_devices(&phone_service, &known_devices).await;

        // Start background polling
        let ps = phone_service.clone();
        let kd = known_devices.clone();
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                Self::sync_devices(&ps, &kd).await;
            }
        });

        let mut h = self.poll_handle.lock().await;
        *h = Some(handle);
        tracing::info!("[Detector] Device detection started (1s polling)");
    }

    /// Stop the detection loop.
    pub async fn stop(&self) {
        let mut h = self.poll_handle.lock().await;
        if let Some(handle) = h.take() {
            handle.abort();
        }
        tracing::info!("[Detector] Device detection stopped");
    }
}
