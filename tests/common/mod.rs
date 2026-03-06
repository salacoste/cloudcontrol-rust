#![allow(dead_code)]

use cloudcontrol::config::{AppConfig, DbConfig, ServerConfig};
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

/// Create a test AppConfig with defaults.
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
        descption: None,
        redis_configs: None,
        kafka_configs: None,
        rest_server_configs: None,
        influxdb_configs: None,
        spider: None,
    }
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
