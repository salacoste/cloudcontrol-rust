---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
inputDocuments: [project-context.md, docs/]
workflowType: 'architecture'
project_name: 'cloudcontrol-rust'
user_name: 'R2d2'
date: '2025-03-05'
lastStep: 8
status: 'complete'
completedAt: '2025-03-05'
---

# Architecture Decision Document

_This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together._

---

## Project Context Analysis

### Requirements Overview

**Functional Requirements:**

| Category | Features | Architectural Impact |
|----------|----------|---------------------|
| Device Management | WiFi + USB detection, lifecycle tracking | Connection pool, ADB integration |
| Screenshot | Real-time streaming via WebSocket | Binary WS frames, moka cache |
| Remote Control | Touch, swipe, input, keyevent | ATX agent client, JSON-RPC |
| UI Hierarchy | XML → JSON parsing | Parser module, inspector endpoint |
| File Management | Upload, install, track | FileService, SQLite storage |
| Screen Mirroring | Scrcpy integration | Process management, video parsing |
| Batch Operations | Multi-device control | Async processing, concurrent execution |

**Non-Functional Requirements:**

| NFR | Requirement | Architectural Decision |
|-----|-------------|----------------------|
| Latency | <100ms screenshot streaming | Binary WS, device-side compression |
| Concurrency | 1200 max pooled connections | moka LRU cache with TTL |
| Reliability | Graceful degradation | Error handlers, fallback paths |
| Testability | 90 tests coverage | Modular services, dependency injection |

**Scale & Complexity:**

- Primary domain: **Backend (API + WebSocket)**
- Complexity level: **Medium**
- Estimated architectural components: **12 modules**

### Technical Constraints & Dependencies

| Constraint | Impact |
|------------|--------|
| SQLite only | Single-file DB, no external DB server |
| ATX Agent required | Devices must run uiautomator2 on port 7912/9008 |
| ADB required | USB devices need Android Debug Bridge |
| Rust 2021 Edition | Edition-specific features |
| actix-web 4.x | Actor-based async handlers |

### Cross-Cutting Concerns Identified

1. **Connection Pooling** - Shared across all device operations
2. **Error Handling** - `Result<T, String>` pattern in services
3. **State Management** - AppState Clone + Arc for thread safety
4. **Caching** - Device info cache, screenshot cache
5. **Logging** - tracing crate with timing logs
6. **WebSocket Sessions** - DashMap for concurrent session tracking

---

## Starter Template Evaluation

### Primary Technology Domain

**Backend API + WebSocket** based on project requirements analysis

> **Note:** This is a **brownfield project** (existing codebase). The project was built without a formal starter template. We document existing technology choices rather than selecting new ones.

### Existing Technology Stack

| Category | Technology | Version | Rationale |
|----------|------------|---------|-----------|
| **Language** | Rust | 2021 Edition | Memory safety, zero-cost abstractions, async support |
| **Web Framework** | actix-web | 4.x | Actor-based, high performance, mature ecosystem |
| **Async Runtime** | tokio | 1.x (full) | Industry standard, excellent ecosystem |
| **Database** | sqlx + SQLite | 0.8 | Compile-time verification, simple deployment |
| **WebSocket** | actix-ws | 0.3 | Native actix integration |
| **HTTP Client** | reqwest | 0.12 | Async, feature-rich |
| **Caching** | moka | 0.12 | High-performance LRU cache |
| **Templating** | tera | 1.x | Jinja2-like syntax |
| **Logging** | tracing | 0.1.x | Structured, async-compatible |
| **Serialization** | serde_json | 1.x | De facto standard |

### Architectural Decisions Provided by Existing Stack

**Language & Runtime:**
- Rust 2021 Edition with full async support
- Zero-cost abstractions for performance-critical paths
- Memory safety without garbage collection

**Build Tooling:**
- cargo for dependency management
- Release profile with LTO enabled
- sqlx compile-time query verification

**Testing Framework:**
- Built-in `#[test]` and `#[tokio::test]`
- actix-web test utilities
- tempfile for temporary databases

**Code Organization:**
- Module-based structure (db/, device/, models/, pool/, routes/, services/, utils/)
- Service layer separation
- Shared state pattern (AppState)

