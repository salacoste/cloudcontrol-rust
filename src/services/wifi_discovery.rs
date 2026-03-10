//! WiFi Device Auto-Discovery Service
//!
//! Automatically discovers Android devices running ATX Agent on the local network.
//! Scans configurable ports (default: 7912, 9008).

use crate::services::phone_service::PhoneService;
use crate::utils::host_ip::{get_primary_subnet, get_local_subnets};
use chrono::Utc;
use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Default ports to scan for ATX Agent
const DEFAULT_ATX_PORTS: &[u16] = &[7912, 9008];

/// HTTP timeout for device probing
const DEFAULT_PROBE_TIMEOUT_MS: u64 = 500;

/// Maximum concurrent probes
const DEFAULT_MAX_CONCURRENT_PROBES: usize = 50;

/// Default scan interval in seconds
const DEFAULT_SCAN_INTERVAL_SECS: u64 = 30;

/// Default retry count before marking device offline
const DEFAULT_OFFLINE_RETRY_COUNT: u8 = 3;

/// Configuration for WiFi discovery service
#[derive(Debug, Clone, Deserialize)]
pub struct WifiDiscoveryConfig {
    /// Ports to scan for ATX Agent
    #[serde(default = "default_atx_ports")]
    pub ports: Vec<u16>,
    /// HTTP timeout for device probing in milliseconds
    #[serde(default = "default_probe_timeout")]
    pub probe_timeout_ms: u64,
    /// Maximum concurrent probes
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_probes: usize,
    /// Scan interval in seconds
    #[serde(default = "default_scan_interval")]
    pub scan_interval_secs: u64,
    /// Retry count before marking device offline
    #[serde(default = "default_offline_retry_count")]
    pub offline_retry_count: u8,
}

fn default_atx_ports() -> Vec<u16> {
    DEFAULT_ATX_PORTS.to_vec()
}

fn default_probe_timeout() -> u64 {
    DEFAULT_PROBE_TIMEOUT_MS
}

fn default_max_concurrent() -> usize {
    DEFAULT_MAX_CONCURRENT_PROBES
}

fn default_scan_interval() -> u64 {
    DEFAULT_SCAN_INTERVAL_SECS
}

fn default_offline_retry_count() -> u8 {
    DEFAULT_OFFLINE_RETRY_COUNT
}

impl Default for WifiDiscoveryConfig {
    fn default() -> Self {
        Self {
            ports: default_atx_ports(),
            probe_timeout_ms: default_probe_timeout(),
            max_concurrent_probes: default_max_concurrent(),
            scan_interval_secs: default_scan_interval(),
            offline_retry_count: default_offline_retry_count(),
        }
    }
}

/// Device tracking entry for offline detection
struct DeviceEntry {
    udid: String,
    missed_count: u8,
}

/// WiFi device auto-discovery service.
pub struct WifiDiscovery {
    phone_service: PhoneService,
    /// Maps "ip:port" -> DeviceEntry for known discovered devices
    known_devices: Arc<Mutex<HashMap<String, DeviceEntry>>>,
    /// Background polling task handle
    poll_handle: Mutex<Option<JoinHandle<()>>>,
    /// Cancellation token for graceful shutdown
    cancel_token: CancellationToken,
    /// HTTP client for probing (shared, no cloning needed)
    client: Arc<Client>,
    /// Configuration
    config: WifiDiscoveryConfig,
    /// Host IP for subnet determination
    host_ip: String,
    /// Detected subnets to scan
    subnets: Vec<String>,
}

impl WifiDiscovery {
    /// Create a new WiFi discovery service with automatic subnet detection.
    pub fn new(phone_service: PhoneService) -> Self {
        Self::with_config(phone_service, WifiDiscoveryConfig::default())
    }

