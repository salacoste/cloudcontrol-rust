use crate::config::AppConfig;
use crate::db::Database;
use crate::pool::connection_pool::ConnectionPool;
use crate::pool::screenshot_cache::ScreenshotCache;
use crate::services::recording_service::RecordingService;
use crate::services::scrcpy_manager::ScrcpyManager;
use dashmap::DashMap;
use moka::future::Cache;
use serde_json::Value;
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

/// Metrics tracker for screenshot latency and connection counts (Story 5-3)
#[derive(Debug)]
pub struct MetricsTracker {
    /// Screenshot latency samples in seconds (last 1000)
    pub screenshot_latencies: Mutex<Vec<f64>>,
    /// WebSocket connection count
    pub websocket_count: AtomicU32,
}

impl MetricsTracker {
    pub fn new() -> Self {
        Self {
            screenshot_latencies: Mutex::new(Vec::with_capacity(1000)),
            websocket_count: AtomicU32::new(0),
        }
    }

    /// Record a screenshot latency sample
    pub fn record_screenshot_latency(&self, latency_secs: f64) {
        if let Ok(mut latencies) = self.screenshot_latencies.lock() {
            latencies.push(latency_secs);
            // Keep only last 1000 samples
            if latencies.len() > 1000 {
                latencies.remove(0);
            }
        }
    }

    /// Get percentile of screenshot latencies (0.0-1.0)
    pub fn get_latency_percentile(&self, percentile: f64) -> Option<f64> {
        if let Ok(latencies) = self.screenshot_latencies.lock() {
            if latencies.is_empty() {
                return None;
            }
            let mut sorted = latencies.clone();
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
    pub recording_service: RecordingService,
    /// Scrcpy session manager for high-fidelity screen mirroring (Story 6-1)
    pub scrcpy_manager: ScrcpyManager,
    /// Metrics tracker for latency and connection monitoring (Story 5-3)
    pub metrics: Arc<MetricsTracker>,
}

impl AppState {
    pub fn new(
        db: Database,
        config: AppConfig,
        connection_pool: ConnectionPool,
        tera: tera::Tera,
        host_ip: String,
    ) -> Self {
        let recording_service = RecordingService::new(db.get_pool());
        Self {
            db,
            config,
            connection_pool: Arc::new(connection_pool),
            screenshot_cache: Arc::new(ScreenshotCache::new(20, Duration::from_millis(300))),
            device_info_cache: Cache::builder()
                .max_capacity(500)
                .time_to_live(Duration::from_secs(300))
                .build(),
            tera,
            heartbeat_sessions: Arc::new(DashMap::new()),
            host_ip,
            recording_service,
            scrcpy_manager: ScrcpyManager::new(),
            metrics: Arc::new(MetricsTracker::new()),
        }
    }
}
