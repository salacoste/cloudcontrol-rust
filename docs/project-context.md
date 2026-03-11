# CloudControl-Rust: Project Context

## 1. Project Overview

CloudControl-Rust is a WiFi-based Android device farm control and monitoring platform, rewritten from the original Python [CloudControl](https://github.com/ZSJnbu/CloudControl) project into Rust. It provides a web-based dashboard for managing, monitoring, and remotely controlling multiple Android devices simultaneously -- targeting QA teams, CI/CD pipelines, and device lab operators.

The server discovers Android devices over USB and WiFi, communicates with them via the ATX Agent (uiautomator2) HTTP protocol and ADB, streams real-time screenshots over WebSocket, and supports batch operations across device groups. A versioned REST API (`/api/v1/`) enables headless CI/CD integration.

**Current status**: All 7 epics and 39 stories are complete (MVP + Growth + Post-MVP phases). See `_bmad-output/implementation-artifacts/sprint-status.yaml` for the full breakdown.

---

## 2. Architecture

### Layered Design

```
┌─────────────────────────────────────────────────────────┐
│                    Frontend (Tera HTML + JS)             │
│  index.html · remote.html · device_synchronous.html     │
├─────────────────────────────────────────────────────────┤
│                    Route Layer (actix-web)               │
│  control.rs · api_v1.rs · nio.rs · recording.rs         │
│  scrcpy.rs · scrcpy_ws.rs · batch_report.rs             │
├─────────────────────────────────────────────────────────┤
│                    Service Layer                         │
│  phone_service · device_service · device_detector       │
│  wifi_discovery · recording_service · scrcpy_manager    │
│  file_service                                           │
├─────────────────────────────────────────────────────────┤
│                    Device Layer                          │
│  atx_client (HTTP/JSON-RPC) · adb (CLI) · scrcpy       │
│  atx_init (u2 server bootstrap)                         │
├─────────────────────────────────────────────────────────┤
│              Infrastructure / Shared State               │
│  AppState · ConnectionPool · ScreenshotCache · Database │
│  MetricsTracker · RecordingState                        │
└─────────────────────────────────────────────────────────┘
```

### Data Flow

1. **Device discovery**: `DeviceDetector` polls `adb devices` every 1s; `WifiDiscovery` scans the local subnet (ports 7912/9008) every 30s. Both register devices via `PhoneService.on_connected()`.
2. **Device communication**: `AtxClient` sends HTTP/JSON-RPC requests to the ATX Agent running on each device (port 9008 for new u2.jar, 7912 for legacy atx-agent). ADB is the fallback for USB-only operations.
3. **Screenshot pipeline**: `AtxClient.screenshot()` fetches JPEG via JSON-RPC `takeScreenshot` -> `DeviceService` handles resize/recompress -> `ScreenshotCache` deduplicates concurrent requests -> delivered as base64 JSON or raw binary over WebSocket.
4. **Control commands**: Touch/swipe/input/keyevent go through `AtxClient` (ATX-first) with ADB shell fallback. Fire-and-forget for touch; acknowledged for text input.
5. **WebSocket channels**: NIO multiplexes screenshot streaming + bidirectional control on a single connection per device. Scrcpy WebSocket relays H.264 video frames.

---

## 3. Tech Stack

| Component | Technology | Version | Purpose |
|-----------|-----------|---------|---------|
| Language | Rust | 2021 Edition | Systems language |
| Web framework | actix-web | 4.x | HTTP server + WebSocket |
| Async runtime | tokio | 1.x (full) | Async I/O |
| Database | SQLite via sqlx | 0.8 | Device state persistence |
| Template engine | Tera | 1.x | Server-side HTML rendering |
| HTTP client | reqwest | 0.12 | ATX Agent communication |
| Connection cache | moka | 0.12 | LRU pool for AtxClient instances |
| Concurrent map | dashmap | 6 | Heartbeat sessions, screenshot cache |
| Image processing | image | 0.25 | JPEG resize (Nearest filter) |
| XML parsing | quick-xml | 0.37 | UI hierarchy dump |
| Logging | tracing + tracing-subscriber | 0.1 / 0.3 | Structured logging to stdout + daily rolling file |
| Serialization | serde / serde_json / serde_yml | 1.x | JSON API + YAML config |

### External Dependencies (on-device)

- **ATX Agent / u2.jar**: UiAutomator2 Java server running on Android (port 9008). Provides screenshot, touch, swipe, input, keyevent, and UI hierarchy over HTTP JSON-RPC.
- **ADB**: Android Debug Bridge CLI. Used for USB device detection, port forwarding, shell commands, screencap fallback, and scrcpy bootstrapping.
- **scrcpy-server.jar**: Bundled at `resources/scrcpy/scrcpy-server.jar` (v2.7). Pushed to device for H.264 screen mirroring.

---

## 4. Key Components

### 4.1 Device Management

**Discovery** -- Two parallel discovery mechanisms run on startup:

- `src/services/device_detector.rs` -- Polls `adb devices` every 1 second. For each new serial, fetches device properties via ADB shell, initializes the ATX server via `AtxInit`, and registers the device through `PhoneService`.
- `src/services/wifi_discovery.rs` -- Scans all /24 subnets on the host's network interfaces. Probes ports 7912 and 9008 with up to 50 concurrent HTTP requests (500ms timeout). Tracks known devices and marks them offline after 3 consecutive failed probes.

**ATX Agent Protocol** -- `src/device/atx_client.rs` is the core device communication client. It wraps HTTP requests to the ATX Agent's JSON-RPC interface:
- `screenshot()` -- JSON-RPC `takeScreenshot` returning base64 JPEG
- `touch(x, y)` -- Fire-and-forget click
- `swipe(x1, y1, x2, y2, duration)` -- Gesture with configurable duration
- `input_text(text)` -- Text input via IME
- `press_key(key)` -- Physical key simulation
- `dump_hierarchy()` -- UI element tree as XML

**Connection Pool** -- `src/pool/connection_pool.rs` uses a moka LRU cache (capacity 1200, 600s idle timeout) to reuse `AtxClient` instances across requests.

**State Persistence** -- `src/services/phone_service.rs` manages device lifecycle with SQLite persistence. On startup, `restore_devices()` reloads previously known devices. The database auto-recovers from corruption by creating a timestamped backup and reinitializing.

### 4.2 Real-Time Monitoring

**Screenshot Streaming** -- The NIO WebSocket (`src/routes/nio.rs`) provides a multiplexed channel per device. Clients send `{"type": "screenshot"}` to request frames; the server responds with base64 JPEG. Concurrent screenshot requests for the same device are deduplicated by `src/pool/screenshot_cache.rs` (20-entry cache, 300ms TTL).

**WebSocket Protocol** -- NIO messages follow the format `{"type": "<event>", "data": {...}, "id": "<correlation>"}`. Supported event types: `screenshot`, `touch`, `swipe`, `input`, `keyevent`, `subscribe`, `unsubscribe`.

**API WebSocket** -- `/api/v1/ws/screenshot/{udid}` provides a dedicated screenshot streaming endpoint. `/api/v1/ws/nio` provides a JSON-RPC WebSocket for CI/CD integration.

**Metrics** -- `src/state.rs` contains `MetricsTracker` which records screenshot latency percentiles (last 1000 samples) and active WebSocket connection counts, exposed via `GET /api/v1/metrics`.

### 4.3 Remote Control

All control operations use an ATX-first strategy with ADB fallback:

- **Touch/Tap**: `POST /inspector/{udid}/touch` -- ATX `click(x, y)`, fire-and-forget (no response wait)
- **Swipe**: `POST /inspector/{udid}/touch` with swipe params -- ATX `swipe(x1, y1, x2, y2, duration)`
- **Text Input**: `POST /inspector/{udid}/input` -- ATX `send_keys()` or ADB `input text`
- **Key Events**: `POST /inspector/{udid}/keyevent` -- ATX `press()` with key validation (home, back, menu, power, volume_up, volume_down, etc.)
- **Shell**: `POST /api/devices/{udid}/shell` -- Direct ADB shell command execution
- **UI Hierarchy**: `GET /inspector/{udid}/hierarchy` -- ATX XML dump parsed to JSON via `src/utils/hierarchy.rs`

### 4.4 Batch Operations

- `POST /api/batch/tap` -- Synchronized tap across multiple devices
- `POST /api/batch/swipe` -- Synchronized swipe across multiple devices
- `POST /api/batch/input` -- Synchronized text input across multiple devices
- `POST /api/screenshot/batch` -- Parallel screenshot capture

Batch operations execute concurrently via `tokio::spawn` and return per-device success/failure results. Maximum batch size is 20 devices (enforced in `src/routes/api_v1.rs`).

### 4.5 Recording & Playback

`src/services/recording_service.rs` provides action recording and playback:

- **Recording**: Start a session on a device, record tap/swipe/input/keyevent actions with timestamps and coordinates. Sessions can be paused, resumed, or cancelled.
- **Playback**: Replay recorded actions on any device with configurable speed multiplier. Supports pause/resume/stop during playback.
- **Management**: List, get, edit, and delete recordings. Edit individual actions within a recording.

Data is persisted in SQLite (`recordings` and `recording_actions` tables).

### 4.6 External API (v1)

`src/routes/api_v1.rs` provides a versioned REST API for CI/CD integration:

- Standardized response format: `{"status": "success|error", "data": {...}, "timestamp": "..."}`
- Error codes: `ERR_DEVICE_NOT_FOUND`, `ERR_DEVICE_DISCONNECTED`, `ERR_INVALID_REQUEST`, `ERR_OPERATION_FAILED`, `ERR_NO_DEVICES_SELECTED`, `ERR_BATCH_PARTIAL_FAILURE`
- OpenAPI spec: `GET /api/v1/openapi.json`
- Health check: `GET /api/v1/health`
- Metrics: `GET /api/v1/metrics`

### 4.7 Screen Mirroring (scrcpy)

`src/device/scrcpy.rs` + `src/services/scrcpy_manager.rs` provide high-fidelity H.264 screen mirroring:

1. Push `scrcpy-server.jar` (v2.7) to device via ADB
2. Launch scrcpy server process, establish video + control TCP streams
3. Parse H.264 NAL units from the video stream
4. Relay frames over WebSocket (`/scrcpy/{udid}/ws`) to browser clients
5. Support device control through scrcpy's control protocol (tap, key, swipe)
6. Session recording: capture raw H.264 stream to file

Session management endpoints: start, stop, list sessions. Recording endpoints: start/stop recording, list/download/delete recordings.

---

## 5. Directory Structure

```
cloudcontrol-rust/
├── Cargo.toml                          # Dependencies and build config (LTO enabled for release)
├── config/
│   └── default_dev.yaml                # Server port, SQLite config, legacy service refs
├── src/
│   ├── main.rs                         # Entry point: logging, config, DB, detectors, HTTP server
│   ├── lib.rs                          # Module re-exports
│   ├── config.rs                       # AppConfig deserialization from YAML
│   ├── error.rs                        # 404/500 error page handlers (Tera templates)
│   ├── state.rs                        # AppState (shared across all handlers), MetricsTracker
│   ├── db/
│   │   ├── mod.rs                      # Re-exports Database
│   │   └── sqlite.rs                   # SQLite ops: CRUD, field mapping, schema init, corruption recovery
│   ├── device/
│   │   ├── mod.rs
│   │   ├── adb.rs                      # ADB CLI wrapper (list, shell, forward, push, screencap, connect)
│   │   ├── atx_client.rs              # ATX Agent HTTP client (screenshot, touch, swipe, input, key, hierarchy)
│   │   ├── atx_init.rs                # UiAutomator2 server bootstrap on device
│   │   └── scrcpy.rs                  # Scrcpy session: JAR push, server launch, H.264 stream parsing
│   ├── models/
│   │   ├── mod.rs
│   │   ├── device.rs                  # Device struct (maps SQLite columns to JSON API fields)
│   │   ├── file.rs                    # File upload model
│   │   ├── recording.rs              # RecordingSession, RecordedAction, ActionType, playback models
│   │   ├── api_response.rs           # ApiResponse<T> wrapper, request/response types, error codes
│   │   ├── batch_report.rs           # Batch operation report model
│   │   └── openapi.rs                # OpenAPI 3.0 spec generation
│   ├── pool/
│   │   ├── mod.rs
│   │   ├── connection_pool.rs         # Moka LRU cache of Arc<AtxClient> (1200 capacity, 600s TTL)
│   │   ├── screenshot_cache.rs        # DashMap cache with request deduplication (300ms TTL)
│   │   └── batch_processor.rs         # Concurrent batch operation executor
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── control.rs                 # Primary HTTP routes: pages, device CRUD, screenshot, touch, upload
│   │   ├── api_v1.rs                  # Versioned REST API: devices, control, batch, health, metrics, WS
│   │   ├── nio.rs                     # NIO WebSocket: multiplexed screenshot + control per device
│   │   ├── recording.rs              # Recording/playback REST endpoints
│   │   ├── scrcpy.rs                 # Scrcpy session + recording management endpoints
│   │   ├── scrcpy_ws.rs              # Scrcpy H.264 WebSocket relay
│   │   └── batch_report.rs           # Batch report listing/retrieval/deletion
│   ├── services/
│   │   ├── mod.rs
│   │   ├── phone_service.rs           # Device lifecycle: connect, disconnect, restore, query, tags, history
│   │   ├── device_service.rs          # High-level ops: screenshot (ATX + USB paths), resize, recompress
│   │   ├── device_detector.rs         # USB auto-detection: polls adb devices every 1s
│   │   ├── wifi_discovery.rs          # WiFi subnet scanning: probes ATX ports every 30s
│   │   ├── recording_service.rs       # Recording + playback state machine (SQLite-backed)
│   │   ├── scrcpy_manager.rs          # Scrcpy session lifecycle + H.264 recording
│   │   └── file_service.rs            # APK/file upload handling
│   └── utils/
│       ├── mod.rs
│       ├── hierarchy.rs               # XML UI hierarchy -> JSON tree conversion
│       └── host_ip.rs                 # Host IP detection + subnet enumeration
├── resources/
│   ├── templates/                     # Tera HTML templates (index, remote, async, file, 404, 500, etc.)
│   ├── static/
│   │   ├── js/
│   │   │   ├── remote.js             # Remote control UI logic (touch, swipe, screenshot polling)
│   │   │   ├── remote_synchronous.js  # Synchronized multi-device control page
│   │   │   ├── nio-client.js          # NIO WebSocket client
│   │   │   ├── scrcpy-client.js       # Scrcpy H.264 decoder + WebSocket client
│   │   │   └── common.js             # Shared utilities
│   │   ├── css/                       # Bootstrap, UIKit, custom themes
│   │   └── server                     # ATX agent binary (legacy)
│   └── scrcpy/
│       └── scrcpy-server.jar          # Scrcpy server v2.7 (pushed to devices)
├── tests/
│   ├── common/mod.rs                  # Test helpers: create_temp_db(), make_device_json()
│   ├── test_config.rs                 # Configuration loading tests (3 tests)
│   ├── test_database.rs               # SQLite CRUD tests (8 tests)
│   ├── test_services.rs               # PhoneService tests (7 tests)
│   └── test_server.rs                 # E2E HTTP tests (20+ tests)
├── database/                          # SQLite database files (created at runtime)
├── log/                               # Daily rolling log files
└── docs/                              # Additional documentation
```

---

## 6. API Endpoints

### Page Routes

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Device grid dashboard |
| GET | `/devices/{udid}/remote` | Single-device remote control page |
| GET/POST | `/async` | Multi-device synchronous control page |
| GET | `/installfile` | File/APK management page |
| GET | `/files` | Uploaded files listing |

### Device API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/list` | List all devices (JSON) |
| GET | `/devices/{udid}/info` | Device metadata |
| POST | `/api/wifi-connect` | Connect to WiFi device by IP |
| POST | `/api/devices/add` | Manually add device |
| DELETE | `/api/devices/{udid}` | Disconnect device |
| POST | `/api/devices/{udid}/reconnect` | Reconnect device |
| POST | `/api/devices/{udid}/tags` | Add tags to device |
| DELETE | `/api/devices/{udid}/tags/{tag}` | Remove tag |
| GET | `/api/devices/{udid}/history` | Connection history |
| GET | `/api/devices/{udid}/stats` | Connection statistics |

### Screenshot & Control

| Method | Path | Description |
|--------|------|-------------|
| GET | `/inspector/{udid}/screenshot` | Screenshot as base64 JSON |
| GET | `/inspector/{udid}/screenshot/img` | Screenshot as raw JPEG |
| POST | `/inspector/{udid}/touch` | Tap or swipe |
| POST | `/inspector/{udid}/input` | Text input |
| POST | `/inspector/{udid}/keyevent` | Key event (home, back, etc.) |
| GET | `/inspector/{udid}/hierarchy` | UI hierarchy (JSON) |
| POST | `/inspector/{udid}/upload` | Push file to device |
| POST | `/api/devices/{udid}/shell` | Execute shell command |

### Batch Operations

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/batch/tap` | Batch tap (max 20 devices) |
| POST | `/api/batch/swipe` | Batch swipe |
| POST | `/api/batch/input` | Batch text input |
| POST | `/api/screenshot/batch` | Batch screenshot |
| GET | `/api/batch/reports` | List batch reports |
| GET | `/api/batch/reports/{id}` | Get batch report |
| DELETE | `/api/batch/reports/{id}` | Delete batch report |

### Recording & Playback

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/recordings/start` | Start recording session |
| POST | `/api/recordings/{id}/action` | Record an action |
| POST | `/api/recordings/{id}/stop` | Stop recording |
| POST | `/api/recordings/{id}/pause` | Pause recording |
| POST | `/api/recordings/{id}/resume` | Resume recording |
| POST | `/api/recordings/{id}/cancel` | Cancel recording |
| GET | `/api/recordings/{id}/status` | Recording status |
| GET | `/api/recordings` | List all recordings |
| GET | `/api/recordings/{id}` | Get recording with actions |
| DELETE | `/api/recordings/{id}` | Delete recording |
| PUT | `/api/recordings/{id}/actions/{action_id}` | Edit action |
| DELETE | `/api/recordings/{id}/actions/{action_id}` | Delete action |
| POST | `/api/recordings/{id}/play` | Start playback |
| GET | `/api/recordings/{id}/playback/status` | Playback status |
| POST | `/api/recordings/{id}/playback/stop` | Stop playback |
| POST | `/api/recordings/{id}/playback/pause` | Pause playback |
| POST | `/api/recordings/{id}/playback/resume` | Resume playback |

### API v1 (CI/CD Integration)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/devices` | List devices (standardized response) |
| GET | `/api/v1/devices/{udid}` | Get device info |
| GET | `/api/v1/devices/{udid}/screenshot` | Screenshot |
| POST | `/api/v1/devices/{udid}/tap` | Tap |
| POST | `/api/v1/devices/{udid}/swipe` | Swipe |
| POST | `/api/v1/devices/{udid}/input` | Text input |
| POST | `/api/v1/devices/{udid}/keyevent` | Key event |
| POST | `/api/v1/batch/tap` | Batch tap |
| POST | `/api/v1/batch/swipe` | Batch swipe |
| POST | `/api/v1/batch/input` | Batch input |
| GET | `/api/v1/status` | Device status summary |
| GET | `/api/v1/health` | Health check |
| GET | `/api/v1/metrics` | Latency + connection metrics |
| GET | `/api/v1/openapi.json` | OpenAPI 3.0 spec |

### WebSocket Endpoints

| Path | Description |
|------|-------------|
| `/nio/{udid}/ws` | NIO: multiplexed screenshot streaming + device control |
| `/nio/stats` | NIO connection statistics |
| `/api/v1/ws/screenshot/{udid}` | API: dedicated screenshot streaming |
| `/api/v1/ws/nio` | API: JSON-RPC WebSocket |
| `/devices/{udid}/shell` | Interactive ADB shell |
| `/scrcpy/{udid}/ws` | Scrcpy H.264 video relay |
| `/feeds` | Global event feed (stub) |

### Scrcpy Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/scrcpy/{udid}/start` | Start scrcpy session |
| POST | `/scrcpy/{udid}/stop` | Stop scrcpy session |
| GET | `/scrcpy/sessions` | List active sessions |
| GET | `/scrcpy/{udid}/status` | Session status |
| POST | `/scrcpy/{udid}/tap` | Tap via scrcpy control |
| POST | `/scrcpy/{udid}/key` | Key via scrcpy control |
| POST | `/scrcpy/{udid}/swipe` | Swipe via scrcpy control |
| POST | `/scrcpy/{udid}/recording/start` | Start H.264 recording |
| POST | `/scrcpy/{udid}/recording/stop` | Stop recording |
| GET | `/scrcpy/recordings` | List recordings |
| GET | `/scrcpy/recordings/{id}` | Get recording metadata |
| GET | `/scrcpy/recordings/{id}/download` | Download recording file |
| DELETE | `/scrcpy/recordings/{id}` | Delete recording |

---

## 7. Development Setup

### Prerequisites

- Rust toolchain (2021 edition) -- install via [rustup](https://rustup.rs/)
- ADB installed and on `PATH` (`android-platform-tools`)
- Android devices with UiAutomator2 initialized: `python3 -m uiautomator2 init --serial <serial>`
- (Optional) scrcpy for H.264 mirroring

### Build & Run

```bash
# Development build
cargo build

# Production build (LTO enabled, opt-level 3)
cargo build --release

# Run (defaults to port 8000)
cargo run

# Run with custom log level
RUST_LOG=cloudcontrol=debug cargo run

# Access dashboard
open http://localhost:8000
```

### Configuration

Edit `config/default_dev.yaml`:

```yaml
server:
  port: 8000

db_configs:
  type: sqlite
  db_name: cloudcontrol.db
```

### Testing

```bash
# All tests
cargo test

# Unit tests only
cargo test --lib

# Integration/E2E tests
cargo test --test test_server
cargo test --test test_database
cargo test --test test_services
cargo test --test test_config
```

### Startup Sequence

1. Initialize tracing (stdout + daily rolling `log/app.log`)
2. Load `config/default_dev.yaml`
3. Initialize SQLite database (auto-create tables, recover from corruption)
4. Restore persisted devices from database
5. Create connection pool (1200 max, 600s idle timeout)
6. Load Tera templates from `resources/templates/`
7. Detect host IP address
8. Build `AppState` (shared across all handlers)
9. Start USB device detector (1s polling interval)
10. Start WiFi discovery (30s scan interval)
11. Bind HTTP server to `0.0.0.0:{port}`

---

## 8. Key Design Decisions

### ATX-First with ADB Fallback

The ATX Agent (uiautomator2 Java server on port 9008) is the primary device communication channel. It provides lower-latency screenshot capture, more reliable touch/input, and richer device info than raw ADB. ADB is used as fallback for USB-only devices and for operations ATX doesn't support (shell commands, file push, port forwarding, screencap).

### Fire-and-Forget Touch

Touch and swipe commands are sent without waiting for the ATX response to complete. This minimizes perceived latency in the remote control UI -- the next screenshot will show the result. Text input and key events do wait for acknowledgment since they affect state.

### NIO WebSocket Multiplexing

Instead of separate connections for screenshots and control, the NIO WebSocket multiplexes both on a single connection per device. This reduces connection overhead and simplifies client-side state management. The message protocol uses `{type, data, id}` for request/response correlation.

### Screenshot Deduplication

The `ScreenshotCache` prevents thundering-herd problems when multiple clients request screenshots for the same device simultaneously. A `watch` channel is used so concurrent requesters share a single in-flight screenshot capture (300ms TTL cache).

### Connection Pool with Moka LRU

`AtxClient` instances are expensive to create (HTTP client setup, DNS resolution). The moka-based connection pool reuses clients across requests with a 600-second idle timeout, supporting up to 1200 concurrent device connections.

### SQLite over MongoDB

The original Python project used MongoDB. The Rust rewrite uses SQLite for zero-dependency deployment. The database layer maintains a MongoDB-style field mapping (`FIELD_MAPPING` in `src/db/sqlite.rs`) for API compatibility with the original frontend.

### Image Resize with Nearest Filter

Screenshots are resized using `image::imageops::FilterType::Nearest` instead of `Lanczos3` for speed. Quality is acceptable for monitoring dashboards where screenshots refresh multiple times per second.

### Device State Persistence

Devices persist across server restarts. On startup, `PhoneService.restore_devices()` reloads all known devices from SQLite and attempts to re-establish connections. The database includes automatic corruption recovery with timestamped backups.

### Scrcpy H.264 Relay

For high-fidelity screen mirroring, the server bootstraps a scrcpy session (push JAR, forward port, launch server), receives raw H.264 NAL units over TCP, and relays them to browser clients over WebSocket. The browser uses a JS H.264 decoder for playback. This provides smoother video than the screenshot-polling approach.

### USB vs WiFi Resolution

```
WiFi device:      ip != "127.0.0.1"  -> connect directly to device IP
ADB-forwarded:    ip == "127.0.0.1" && port != 9008  -> use forwarded port
USB device:       requires `adb forward` to create local port mapping
```

---

*Generated: 2026-03-10 | 39 stories complete | SQLite | actix-web 4 | Rust 2021*
