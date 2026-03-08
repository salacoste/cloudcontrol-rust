use crate::config::AppConfig;
use crate::db::Database;
use crate::pool::connection_pool::ConnectionPool;
use crate::pool::screenshot_cache::ScreenshotCache;
use crate::services::recording_service::RecordingService;
use dashmap::DashMap;
use moka::future::Cache;
use serde_json::Value;
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
        }
    }
}
