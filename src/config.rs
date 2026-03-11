use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

// ── Configurable bounds (Code Review Issue #1) ──
const POOL_MAX_SIZE_MIN: u64 = 1;
const POOL_MAX_SIZE_MAX: u64 = 100_000;
const POOL_IDLE_TIMEOUT_MIN: u64 = 1;
const POOL_IDLE_TIMEOUT_MAX: u64 = 86_400; // 24 hours
const CACHE_MAX_MIN: u64 = 1;
const CACHE_MAX_MAX: u64 = 100_000;
const CACHE_TTL_MIN: u64 = 1;
const CACHE_TTL_MAX: u64 = 86_400; // 24 hours

/// Main application configuration (Story 12-4: configurable server settings).
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub db_configs: DbConfig,
    /// Server description (fixed typo from "descption", alias for backwards compatibility)
    #[serde(alias = "descption", default)]
    pub description: Option<String>,
    /// Connection pool configuration (Story 12-4)
    #[serde(default)]
    pub pool: PoolConfig,
    /// Cache configuration (Story 12-4)
    #[serde(default)]
    pub cache: CacheConfig,
    /// API key for authentication (Story 12-1). If None or empty, auth is disabled.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Rate limiting configuration (Story 12-2). If None, rate limiting is disabled.
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
}

/// Connection pool configuration (Story 12-4).
/// Note: Uses u64 to match moka::Cache::max_capacity signature (Code Review Issue #2).
#[derive(Debug, Clone, Deserialize)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool (default: 1200, range: 1-100000)
    #[serde(default = "default_pool_max_size")]
    pub max_size: u64,
    /// Idle timeout in seconds (default: 600, range: 1-86400)
    #[serde(default = "default_pool_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: default_pool_max_size(),
            idle_timeout_secs: default_pool_idle_timeout_secs(),
        }
    }
}

fn default_pool_max_size() -> u64 {
    1200
}

fn default_pool_idle_timeout_secs() -> u64 {
    600
}

/// Cache configuration (Story 12-4).
/// Note: Uses u64 to match moka::Cache::max_capacity signature (Code Review Issue #2).
#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    /// Maximum capacity for device info cache (default: 500, range: 1-100000)
    #[serde(default = "default_cache_device_info_max")]
    pub device_info_max: u64,
    /// Time-to-live for device info cache entries in seconds (default: 300, range: 1-86400)
    #[serde(default = "default_cache_device_info_ttl_secs")]
    pub device_info_ttl_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            device_info_max: default_cache_device_info_max(),
            device_info_ttl_secs: default_cache_device_info_ttl_secs(),
        }
    }
}

fn default_cache_device_info_max() -> u64 {
    500
}

fn default_cache_device_info_ttl_secs() -> u64 {
    300
}

/// Rate limiting configuration (Story 12-2).
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per window per IP (default: 100)
    #[serde(default = "default_rate_limit")]
    pub requests_per_window: u32,
    /// Window size in seconds (default: 60)
    #[serde(default = "default_window_secs")]
    pub window_secs: u64,
    /// Per-category overrides: category_name → requests_per_window
    #[serde(default)]
    pub category_limits: HashMap<String, u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { port: default_port() }
    }
}

fn default_port() -> u16 {
    8000
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DbConfig {
    #[serde(default = "default_db_type")]
    pub r#type: String,
    #[serde(default = "default_db_name")]
    pub db_name: String,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub passwd: Option<String>,
    #[serde(default)]
    pub db_name1: Option<String>,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            r#type: default_db_type(),
            db_name: default_db_name(),
            user: None,
            passwd: None,
            db_name1: None,
        }
    }
}

fn default_db_type() -> String {
    "sqlite".to_string()
}

fn default_db_name() -> String {
    "cloudcontrol.db".to_string()
}

fn default_rate_limit() -> u32 {
    100
}

fn default_window_secs() -> u64 {
    60
}

