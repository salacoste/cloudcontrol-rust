---
project_name: 'cloudcontrol-rust'
user_name: 'R2d2'
date: '2025-03-05'
sections_completed:
  ['technology_stack', 'language_rules', 'framework_rules', 'testing_rules', 'code_quality', 'workflow_rules', 'critical_rules']
status: 'complete'
rule_count: 25
optimized_for_llm: true
---

# Project Context for AI Agents

_Critical rules and patterns that AI agents must follow when implementing code in this project. Focus on unobvious details that agents might miss._

---

## Technology Stack & Versions

| Component | Version | Notes |
|-----------|---------|-------|
| Rust | 2021 Edition | Required edition |
| actix-web | 4.x | Web framework + actix-ws 0.3 |
| tokio | 1.x | Features: full |
| sqlx | 0.8 | SQLite, compile-time verification |
| tera | 1.x | Templates in resources/templates/ |
| reqwest | 0.12 | Features: json, multipart |
| moka | 0.12 | LRU cache with async support |
| serde_json | 1.x | JSON primary format |
| image | 0.25 | JPEG/PNG only (no default features) |
| tracing | 0.1.x | Structured logging |
| quick-xml | 0.37 | UI hierarchy parsing |
| dashmap | 6 | Concurrent hashmap |

```toml
[profile.release]
opt-level = 3
lto = true
```

---

## Critical Implementation Rules

### Rust Language Rules

**Async/Await:**
```rust
#[tokio::test]           // Unit tests with async
#[actix_web::test]       // Route tests with actix context
pub async fn handler()   // Service layer async
```

**Error Handling:**
```rust
// Service layer: Result<T, String>
pub async fn screenshot_base64(...) -> Result<String, String>

// Use map_err for conversions
.map_err(|e| format!("Failed: {}", e))?
```

**Ownership & Sharing:**
```rust
#[derive(Clone)]
pub struct AppState {
    pub connection_pool: Arc<ConnectionPool>,
    pub heartbeat_sessions: Arc<DashMap<String, HeartbeatSession>>,
}
```

---

### actix-web Framework Rules

**Route Registration (main.rs):**
```rust
.route("/list", web::get().to(routes::control::device_list))
.route("/inspector/{udid}/screenshot", web::get().to(...))
.route("/api/wifi-connect", web::post().to(...))
```

**Handler Signatures:**
```rust
// Basic
pub async fn handler(state: web::Data<AppState>) -> HttpResponse

// Path params
pub async fn device_info(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse

// JSON body
pub async fn wifi_connect(state: web::Data<AppState>, body: web::Json<Req>) -> HttpResponse

// WebSocket
pub async fn ws(req: HttpRequest, stream: Payload, state: web::Data<AppState>) -> Result<HttpResponse, Error>
```

**Error Middleware:**
```rust
.wrap(ErrorHandlers::new()
    .handler(StatusCode::NOT_FOUND, error::handle_404)
    .handler(StatusCode::INTERNAL_SERVER_ERROR, error::handle_500))
```

---

### Testing Rules

**Organization:**
```
tests/
├── common/mod.rs         # create_temp_db(), make_device_json()
├── test_config.rs        # 3 tests
├── test_database.rs      # 8 tests
├── test_services.rs      # 7 tests
└── test_server.rs        # 20 E2E tests
```

**Unit Test Pattern:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_name() { }
}
```

**Integration Test Pattern:**
```rust
mod common;
use common::{create_temp_db, make_device_json};