**Development Experience:**
- Hot reload via cargo-watch (optional)
- Structured logging with tracing
- Type-safe database queries

### No Starter Template Required

This brownfield project was developed incrementally. The technology choices reflect:
1. Migration from Python (original CloudControl)
2. SQLite replacing MongoDB for simplicity
3. actix-web for async performance
4. moka for connection pooling (replacing custom Python implementation)

---

## Core Architectural Decisions

### Decision Summary

| # | Category | Decision | Status |
|---|----------|----------|--------|
| 1 | API Architecture | actix-web + actix-ws | ✅ Documented |
| 2 | Data Layer | SQLite + sqlx | ✅ Documented |
| 3 | State Management | AppState (Clone + Arc + DashMap) | ✅ Documented |
| 4 | Device Integration | ATX Client + ADB + Scrcpy | ✅ Documented |
| 5 | Connection Pooling | moka LRU cache | ✅ Documented |
| 6 | Services | Service layer pattern | ✅ Documented |
| 7 | Configuration | YAML + AppConfig | ✅ Documented |
| 8 | Error Handling | Result<T, String> | ✅ Documented |
| 9 | Testing | 90 tests | ✅ Documented |
| 10 | Frontend | tera templates | ✅ Documented |
| 11 | Authentication | None | ✅ No auth required |
| 12 | Validation | sqlx compile-time | ✅ Documented |
| 13 | Session Management | actix-ws + DashMap | ✅ Documented |
| 14 | Logging | tracing structured | ✅ Documented |
| 15 | Background Tasks | tokio::spawn | ✅ Documented |

---

### Category 1: API Architecture

**Decision:** actix-web 4.x + actix-ws for WebSockets

**Rationale:**
- Actor-based model for high concurrency
- Native WebSocket support (actix-ws)
- Mature ecosystem with good documentation

**Key Patterns:**
- Route handlers receive `web::Data<AppState>`
- JSON responses via `HttpResponse::Ok().json(data)`
- WebSocket binary frames for screenshots

**Endpoints:**

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/list` | Device list |
| GET | `/devices/{udid}/info` | Device info |
| GET | `/inspector/{udid}/screenshot` | Screenshot |
| POST | `/inspector/{udid}/touch` | Touch event |
| GET | `/nio/{udid}/ws` | WebSocket control |

---

### Category 2: Data Layer

**Decision:** SQLite + sqlx with compile-time verification

**Rationale:**
- Single-file deployment (no DB server)
- Compile-time SQL verification prevents runtime errors
- Simple backup/restore

**Schema:**
- `devices` - Device state and metadata
- `installed_file` - File upload tracking

**Field Mapping:**
- JSON fields stored as TEXT (memory, cpu, battery, display)
- MongoDB-style field names for API compatibility

---

### Category 3: State Management

**Decision:** AppState with Clone + Arc + DashMap

```rust
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: AppConfig,
    pub connection_pool: Arc<ConnectionPool>,
    pub screenshot_cache: Arc<ScreenshotCache>,
    pub device_info_cache: Cache<String, Value>,
    pub tera: tera::Tera,
    pub heartbeat_sessions: Arc<DashMap<String, HeartbeatSession>>,
    pub host_ip: String,
}
```

**Rationale:**
- Clone for actix-web handler requirements
- Arc for thread-safe shared resources
- DashMap for concurrent session tracking
- moka Cache for device info caching

---

### Category 4: Device Integration

**Decision:** ATX Client (HTTP) + ADB (command) + Scrcpy (mirroring)

**Components:**

| Component | Protocol | Port |
|-----------|----------|------|
| ATX Client | HTTP | 7912/9008 |
| ADB | Command | 5555 |
| Scrcpy | Video | Dynamic |

**Connection Resolution:**
1. WiFi → Direct IP connection
2. USB → ADB forward to localhost

---

### Category 5: Connection Pooling

**Decision:** moka LRU cache for ATX connections

```rust
pub struct ConnectionPool {
    cache: Cache<String, Arc<AtxClient>>,
}
```

**Configuration:**
- Max size: 1200
- Idle timeout: 600s
- Automatic eviction on TTL

---

### Category 6: Services

**Decision:** Service layer pattern with `Result<T, String>`

**Services:**

| Service | Responsibility |
|---------|----------------|
| PhoneService | Device lifecycle |
| DeviceService | Screenshot, hierarchy, operations |
| FileService | File upload tracking |

**Pattern:**
- Services wrap Database
- Async methods with tokio
- Error propagation via Result

---

### Category 7: Configuration

**Decision:** YAML configuration with AppConfig struct

**File:** `config/default_dev.yaml`

**Structure:**
```yaml
server:
  port: 8000

