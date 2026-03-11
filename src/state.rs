use crate::config::AppConfig;
use crate::db::Database;
use crate::pool::connection_pool::ConnectionPool;
use crate::pool::screenshot_cache::ScreenshotCache;
use crate::services::phone_service::PhoneService;
use crate::services::recording_service::RecordingService;
use crate::services::scrcpy_manager::ScrcpyManager;
use crate::services::video_service::VideoService;
use dashmap::DashMap;
use moka::future::Cache;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

/// Heartbeat session entry
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HeartbeatSession {
    pub identifier: String,
    pub remote_host: String,
    /// Expiry time as unix timestamp
    pub timer: f64,
}

/// Metrics tracker for screenshot latency and connection counts (Story 5-3, Story 12-6)
#[derive(Debug)]
pub struct MetricsTracker {
    /// Screenshot latency samples in seconds (last 1000) - VecDeque for O(1) operations (Story 12-6)
    pub screenshot_latencies: Mutex<VecDeque<f64>>,
    /// WebSocket connection count
    pub websocket_count: AtomicU32,
}

impl MetricsTracker {
    pub fn new() -> Self {
        Self {
            screenshot_latencies: Mutex::new(VecDeque::with_capacity(1000)),
            websocket_count: AtomicU32::new(0),
        }
    }

    /// Record a screenshot latency sample (Story 12-6: O(1) with VecDeque)
    pub fn record_screenshot_latency(&self, latency_secs: f64) {
        if let Ok(mut latencies) = self.screenshot_latencies.lock() {
            latencies.push_back(latency_secs);
            // Keep only last 1000 samples - O(1) pop_front vs O(n) remove(0)
            if latencies.len() > 1000 {
                latencies.pop_front();
            }
        }
    }

    /// Get percentile of screenshot latencies (0.0-1.0)
    pub fn get_latency_percentile(&self, percentile: f64) -> Option<f64> {
        if let Ok(latencies) = self.screenshot_latencies.lock() {
            if latencies.is_empty() {
                return None;
            }
            // Convert to Vec for sorting (needed for percentile calculation)
            let mut sorted: Vec<f64> = latencies.iter().copied().collect();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let idx = ((sorted.len() - 1) as f64 * percentile) as usize;
            Some(sorted[idx])
        } else {
            None
        }
    }

    /// Increment WebSocket count
    pub fn increment_ws_count(&self) {
        self.websocket_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement WebSocket count
    pub fn decrement_ws_count(&self) {
        self.websocket_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current WebSocket count
    pub fn get_ws_count(&self) -> u32 {
        self.websocket_count.load(Ordering::Relaxed)
    }
}

impl Default for MetricsTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared application state passed to all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: AppConfig,
    pub connection_pool: Arc<ConnectionPool>,
    pub screenshot_cache: Arc<ScreenshotCache>,
    /// Device info cache (5 min TTL)
    pub device_info_cache: Cache<String, Value>,
    pub tera: tera::Tera,
    pub heartbeat_sessions: Arc<DashMap<String, HeartbeatSession>>,
    pub host_ip: String,
    /// Phone service for device queries (Story 13-1: shared via AppState)
    pub phone_service: Arc<PhoneService>,
    pub recording_service: RecordingService,
    /// Scrcpy session manager for high-fidelity screen mirroring (Story 6-1)
    pub scrcpy_manager: ScrcpyManager,
    /// Metrics tracker for latency and connection monitoring (Story 5-3)
    pub metrics: Arc<MetricsTracker>,
    /// Provider heartbeat tracking: provider_id → expiry timestamp (Story 9-2)
    pub provider_heartbeats: Arc<DashMap<i64, f64>>,
    /// Device reservation tracking: UDID → remote client address (Story 10-2)
    pub reserved_devices: Arc<DashMap<String, String>>,
    /// Video recording service for JPEG-to-MP4 conversion (Story 11-1)
    pub video_service: VideoService,
    /// Whether FFmpeg is available on the system (Story 11-1)
    pub ffmpeg_available: bool,
    /// Whether API key authentication is enabled (Story 12-1)
    pub api_key_enabled: bool,
    /// Whether rate limiting is enabled (Story 12-2)
    pub rate_limiting_enabled: bool,
}

impl AppState {
    pub fn new(
        db: Database,
        config: AppConfig,
        connection_pool: ConnectionPool,
        tera: tera::Tera,
        host_ip: String,
    ) -> Self {
        let phone_service = Arc::new(PhoneService::new(db.clone()));
        let recording_service = RecordingService::new(db.get_pool());
        let video_service = VideoService::new(db.clone());
        let api_key_enabled = config.api_key.as_ref().map_or(false, |k| !k.is_empty());
        let rate_limiting_enabled = config.rate_limit.is_some();

        // Use configurable cache settings (Story 12-4)
        let device_info_max = config.cache.device_info_max;
        let device_info_ttl = Duration::from_secs(config.cache.device_info_ttl_secs);

        Self {
            db,
            config,
            connection_pool: Arc::new(connection_pool),
            screenshot_cache: Arc::new(ScreenshotCache::new(20, Duration::from_millis(300))),
            device_info_cache: Cache::builder()
                .max_capacity(device_info_max)
                .time_to_live(device_info_ttl)
                .build(),
            tera,
            heartbeat_sessions: Arc::new(DashMap::new()),
            host_ip,
            phone_service,
            recording_service,
            scrcpy_manager: ScrcpyManager::new(),
            metrics: Arc::new(MetricsTracker::new()),
            provider_heartbeats: Arc::new(DashMap::new()),
            reserved_devices: Arc::new(DashMap::new()),
            video_service,
            ffmpeg_available: false, // Set at startup after async check
            api_key_enabled,
            rate_limiting_enabled,
        }
    }
}