impl AppConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref)
            .map_err(|e| format!("Failed to read config file '{}': {}", path_ref.display(), e))?;
        let config: AppConfig = serde_yml::from_str(&content)
            .map_err(|e| format!("Failed to parse config file '{}': {}", path_ref.display(), e))?;
        config.validate()?;
        tracing::info!("Configuration loaded successfully from {}", path_ref.display());
        Ok(config)
    }

    /// Validate configuration values are within acceptable bounds (Code Review Issue #1).
    pub fn validate(&self) -> Result<(), String> {
        // Pool validation
        if self.pool.max_size < POOL_MAX_SIZE_MIN {
            return Err(format!(
                "pool.max_size ({}) must be >= {}",
                self.pool.max_size, POOL_MAX_SIZE_MIN
            ));
        }
        if self.pool.max_size > POOL_MAX_SIZE_MAX {
            return Err(format!(
                "pool.max_size ({}) must be <= {}",
                self.pool.max_size, POOL_MAX_SIZE_MAX
            ));
        }
        if self.pool.idle_timeout_secs < POOL_IDLE_TIMEOUT_MIN {
            return Err(format!(
                "pool.idle_timeout_secs ({}) must be >= {}",
                self.pool.idle_timeout_secs, POOL_IDLE_TIMEOUT_MIN
            ));
        }
        if self.pool.idle_timeout_secs > POOL_IDLE_TIMEOUT_MAX {
            return Err(format!(
                "pool.idle_timeout_secs ({}) must be <= {} seconds",
                self.pool.idle_timeout_secs, POOL_IDLE_TIMEOUT_MAX
            ));
        }

        // Cache validation
        if self.cache.device_info_max < CACHE_MAX_MIN {
            return Err(format!(
                "cache.device_info_max ({}) must be >= {}",
                self.cache.device_info_max, CACHE_MAX_MIN
            ));
        }
        if self.cache.device_info_max > CACHE_MAX_MAX {
            return Err(format!(
                "cache.device_info_max ({}) must be <= {}",
                self.cache.device_info_max, CACHE_MAX_MAX
            ));
        }
        if self.cache.device_info_ttl_secs < CACHE_TTL_MIN {
            return Err(format!(
                "cache.device_info_ttl_secs ({}) must be >= {}",
                self.cache.device_info_ttl_secs, CACHE_TTL_MIN
            ));
        }
        if self.cache.device_info_ttl_secs > CACHE_TTL_MAX {
            return Err(format!(
                "cache.device_info_ttl_secs ({}) must be <= {} seconds",
                self.cache.device_info_ttl_secs, CACHE_TTL_MAX
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config_from_file() {
        let config = AppConfig::load("config/default_dev.yaml");
        assert!(config.is_ok(), "Should load config from file: {:?}", config.err());
        let config = config.unwrap();
        assert!(config.server.port > 0);
    }

    #[test]
    fn test_config_defaults() {
        let yaml = "{}";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.server.port, 8000);
        assert_eq!(config.db_configs.db_name, "cloudcontrol.db");
        assert_eq!(config.db_configs.r#type, "sqlite");
    }

    #[test]
    fn test_load_config_missing_file() {
        let result = AppConfig::load("nonexistent_path/config.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_api_key() {
        let yaml = "api_key: my-secret-key";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.api_key, Some("my-secret-key".to_string()));
    }

    #[test]
    fn test_config_without_api_key() {
        let yaml = "{}";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.api_key, None);
    }

    #[test]
    fn test_config_with_rate_limit() {
        let yaml = r#"
rate_limit:
  requests_per_window: 50
  window_secs: 30
  category_limits:
    screenshot: 10
    batch: 5
"#;
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        let rl = config.rate_limit.unwrap();
        assert_eq!(rl.requests_per_window, 50);
        assert_eq!(rl.window_secs, 30);
        assert_eq!(rl.category_limits.get("screenshot"), Some(&10));
        assert_eq!(rl.category_limits.get("batch"), Some(&5));
    }

    #[test]
    fn test_config_without_rate_limit() {
        let yaml = "{}";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert!(config.rate_limit.is_none());
    }

    #[test]
    fn test_config_rate_limit_defaults() {
        let yaml = "rate_limit: {}";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        let rl = config.rate_limit.unwrap();
        assert_eq!(rl.requests_per_window, 100);
        assert_eq!(rl.window_secs, 60);
        assert!(rl.category_limits.is_empty());
    }

    #[test]
    fn test_load_config_invalid_yaml() {
        let tmp = std::env::temp_dir().join("test_invalid.yaml");
        std::fs::write(&tmp, "{{{{invalid yaml!!!!").unwrap();
        let result = AppConfig::load(&tmp);
        assert!(result.is_err());
        let _ = std::fs::remove_file(&tmp);
    }

    // ── Story 12-4: Pool and Cache config tests ──

    #[test]
    fn test_config_pool_defaults() {
        let yaml = "{}";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.pool.max_size, 1200);
        assert_eq!(config.pool.idle_timeout_secs, 600);
    }

    #[test]
    fn test_config_cache_defaults() {
        let yaml = "{}";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.cache.device_info_max, 500);
        assert_eq!(config.cache.device_info_ttl_secs, 300);
    }

    #[test]
    fn test_config_with_pool_settings() {
        let yaml = r#"
pool:
  max_size: 2000
  idle_timeout_secs: 900
"#;
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.pool.max_size, 2000);
        assert_eq!(config.pool.idle_timeout_secs, 900);
    }

    #[test]
    fn test_config_with_cache_settings() {
        let yaml = r#"
cache:
  device_info_max: 1000
  device_info_ttl_secs: 600
"#;
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.cache.device_info_max, 1000);
        assert_eq!(config.cache.device_info_ttl_secs, 600);
    }

    #[test]
    fn test_config_description_alias() {
        // Test that "descption" (typo) still works via alias
        let yaml = "descption: old-style-description";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.description, Some("old-style-description".to_string()));

        // Test that "description" (correct) works
        let yaml = "description: new-style-description";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.description, Some("new-style-description".to_string()));
    }

    #[test]
    fn test_config_pool_partial() {
        // Test that partial pool config uses defaults for missing fields
        let yaml = "pool: { max_size: 500 }";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.pool.max_size, 500);
        assert_eq!(config.pool.idle_timeout_secs, 600); // default
    }

    #[test]
    fn test_config_cache_partial() {
        // Test that partial cache config uses defaults for missing fields
        let yaml = "cache: { device_info_max: 200 }";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.cache.device_info_max, 200);
        assert_eq!(config.cache.device_info_ttl_secs, 300); // default
    }

    // ── Validation tests (Code Review Issue #1) ──

    #[test]
    fn test_config_validation_pool_max_size_too_low() {
        let yaml = "pool: { max_size: 0 }";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.contains("pool.max_size"));
        assert!(err.contains("must be >="));
    }

    #[test]
    fn test_config_validation_pool_max_size_too_high() {
        let yaml = "pool: { max_size: 999999999 }";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.contains("pool.max_size"));
        assert!(err.contains("must be <="));
    }

    #[test]
    fn test_config_validation_pool_idle_timeout_too_low() {
        let yaml = "pool: { idle_timeout_secs: 0 }";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.contains("pool.idle_timeout_secs"));
    }

    #[test]
    fn test_config_validation_cache_max_too_low() {
        let yaml = "cache: { device_info_max: 0 }";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.contains("cache.device_info_max"));
    }

    #[test]
    fn test_config_validation_cache_ttl_too_low() {
        let yaml = "cache: { device_info_ttl_secs: 0 }";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.contains("cache.device_info_ttl_secs"));
    }

    #[test]
    fn test_config_validation_valid_bounds() {
        // Test edge cases that should pass
        let yaml = r#"
pool:
  max_size: 1
  idle_timeout_secs: 86400
cache:
  device_info_max: 1
  device_info_ttl_secs: 86400
"#;
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_defaults_pass() {
        // Default values should always pass validation
        let yaml = "{}";
        let config: AppConfig = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }
}
