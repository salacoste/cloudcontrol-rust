# Architecture

## System Overview

CloudControl-Rust follows a layered architecture with clear separation of concerns:

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Browser   │────▶│  actix-web  │────▶│   SQLite    │
└─────────────┘     └─────────────┘     └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │  AtxClient  │
                    │  (devices)  │
                    └─────────────┘
```

## Layers

### 1. HTTP Layer (routes/)

**Entry:** `src/main.rs`
**Routes:**
- `routes/control.rs` - Main HTTP routes
- `routes/nio.rs` - NIO WebSocket
- `routes/scrcpy_ws.rs` - Scrcpy WebSocket

**Key Handlers:**
- `device_list` - GET /list
- `device_info` - GET /devices/{udid}/info
- `inspector_screenshot` - GET /inspector/{udid}/screenshot
- `inspector_touch` - POST /inspector/{udid}/touch
- `wifi_connect` - POST /api/wifi-connect

### 2. Service Layer (services/)

**Purpose:** Business logic and device operations

**Key Services:**
| Service | File | Purpose |
|---------|------|---------|
| PhoneService | phone_service.rs | Device lifecycle (on_connected, offline, update) |
| DeviceService | device_service.rs | Screenshot, hierarchy, operations |
| FileService | file_service.rs | File upload/install tracking |
| DeviceDetector | device_detector.rs | USB device auto-detection |

### 3. Data Layer (db/, models/)

**Database:** `db/sqlite.rs`
- SQLite with sqlx
- Compile-time query verification
- MongoDB-style field names for API compatibility

**Models:**
- `Device` - Device state and metadata
- `InstalledFile` - Uploaded file tracking

### 4. Infrastructure Layer (pool/, state/)

**Connection Pool:** `pool/connection_pool.rs`
- moka LRU cache for AtxClient connections
- 1200 max capacity, 600s idle timeout

**Batch Processor:** `pool/batch_processor.rs`
- Event-driven batch processing
- Configurable batch size and flush interval

**Screenshot Cache:** `pool/screenshot_cache.rs`
- Request deduplication
- Concurrent request handling

### 5. Device Integration (device/)

**ADB Wrapper:** `device/adb.rs`
- Command execution (devices, forward, screencap, shell)
- USB serial detection

**ATX Client:** `device/atx_client.rs`
- HTTP client for uiautomator2 agent
- Screenshot, touch, input, keyevent, hierarchy

**Scrcpy:** `device/scrcpy.rs`
- Screen mirroring via scrcpy binary
- Video stream parsing

## Data Flow

### Screenshot Request Flow

```
Browser → GET /inspector/{udid}/screenshot
    → routes/control.rs
    → get_device_client()
        → device_info_cache lookup
        → PhoneService.query_info_by_udid()
        → resolve_device_connection()
            → WiFi: use IP directly
            → USB: ADB forward → 127.0.0.1:port
        → ConnectionPool.get_or_create()
    → AtxClient.screenshot_scaled() OR DeviceService.screenshot_usb_jpeg()
    → JPEG bytes → base64 JSON response
```

### WebSocket Screenshot Streaming

```
Browser → WebSocket /nio/{udid}/ws
    → routes/nio.rs
    → Session loop:
        ← subscribe {target: "screenshot"}
        → Spawn screenshot task:
            → AtxClient.screenshot_scaled() (primary)
            → DeviceService.screenshot_usb_jpeg() (fallback)
            → session.binary(jpeg_bytes)
            → Sleep for interval - elapsed
        ← unsubscribe → abort task
```

## Key Design Decisions

### 1. SQLite over MongoDB
- Original Python version used MongoDB
- Migrated to SQLite for simplicity and zero-config
- Maintains MongoDB-style field names for API compatibility

### 2. Connection Pooling
- moka LRU cache prevents reconnection overhead
- Automatic eviction after 600s idle
- Arc-wrapped AtxClient for thread safety

### 3. USB vs WiFi Resolution
```rust
// WiFi: use IP directly
if ip != "127.0.0.1" { return (ip, port); }

// Already forwarded: use as-is
if ip == "127.0.0.1" && port != 9008 { return (ip, port); }

// USB: requires ADB forward
Adb::forward(serial, 9008) → (127.0.0.1, local_port)
```

### 4. Startup Behavior
- Clear devices table on startup (matches Python behavior)
- USB detector runs in background
- Templates loaded at startup

## Concurrency Model

```rust
// AppState is Clone for actix-web handlers
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub connection_pool: Arc<ConnectionPool>,
    pub device_info_cache: Cache<String, Value>,
    pub heartbeat_sessions: Arc<DashMap<String, HeartbeatSession>>,
}
```

- **Arc** for shared ownership (connection_pool, sessions)
- **DashMap** for concurrent session tracking
- **moka Cache** for thread-safe caching

## Error Handling

```rust
// Service layer: Result<T, String>
pub async fn screenshot_base64(...) -> Result<String, String>

// HTTP layer: HttpResponse
HttpResponse::Ok().json(data)
HttpResponse::InternalServerError().json(json!({"error": msg}))
```
