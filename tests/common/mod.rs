#![allow(dead_code)]

use cloudcontrol::config::{AppConfig, AuthConfig, CacheConfig, DbConfig, PoolConfig, RateLimitConfig, ServerConfig};
use cloudcontrol::db::Database;
use cloudcontrol::pool::connection_pool::ConnectionPool;
use cloudcontrol::state::AppState;
use serde_json::{json, Value};
use std::time::Duration;
use tempfile::TempDir;

/// Create a temporary database for testing.
pub async fn create_temp_db() -> (TempDir, Database) {
    let tmp = TempDir::new().unwrap();
    let db = Database::new(tmp.path().to_str().unwrap(), "test.db")
        .await
        .unwrap();
    (tmp, db)
}

/// Build a device JSON object for testing.
pub fn make_device_json(udid: &str, present: bool, is_mock: bool) -> Value {
    json!({
        "udid": udid,
        "serial": udid,
        "ip": "192.168.1.100",
        "port": 7912,
        "present": present,
        "ready": true,
        "using": false,
        "is_server": false,
        "is_mock": is_mock,
        "model": "TestPhone",
        "brand": "TestBrand",
        "version": "12",
        "sdk": 31,
        "display": {"width": 1080, "height": 1920},
        "memory": {"total": 8192},
        "cpu": {"cores": 8},
        "battery": {"level": 85},
        "tags": [],
    })
}

/// Create a test AppConfig with defaults (Story 12-4: updated structure).
pub fn make_test_config() -> AppConfig {
    AppConfig {
        server: ServerConfig { port: 8000 },
        db_configs: DbConfig {
            r#type: "sqlite".into(),
            db_name: "test.db".into(),
            user: None,
            passwd: None,
            db_name1: None,
        },
        description: None,
        pool: PoolConfig::default(),
        cache: CacheConfig::default(),
        api_key: None,
        rate_limit: None,
        auth: None,
    }
}

/// Create a test AppConfig with API key authentication enabled.
pub fn make_test_config_with_auth(api_key: &str) -> AppConfig {
    let mut config = make_test_config();
    config.api_key = Some(api_key.to_string());
    config
}

/// Create a test AppConfig with rate limiting enabled.
pub fn make_test_config_with_rate_limit(requests_per_window: u32, window_secs: u64) -> AppConfig {
    let mut config = make_test_config();
    config.rate_limit = Some(RateLimitConfig {
        requests_per_window,
        window_secs,
        category_limits: std::collections::HashMap::new(),
    });
    config
}

/// Create a test AppConfig with custom pool settings (Story 12-4).
pub fn make_test_config_with_pool(max_size: u64, idle_timeout_secs: u64) -> AppConfig {
    let mut config = make_test_config();
    config.pool = PoolConfig {
        max_size,
        idle_timeout_secs,
    };
    config
}

/// Create a test AppConfig with custom cache settings (Story 12-4).
pub fn make_test_config_with_cache(device_info_max: u64, device_info_ttl_secs: u64) -> AppConfig {
    let mut config = make_test_config();
    config.cache = CacheConfig {
        device_info_max,
        device_info_ttl_secs,
    };
    config
}

/// Create a test AppConfig with JWT authentication enabled (Story 14-1).
pub fn make_test_config_with_jwt_auth(jwt_secret: &str) -> AppConfig {
    let mut config = make_test_config();
    config.auth = Some(AuthConfig {
        jwt_secret: Some(jwt_secret.to_string()),
        access_token_expiry_minutes: 15,
        refresh_token_expiry_days: 7,
    });
    config
}

/// Create a complete test AppState with a temporary database.
pub async fn create_test_app_state() -> (TempDir, AppState) {
    let (tmp, db) = create_temp_db().await;
    let config = make_test_config();
    let pool = ConnectionPool::new(100, Duration::from_secs(60));
    let tera = tera::Tera::new("resources/templates/**/*").unwrap();
    let state = AppState::new(db, config, pool, tera, "127.0.0.1".to_string());
    (tmp, state)
}

/// Create a test AppState with JWT authentication enabled (Story 14-1).
pub async fn create_test_app_state_with_jwt_auth() -> (TempDir, AppState) {
    let (tmp, db) = create_temp_db().await;
    let config = make_test_config_with_jwt_auth("test-secret-key-at-least-32-characters-long");
    let pool = ConnectionPool::new(100, Duration::from_secs(60));
    let tera = tera::Tera::new("resources/templates/**/*").unwrap();
    let state = AppState::new(db, config, pool, tera, "127.0.0.1".to_string());
    (tmp, state)
}