db_configs:
  type: sqlite
  db_name: cloudcontrol.db
```

---

### Category 8: Error Handling

**Decision:** `Result<T, String>` in services, HTTP error codes

**Service Layer:**
```rust
pub async fn operation() -> Result<T, String>
```

**HTTP Layer:**
```rust
HttpResponse::InternalServerError().json(json!({"error": msg}))
HttpResponse::NotFound().json(json!({"error": "Not found"}))
```

---

### Category 9: Testing

**Decision:** 90 tests with tempfile databases

**Distribution:**
- Unit tests: `#[cfg(test)]` in source files
- Integration tests: `tests/test_*.rs`
- E2E tests: actix-web test utilities

**Test Utilities:**
- `create_temp_db()` - Creates temporary SQLite
- `make_device_json()` - Builds test device JSON

---

### Category 10: Frontend

**Decision:** tera templates + static files

**Templates:**
- `device_synchronous.html` - Device list page
- `remote.html` - Remote control page
- `file.html` - File upload page

**Static:**
- Served via `/static/*`
- CSS and JavaScript files

---

### Category 11: Authentication

**Decision:** None required

**Rationale:**
- Internal API for device management
- No user accounts or sessions
- Devices authenticate via ATX agent connection

---

### Category 12: Validation

**Decision:** sqlx compile-time + serde_json parsing

**Patterns:**
- Compile-time SQL verification catches errors early
- JSON parsing with serde
- Query parameters with defaults

---

### Category 13: Session Management

**Decision:** actix-ws + DashMap for heartbeat tracking

**Implementation:**
- actix-ws handles WebSocket lifecycle
- DashMap tracks active heartbeat sessions
- Cleanup on disconnect

---

### Category 14: Logging

**Decision:** tracing crate with structured logs

**Pattern:**
```rust
tracing::info!("Server starting on port {}", port);
tracing::error!("Failed: {}", err);
tracing::debug!("Processing request for device {}", udid);
```

**Features:**
- Timing logs for performance monitoring
- Structured format for filtering

---

### Category 15: Background Tasks

**Decision:** tokio::spawn for async background processing

**Use Cases:**
- USB device detector (polling loop)
- Screenshot streaming (per WebSocket session)
- Scrcpy process management

---

## Implementation Patterns & Consistency Rules

### Potential Conflict Points

| Category | Conflict Risk | Mitigation |
|----------|---------------|------------|
| Naming | Low | Existing conventions well-established |
| Structure | Low | Module organization documented |
| Format | Low | Error/response formats consistent |
| Process | Low | Service patterns established |

### Naming Patterns

**Files & Modules:**
| Element | Convention | Example |
|---------|------------|---------|
| Files | snake_case | `device_service.rs` |
| Modules | snake_case | `pub mod device_service` |
| Structs/Enums | PascalCase | `AppState`, `DeviceService` |
| Functions | snake_case | `screenshot_base64()` |
| Variables | snake_case | `connection_pool` |
| Constants | SCREAMING_SNAKE | `MOCK_DATA` |

**JSON API:**
| Element | Convention | Example |
|---------|------------|---------|
| Fields | camelCase | `agentVersion`, `createdAt` |
| DB columns | snake_case | `agent_version`, `created_at` |

### Structure Patterns

| Category | Location | Pattern |
|----------|----------|---------|
| Unit tests | Same file | `#[cfg(test)] mod tests` |
| Integration tests | `tests/` | `tests/test_*.rs` |
| Config | `config/` | YAML files |
| Static assets | `resources/static/` | CSS, JS |
| Templates | `resources/templates/` | HTML (tera) |

### Format Patterns

**API Responses:**
```rust
// Success
HttpResponse::Ok().json(device)

// Error
HttpResponse::InternalServerError().json(json!({"error": msg}))
```

