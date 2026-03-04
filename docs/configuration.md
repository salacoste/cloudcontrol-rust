# Configuration

## Configuration File

**Path:** `config/default_dev.yaml`

```yaml
# Database
db_configs:
  type: sqlite
  db_name: cloudcontrol.db

# Server
server:
  port: 8000

# Legacy configs (not used)
redis_configs:
  redis_url: redis://...

kafka_configs:
  topic: lang
  bootstrap_servers: "..."
```

---

## AppConfig

**File:** `src/config.rs`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub db_configs: DbConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 { 8000 }

#[derive(Debug, Clone, Deserialize)]
pub struct DbConfig {
    #[serde(default = "default_db_type")]
    pub r#type: String,
    #[serde(default = "default_db_name")]
    pub db_name: String,
}
```

---

## Loading Configuration

```rust
let config = AppConfig::load("config/default_dev.yaml")
    .expect("Failed to load configuration");
```

---

## Environment Variables

The application uses tracing for logging, configurable via environment:

```bash
# Set log level
RUST_LOG=cloudcontrol=debug,actix_web=info cargo run

# Or in .env
RUST_LOG=cloudcontrol=info
```

---

## Database Location

- Default: `database/cloudcontrol.db`
- Configured via `db_configs.db_name`
- Created automatically on first run

---

## Connection Pool Settings

Hardcoded in `main.rs`:

```rust
let connection_pool = ConnectionPool::new(
    1200,                        // max_size
    Duration::from_secs(600),    // idle_timeout
);
```

---

## Template Location

- Path: `resources/templates/**/*`
- Loaded at startup via Tera

---

## Static Files

- Path: `resources/static/`
- Served at `/static/*`