    /// Create a new WiFi discovery service with custom configuration.
    pub fn with_config(phone_service: PhoneService, config: WifiDiscoveryConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.probe_timeout_ms))
            .connect_timeout(Duration::from_millis(config.probe_timeout_ms))
            .no_proxy()
            .build()
            .unwrap_or_default();

        // Auto-detect subnets from network interfaces
        let subnets = get_local_subnets();
        let host_ip = crate::utils::host_ip::get_host_ip();

        tracing::info!(
            "[WifiDiscovery] Auto-detected subnets: {:?}, host IP: {}",
            subnets,
            host_ip
        );

        Self {
            phone_service,
            known_devices: Arc::new(Mutex::new(HashMap::new())),
            poll_handle: Mutex::new(None),
            cancel_token: CancellationToken::new(),
            client: Arc::new(client),
            config,
            host_ip,
            subnets,
        }
    }

    /// Create a new WiFi discovery service with explicit host IP (for testing).
    pub fn with_host_ip(phone_service: PhoneService, host_ip: &str) -> Self {
        let mut instance = Self::new(phone_service);
        instance.host_ip = host_ip.to_string();
        instance.subnets = vec![Self::parse_subnet(host_ip)
            .unwrap_or_else(|| get_primary_subnet())];
        instance
    }

    /// Parse subnet from host IP (e.g., "192.168.1.100" -> "192.168.1")
    fn parse_subnet(ip: &str) -> Option<String> {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            Some(format!("{}.{}.{}", parts[0], parts[1], parts[2]))
        } else {
            None
        }
    }

    /// Probe a single IP:port for ATX Agent.
    /// Tries GET /info first (old atx-agent), then JSON-RPC deviceInfo (new u2.jar).
    async fn probe_device(client: &Client, ip: &str, port: u16) -> Option<(Value, u16)> {
        let base = format!("http://{}:{}", ip, port);

        // Try 1: GET /info (old atx-agent on port 7912)
        if let Ok(resp) = client.get(&format!("{}/info", base)).send().await {
            if resp.status().is_success() {
                if let Ok(info) = resp.json::<Value>().await {
                    if info.get("serial").is_some()
                        || info.get("brand").is_some()
                        || info.get("model").is_some()
                    {
                        return Some((info, port));
                    }
                }
            }
        }

        // Try 2: JSON-RPC deviceInfo (new u2.jar on port 9008)
        let rpc_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "deviceInfo",
            "params": []
        });
        if let Ok(resp) = client
            .post(&format!("{}/jsonrpc/0", base))
            .json(&rpc_body)
            .send()
            .await
        {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(result) = json.get("result") {
                    // Map JSON-RPC deviceInfo fields to expected format
                    let model = result
                        .get("productName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let sdk = result.get("sdkInt").and_then(|v| v.as_i64()).unwrap_or(30);
                    let width = result
                        .get("displayWidth")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1080);
                    let height = result
                        .get("displayHeight")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1920);

                    let serial = result
                        .get("serial")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let info = serde_json::json!({
                        "model": model,
                        "sdk": sdk,
                        "serial": serial,
                        "display": { "width": width, "height": height },
                    });
                    return Some((info, port));
                }
            }
        }

        None
    }

    /// Build device data from ATX agent info.
    fn build_device_data(info: &Value, ip: &str, port: u16) -> Value {
        let serial = info
            .get("serial")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let model = info
            .get("model")
            .or_else(|| info.get("productName"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        // Generate UDID matching existing pattern: {serial}-{model}
        let udid = format!("{}-{}", serial, model.replace(' ', "_"));

        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        json!({
            "udid": udid,
            "serial": serial,
            "ip": ip.trim(),
            "port": port,
            "model": model.trim(),
            "brand": info.get("brand").and_then(|v| v.as_str()).unwrap_or("Unknown"),
            "version": info.get("version").and_then(|v| v.as_str()).unwrap_or("Unknown"),
            "sdk": info.get("sdk").and_then(|v| v.as_i64()).unwrap_or(30),
            "agentVersion": info.get("agentVersion").and_then(|v| v.as_str()).unwrap_or(""),
            "hwaddr": info.get("hwaddr").and_then(|v| v.as_str()).unwrap_or(""),
            "display": info.get("display").cloned().unwrap_or_else(|| json!({"width": 1080, "height": 1920})),
            "battery": info.get("battery").cloned().unwrap_or_else(|| json!({"level": 0})),
            "memory": info.get("memory").cloned().unwrap_or_else(|| json!({"total": 0})),
            "cpu": info.get("cpu").cloned().unwrap_or_else(|| json!({"cores": 0})),
            "device_type": "wifi",
            "present": true,
            "ready": true,
            "using": false,
            "is_server": false,
            "is_mock": false,
            "createdAt": now,
            "updatedAt": now,
        })
    }

    /// Scan a single IP address for ATX agents on all configured ports.
    async fn scan_ip(client: &Client, ip: &str, ports: &[u16]) -> Option<(Value, u16)> {
        // Try each port sequentially (most devices use one port)
        for &port in ports {
            if let Some(result) = Self::probe_device(client, ip, port).await {
                return Some(result);
            }
        }
        None
    }

    /// Scan the entire subnet for ATX Agent devices using concurrent scanning.
    pub async fn scan_subnet(&self) -> Vec<(String, Value, u16)> {
        let subnet = match self.subnets.first() {
            Some(s) => s.clone(),
            None => {
                // Fallback to parsing from host_ip
                match Self::parse_subnet(&self.host_ip) {
                    Some(s) => s,
                    None => {
                        tracing::warn!("[WifiDiscovery] Could not determine subnet from host IP: {}", self.host_ip);
                        return vec![];
                    }
                }
            }
        };

        tracing::info!("[WifiDiscovery] Starting subnet scan: {}.1-254", subnet);

        // Create a list of IPs to scan
        let ips: Vec<String> = (1..=254)
            .map(|i| format!("{}.{}", subnet, i))
            .filter(|ip| ip != &self.host_ip) // Skip our own IP
            .collect();

        let client = self.client.clone();
        let ports = self.config.ports.clone();
        let max_concurrent = self.config.max_concurrent_probes;

        // Process in batches to limit concurrent connections
        let mut discovered = Vec::new();
        let mut batch = Vec::new();

        for ip in ips {
            let client = client.clone();
            let ports = ports.clone();
            let ip = ip;

            batch.push(async move {
                if let Some((info, port)) = Self::scan_ip(&client, &ip, &ports).await {
                    Some((ip, info, port))
                } else {
                    None
                }
            });

            // Process batch when full
            if batch.len() >= max_concurrent {
                let results: Vec<Option<(String, Value, u16)>> = join_all(batch.drain(..)).await;
                for result in results.into_iter().flatten() {
                    discovered.push(result);
                }
            }
        }

        // Process remaining
        if !batch.is_empty() {
            let results: Vec<Option<(String, Value, u16)>> = join_all(batch).await;
            for result in results.into_iter().flatten() {
                discovered.push(result);
            }
        }

        tracing::info!("[WifiDiscovery] Scan complete: {} devices found", discovered.len());
        discovered
    }

    /// Synchronize discovered devices with database.
    async fn sync_devices(
        phone_service: &PhoneService,
        known_devices: &Arc<Mutex<HashMap<String, DeviceEntry>>>,
        discovered: Vec<(String, Value, u16)>,
        offline_retry_count: u8,
    ) {
        let mut known = known_devices.lock().await;

        // Build set of current addresses
        let current_addresses: std::collections::HashSet<String> = discovered
            .iter()
            .map(|(ip, _, port)| format!("{}:{}", ip, port))
            .collect();

        // Register new devices or reset missed count for existing ones
        for (ip, info, port) in &discovered {
            let addr = format!("{}:{}", ip, port);

            if let Some(entry) = known.get_mut(&addr) {
                // Device still present, reset missed count
                entry.missed_count = 0;
            } else {
                // New device
                let udid = info
                    .get("udid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                tracing::info!("[WifiDiscovery] New device discovered: {} at {}:{}", udid, ip, port);

                if let Err(e) = phone_service.update_field(&udid, info).await {
                    tracing::error!("[WifiDiscovery] Failed to register device {}: {}", udid, e);
                } else {
                    known.insert(addr, DeviceEntry {
                        udid,
                        missed_count: 0,
                    });
                }
            }
        }

        // Increment missed count for offline devices
        let offline: Vec<(String, String, u8)> = known
            .iter()
            .filter(|(addr, _)| !current_addresses.contains(*addr))
            .map(|(a, e)| (a.clone(), e.udid.clone(), e.missed_count))
            .collect();

        for (addr, udid, missed) in offline {
            let entry = known.get_mut(&addr).unwrap();
            entry.missed_count = missed + 1;

            // Only mark offline after configured retry count
            if entry.missed_count >= offline_retry_count {
                tracing::info!(
                    "[WifiDiscovery] Device offline after {} misses: {} ({})",
                    entry.missed_count,
                    udid,
                    addr
                );
                known.remove(&addr);

                if let Err(e) = phone_service.offline_connected(&udid).await {
                    tracing::error!("[WifiDiscovery] Failed to mark device offline {}: {}", udid, e);
                }
            } else {
                tracing::debug!(
                    "[WifiDiscovery] Device missed scan {}/{}: {} ({})",
                    entry.missed_count,
                    offline_retry_count,
                    udid,
                    addr
                );
            }
        }
    }

    /// Start the background discovery polling loop.
    pub async fn start(&self) {
        // Initial scan
        match self.scan_subnet().await {
            discovered => {
                Self::sync_devices(
                    &self.phone_service,
                    &self.known_devices,
                    discovered,
                    self.config.offline_retry_count,
                )
                .await;
            }
        }

        // Start background polling
        let phone_service = self.phone_service.clone();
        let known_devices = self.known_devices.clone();
        let scan_interval = self.config.scan_interval_secs;
        let offline_retry_count = self.config.offline_retry_count;
        let client = self.client.clone();
        let ports = self.config.ports.clone();
        let host_ip = self.host_ip.clone();
        let subnets = self.subnets.clone();
        let max_concurrent = self.config.max_concurrent_probes;
        let cancel_token = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            // Create scanner state for background task
            let subnet = subnets.first().cloned().unwrap_or_else(|| {
                let parts: Vec<&str> = host_ip.split('.').collect();
                if parts.len() == 4 {
                    format!("{}.{}.{}", parts[0], parts[1], parts[2])
                } else {
                    "192.168.1".to_string()
                }
            });

            loop {
                // Use tokio::select for graceful shutdown
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        tracing::info!("[WifiDiscovery] Shutdown signal received, stopping background task");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(scan_interval)) => {
                        // Perform scan with error logging
                        tracing::debug!("[WifiDiscovery] Starting periodic scan");

                        // Create scan task
                        let scan_result = async {
                            let ips: Vec<String> = (1..=254)
                                .map(|i| format!("{}.{}", subnet, i))
                                .filter(|ip| ip != &host_ip)
                                .collect();

                            let mut discovered = Vec::new();
                            let mut batch = Vec::new();

                            for ip in ips {
                                let client = client.clone();
                                let ports = ports.clone();
                                let ip = ip;

                                batch.push(async move {
                                    if let Some((info, port)) = Self::scan_ip(&client, &ip, &ports).await {
                                        Some((ip, info, port))
                                    } else {
                                        None
                                    }
                                });

                                if batch.len() >= max_concurrent {
                                    let results: Vec<Option<(String, Value, u16)>> = join_all(batch.drain(..)).await;
                                    for result in results.into_iter().flatten() {
                                        discovered.push(result);
                                    }
                                }
                            }

                            if !batch.is_empty() {
                                let results: Vec<Option<(String, Value, u16)>> = join_all(batch).await;
                                for result in results.into_iter().flatten() {
                                    discovered.push(result);
                                }
                            }

                            Ok::<_, String>(discovered)
                        };

                        match scan_result.await {
                            Ok(discovered) => {
                                Self::sync_devices(
                                    &phone_service,
                                    &known_devices,
                                    discovered,
                                    offline_retry_count,
                                ).await;
                            }
                            Err(e) => {
                                tracing::error!("[WifiDiscovery] Scan failed: {}", e);
                            }
                        }
                    }
                }
            }
        });

        let mut h = self.poll_handle.lock().await;
        *h = Some(handle);

        tracing::info!(
            "[WifiDiscovery] Started (scanning every {}s on ports {:?})",
            self.config.scan_interval_secs,
            self.config.ports
        );
    }

    /// Stop the background discovery loop gracefully.
    pub async fn stop(&self) {
        // Signal cancellation
        self.cancel_token.cancel();

        // Wait for task to complete
        let mut h = self.poll_handle.lock().await;
        if let Some(handle) = h.take() {
            // Give it a moment to finish gracefully
            match tokio::time::timeout(Duration::from_secs(5), handle).await {
                Ok(_) => {
                    tracing::info!("[WifiDiscovery] Background task stopped gracefully");
                }
                Err(_) => {
                    tracing::warn!("[WifiDiscovery] Background task did not stop in time, aborting");
                }
            }
        }
        tracing::info!("[WifiDiscovery] Stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_subnet() {
        assert_eq!(
            WifiDiscovery::parse_subnet("192.168.1.100"),
            Some("192.168.1".to_string())
        );
        assert_eq!(
            WifiDiscovery::parse_subnet("10.0.0.1"),
            Some("10.0.0".to_string())
        );
        assert_eq!(
            WifiDiscovery::parse_subnet("172.16.0.50"),
            Some("172.16.0".to_string())
        );
        assert_eq!(WifiDiscovery::parse_subnet("invalid"), None);
        assert_eq!(WifiDiscovery::parse_subnet("192.168.1"), None);
    }

    #[test]
    fn test_build_device_data() {
        let info = serde_json::json!({
            "serial": "abc123",
            "model": "Galaxy S21",
            "brand": "Samsung",
            "version": "12",
            "sdk": 31,
            "agentVersion": "2.0.0",
            "display": {"width": 1080, "height": 2400},
            "battery": {"level": 85},
        });

        let data = WifiDiscovery::build_device_data(&info, "192.168.1.50", 9008);

        assert_eq!(data["udid"], "abc123-Galaxy_S21");
        assert_eq!(data["serial"], "abc123");
        assert_eq!(data["ip"], "192.168.1.50");
        assert_eq!(data["port"], 9008);
        assert_eq!(data["model"], "Galaxy S21");
        assert_eq!(data["brand"], "Samsung");
        assert_eq!(data["version"], "12");
        assert_eq!(data["sdk"], 31);
        assert_eq!(data["device_type"], "wifi");
        assert_eq!(data["present"], true);
        assert_eq!(data["ready"], true);
    }

    #[test]
    fn test_build_device_data_with_minimal_info() {
        let info = serde_json::json!({
            "serial": "test123",
        });

        let data = WifiDiscovery::build_device_data(&info, "10.0.0.1", 7912);

        assert_eq!(data["udid"], "test123-Unknown");
        assert_eq!(data["port"], 7912);
        assert_eq!(data["brand"], "Unknown");
        assert_eq!(data["device_type"], "wifi");
        assert_eq!(data["present"], true);
    }

    #[test]
    fn test_build_device_data_with_product_name() {
        // Some devices use productName instead of model
        let info = serde_json::json!({
            "serial": "xyz789",
            "productName": "Pixel 6 Pro",
        });

        let data = WifiDiscovery::build_device_data(&info, "192.168.1.100", 9008);

        assert_eq!(data["model"], "Pixel 6 Pro");
        assert_eq!(data["udid"], "xyz789-Pixel_6_Pro");
    }

    #[test]
    fn test_default_config() {
        let config = WifiDiscoveryConfig::default();
        assert!(config.ports.contains(&7912));
        assert!(config.ports.contains(&9008));
        assert_eq!(config.probe_timeout_ms, 500);
        assert_eq!(config.max_concurrent_probes, 50);
        assert_eq!(config.scan_interval_secs, 30);
        assert_eq!(config.offline_retry_count, 3);
    }

    #[test]
    fn test_config_defaults() {
        assert_eq!(default_atx_ports(), vec![7912, 9008]);
        assert_eq!(default_probe_timeout(), 500);
        assert_eq!(default_max_concurrent(), 50);
        assert_eq!(default_scan_interval(), 30);
        assert_eq!(default_offline_retry_count(), 3);
    }
}