**Service Errors:**
```rust
pub async fn operation() -> Result<T, String>
```

### Process Patterns

**Service Creation:**
```rust
// Per-request service instantiation
let svc = PhoneService::new(state.db.clone());
```

**Client Creation:**
```rust
// Pooled via connection pool
let client = state.connection_pool.get_or_create(udid, ip, port).await;
```

**Error Propagation:**
```rust
.map_err(|e| format!("Operation failed: {}", e))?
```

### AI Agent Enforcement

**All AI agents MUST:**
1. Follow snake_case for files, modules, functions, variables
2. Use PascalCase for structs and enums
3. Place unit tests in same file with `#[cfg(test)]`
4. Use `Result<T, String>` for service layer errors
5. Create services per-request, not as singletons
6. Use connection pool for AtxClient instances

---

## Project Structure & Boundaries

### Complete Project Directory Structure

```
cloudcontrol-rust/
├── Cargo.toml                 # Dependencies & build config
├── Cargo.lock                 # Locked dependencies
├── README.md                  # Project overview
├── config/
│   └── default_dev.yaml       # Configuration file
├── database/                  # SQLite database (auto-created)
├── resources/
│   ├── static/                # CSS, JS files
│   │   └── js/
│   │       ├── main.js
│   │       └── ...
│   └── templates/             # HTML templates (tera)
│       ├── base.html
│       ├── device_synchronous.html
│       ├── remote.html
│       ├── file.html
│       ├── 404.html
│       └── 500.html
├── src/
│   ├── main.rs                # Entry point
│   ├── lib.rs                 # Module exports
│   ├── config.rs              # AppConfig
│   ├── error.rs               # HTTP error handlers
│   ├── state.rs               # AppState
│   ├── db/
│   │   ├── mod.rs
│   │   └── sqlite.rs          # Database operations
│   ├── device/
│   │   ├── mod.rs
│   │   ├── adb.rs             # ADB wrapper
│   │   ├── atx_client.rs      # ATX HTTP client
│   │   ├── atx_init.rs        # ATX initialization
│   │   └── scrcpy.rs          # Scrcpy integration
│   ├── models/
│   │   ├── mod.rs
│   │   ├── device.rs          # Device struct
│   │   └── file.rs            # InstalledFile struct
│   ├── pool/
│   │   ├── mod.rs
│   │   ├── batch_processor.rs # Batch processing
│   │   ├── connection_pool.rs # moka LRU cache
│   │   └── screenshot_cache.rs
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── control.rs         # HTTP routes
│   │   ├── nio.rs             # NIO WebSocket
│   │   └── scrcpy_ws.rs       # Scrcpy WebSocket
│   ├── services/
│   │   ├── mod.rs
│   │   ├── device_detector.rs # USB detection
│   │   ├── device_service.rs  # Device operations
│   │   ├── file_service.rs    # File management
│   │   ├── phone_service.rs   # Device lifecycle
│   │   └── scrcpy_manager.rs  # Scrcpy process
│   └── utils/
│       ├── mod.rs
│       ├── hierarchy.rs       # XML → JSON
│       └── host_ip.rs         # Get host IP
├── tests/
│   ├── common/
│   │   └── mod.rs             # Test utilities
│   ├── test_config.rs         # Config tests (3)
│   ├── test_database.rs       # Database tests (8)
│   ├── test_services.rs       # Service tests (7)
│   └── test_server.rs         # E2E tests (20)
└── _bmad-output/              # BMAD artifacts
    ├── project-context.md
    └── architecture.md
```

### Architectural Boundaries

```
┌─────────────────────────────────────────────────────────────┐
│                    HTTP/WebSocket Layer                      │
│                 (routes/control.rs, nio.rs)                  │
├─────────────────────────────────────────────────────────────┤
│                     Service Layer                            │
│        (services/phone_service.rs, device_service.rs)        │
├─────────────────────────────────────────────────────────────┤
│                  Device Integration                          │
│             (device/atx_client.rs, adb.rs, scrcpy.rs)        │
├─────────────────────────────────────────────────────────────┤
│                    Data Layer                                │
│                   (db/sqlite.rs)                             │
└─────────────────────────────────────────────────────────────┘
```