#[tokio::test]
async fn test_crud() {
    let (_tmp, db) = create_temp_db().await;  // Keep TempDir alive!
    let svc = PhoneService::new(db);
}
```

**E2E Test Pattern:**
```rust
#[actix_web::test]
async fn test_endpoint() {
    let (tmp, state) = create_test_app_state().await;
    let app = test::init_service(App::new().app_data(web::Data::new(state))).await;
    let resp = test::call_service(&app, test::TestRequest::get().uri("/list").to_request()).await;
}
```

---

### Code Quality Rules

**Naming:**
| Element | Convention | Example |
|---------|------------|---------|
| Files/Modules | snake_case | `device_service.rs` |
| Types | PascalCase | `AppState` |
| Functions | snake_case | `screenshot_base64()` |
| Constants | SCREAMING_SNAKE | `MOCK_DATA` |
| Tests | test_ prefix | `test_phone_service()` |

**Comments:**
```rust
// ─── Section headers with em-dash ───
// Brief inline for non-obvious logic
/// Doc comments for public APIs
```

**Logging:**
```rust
tracing::info!("[Screenshot] q={} | total={:.0}ms", quality, total_ms);
```

---

### Workflow Rules

**Commands:**
```bash
cargo build --release    # Production (LTO enabled)
cargo test               # All 90 tests
cargo test --lib         # Unit only (52)
cargo test --test test_server  # E2E only (20)
```

**Startup Sequence:**
1. Load `config/default_dev.yaml`
2. Init SQLite database
3. **Clear devices table** (Python behavior)
4. Create connection pool (1200 max, 600s TTL)
5. Load Tera templates
6. Start USB device detector
7. Bind to `0.0.0.0:8000`

---

### Critical Don't-Miss Rules

**Anti-Patterns:**

| ❌ Don't | ✅ Do |
|----------|-------|
| Reference MongoDB | SQLite only |
| Skip `.await` | Always await futures |
| Use `unwrap()` in prod | Use `?` or `map_err()` |
| Forget `#[derive(Clone)]` for AppState | Required for handlers |
| Mix test macros | `tokio::test` for unit, `actix_web::test` for routes |
| Drop TempDir early | `let (_tmp, db) = create_temp_db()` keeps it alive |

**USB vs WiFi Resolution:**
```rust
// WiFi: ip != "127.0.0.1" → use directly
// Forwarded: ip == "127.0.0.1" && port != 9008 → use as-is
// USB: requires adb forward
```

**Performance:**
```rust
// Image resize: Use Nearest for speed (NOT Lanczos3)
img.resize(w, h, image::imageops::FilterType::Nearest)

// Moka cache: Allow processing time
tokio::time::sleep(Duration::from_millis(50)).await;
```

**Startup Restores Persisted Devices:**
```rust
// Device state persists across restarts
phone_service.restore_devices().await.expect("...");
```

**Corrupted Database Recovery:**
- Automatic backup with timestamp suffix
- Fresh database initialization
- Warning logged about data loss

---

## Project Structure

```
src/
├── main.rs              # Entry, server setup
├── lib.rs               # Module exports
├── config.rs            # AppConfig (YAML)
├── error.rs             # HTTP 404/500 handlers
├── state.rs             # AppState
├── db/sqlite.rs         # Database ops
├── device/
│   ├── adb.rs           # ADB wrapper
│   ├── atx_client.rs    # ATX Agent HTTP
│   └── scrcpy.rs        # Scrcpy integration
├── models/              # Data models
├── pool/
│   ├── connection_pool.rs    # moka LRU
│   └── screenshot_cache.rs   # Dedup
├── routes/
│   ├── control.rs       # Main API
│   ├── nio.rs           # NIO WebSocket
│   └── scrcpy_ws.rs     # Scrcpy WS
├── services/            # Business logic
└── utils/               # Helpers
```

---

_Generated 2025-03-05 | 90 tests | SQLite | actix-web 4_

---

## Usage Guidelines

**For AI Agents:**
- Read this file before implementing any code
- Follow ALL rules exactly as documented
- When in doubt, prefer the more restrictive option
- Update this file if new patterns emerge

**For Humans:**
- Keep this file lean and focused on agent needs
- Update when technology stack changes
- Review quarterly for outdated rules
- Remove rules that become obvious over time
