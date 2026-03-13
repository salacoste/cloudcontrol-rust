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

// ============================================================================
// Team Test Helpers (Story 14-3)
// ============================================================================

use cloudcontrol::models::user::User;

/// Create a test user in the database
pub async fn create_test_user(app_state: &AppState, email: &str, password: &str) -> User {
    let pool = app_state.db.get_pool();
    let user_id = format!("user_{}", uuid::Uuid::new_v4().simple());
    // Use a simple hash for tests - the actual auth service handles real hashing
    let password_hash = format!("test_hash_{}", password);
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, role, created_at)
        VALUES (?, ?, ?, 'agent', ?)
        "#,
    )
    .bind(&user_id)
    .bind(email)
    .bind(&password_hash)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    User {
        id: user_id,
        email: email.to_string(),
        password_hash,
        role: "agent".to_string(),
        team_id: None,
        created_at: now,
        last_login_at: None,
    }
}

/// Add a user to a team
pub async fn add_user_to_team(app_state: &AppState, user_id: &str, team_id: &str) {
    let pool = app_state.db.get_pool();

    sqlx::query("UPDATE users SET team_id = ? WHERE id = ?")
        .bind(team_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
}

/// Promote a user to admin role (Story 14-3 test helper)
pub async fn promote_user_to_admin(app_state: &AppState, user_id: &str) {
    let pool = app_state.db.get_pool();

    sqlx::query("UPDATE users SET role = 'admin' WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
}

/// Create a test device in the database
pub async fn create_test_device(app_state: &AppState, udid: &str) -> cloudcontrol::models::device::Device {
    let pool = app_state.db.get_pool();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO devices (udid, serial, ip, port, present, ready, using_device, is_server, is_mock, model, brand, version, sdk, display, memory, cpu, battery, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(udid)
    .bind(udid)
    .bind("192.168.1.100")
    .bind(7912i64)
    .bind(1i64)
    .bind(1i64)
    .bind(0i64)
    .bind(0i64)
    .bind(0i64)
    .bind("TestModel")
    .bind("TestBrand")
    .bind("12")
    .bind(31i64)
    .bind(json!({"width": 1080, "height": 1920}).to_string())
    .bind(json!({"total": 8192}).to_string())
    .bind(json!({"cores": 8}).to_string())
    .bind(json!({"level": 85}).to_string())
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    cloudcontrol::models::device::Device {
        udid: udid.to_string(),
        serial: Some(udid.to_string()),
        ip: Some("192.168.1.100".to_string()),
        port: Some(7912),
        present: true,
        ready: true,
        using_device: false,
        is_server: false,
        is_mock: false,
        model: Some("TestModel".to_string()),
        brand: Some("TestBrand".to_string()),
        version: Some("12".to_string()),
        sdk: Some(31),
        display: Some(json!({"width": 1080, "height": 1920})),
        memory: Some(json!({"total": 8192})),
        cpu: Some(json!({"cores": 8})),
        battery: Some(json!({"level": 85})),
        owner: None,
        provider: None,
        agent_version: None,
        hwaddr: None,
        created_at: Some(now.clone()),
        updated_at: Some(now),
        update_time: None,
        extra_data: None,
    }
}

/// Assign a device to a team (stored in team_id column - Story 14-3)
pub async fn assign_device_to_team(app_state: &AppState, udid: &str, team_id: &str) {
    let pool = app_state.db.get_pool();

    // Update team_id column directly
    sqlx::query("UPDATE devices SET team_id = ? WHERE udid = ?")
        .bind(team_id)
        .bind(udid)
        .execute(&pool)
        .await
        .unwrap();
}