### Feature to Structure Mapping

| Feature | Location | Key Files |
|---------|----------|-----------|
| Device Management | services/, models/ | phone_service.rs, device.rs |
| Screenshot | services/, routes/ | device_service.rs, control.rs, nio.rs |
| Remote Control | routes/, device/ | nio.rs, atx_client.rs |
| UI Hierarchy | utils/, routes/ | hierarchy.rs, control.rs |
| File Management | services/, routes/ | file_service.rs, control.rs |
| Screen Mirroring | device/, routes/ | scrcpy.rs, scrcpy_ws.rs |
| Batch Operations | pool/ | batch_processor.rs |
| Connection Pooling | pool/ | connection_pool.rs |

### Integration Points

**Internal Communication:**
- Routes → Services (function calls)
- Services → Database (via db module)
- Services → AtxClient (via connection pool)

**External Integrations:**
- ATX Agent (HTTP on port 7912/9008)
- ADB (command-line)
- Scrcpy (process spawn)

**Data Flow:**
```
Request → Route → Service → Database/AtxClient → Response
                      ↓
                Connection Pool (moka cache)
```

---

## Architecture Validation Results

### Coherence Validation ✅

**Decision Compatibility:**
All technology choices (Rust, actix-web, tokio, sqlx, moka) are compatible and work together without conflicts.

**Pattern Consistency:**
Implementation patterns (snake_case, service layer, Result<T, String>) align with Rust best practices and existing codebase conventions.

**Structure Alignment:**
Project structure supports all architectural decisions with clear module boundaries and integration points.

### Requirements Coverage Validation ✅

| Feature | Architecture Support |
|---------|---------------------|
| Device Management | ✅ PhoneService + Database |
| Screenshot | ✅ DeviceService + WebSocket |
| Remote Control | ✅ NIO WebSocket + AtxClient |
| UI Hierarchy | ✅ hierarchy.rs + inspector endpoint |
| File Management | ✅ FileService + SQLite |
| Screen Mirroring | ✅ scrcpy.rs + scrcpy_ws.rs |
| Batch Operations | ✅ batch_processor.rs |

### Implementation Readiness Validation ✅

**Decision Completeness:**
- 15 architectural categories documented
- All critical versions specified
- Technology rationale provided

**Structure Completeness:**
- Complete project tree documented
- All modules and files mapped
- Integration points defined

**Pattern Completeness:**
- Naming conventions for all code types
- Error handling pattern established
- Service creation pattern defined

### Gap Analysis Results

| Priority | Finding | Status |
|----------|---------|--------|
| Critical | None | ✅ No gaps |
| Important | None | ✅ No gaps |
| Nice-to-Have | API versioning | Deferred (single version) |

### Architecture Completeness Checklist

**✅ Requirements Analysis**
- [x] Project context analyzed
- [x] Scale and complexity assessed
- [x] Technical constraints identified
- [x] Cross-cutting concerns mapped

**✅ Architectural Decisions**
- [x] 15 critical decisions documented
- [x] Technology stack specified
- [x] Integration patterns defined
- [x] Performance addressed

**✅ Implementation Patterns**
- [x] Naming conventions established
- [x] Structure patterns defined
- [x] Communication patterns specified
- [x] Process patterns documented

**✅ Project Structure**
- [x] Directory structure defined
- [x] Component boundaries established
- [x] Integration points mapped
- [x] Feature mapping complete

### Architecture Readiness Assessment

**Overall Status:** ✅ READY FOR IMPLEMENTATION
**Confidence Level:** HIGH

**Key Strengths:**
- Existing codebase provides implementation examples
- 90 tests document expected behavior
- Clear service layer separation
- Documented connection pooling

**Areas for Future Enhancement:**
- API versioning (when multiple versions needed)
- Rate limiting (if scaling required)
- Metrics/monitoring integration

### AI Agent Implementation Guidelines

1. Follow all naming conventions exactly (snake_case for files/functions, PascalCase for structs)
2. Use `Result<T, String>` for service layer errors
3. Create services per-request, not as singletons
4. Use connection pool for AtxClient instances
5. Place unit tests in same file with `#[cfg(test)]`
6. Use `tokio::test` for async tests, `actix_web::test` for route tests
