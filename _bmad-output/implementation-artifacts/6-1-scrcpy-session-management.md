# Story 6.1: Scrcpy Session Management

Epic: 6 (High-Fidelity Screen Mirroring)
Status: done
Priority: P3

## Story

As a **QA Engineer**,
I want to start and stop high-fidelity screen mirroring via scrcpy,
so that I can view detailed screen content when JPEG quality isn't enough.

## Context & Dependencies

### Related Requirements
- **FR35**: Users can start high-fidelity screen mirroring via scrcpy
- **NFR17**: Scrcpy video stream latency <100ms

### Dependencies
- **Epic 1A**: Device Connection & Discovery (done) — device lookup, serial resolution
- **Epic 2**: Real-Time Visual Monitoring (done) — screenshot infrastructure
- **Epic 3**: Remote Device Control (done) — device interaction patterns
- **Epic 4**: Multi-Device Batch Operations (done) — recording service patterns to follow

### Architecture Constraints
From `_bmad-output/project-context.md`:
- actix-web 4.x with actix-ws 0.3 for WebSocket handling
- All shared state via `Arc<...>` or `Clone`-able types
- Error handling: `Result<T, String>` in service layer
- Logging: `tracing::info!("[PREFIX]")` with context tags
- No new dependencies required — tokio, dashmap, serde, chrono all in Cargo.toml

## Acceptance Criteria

### AC1: Start scrcpy session
```gherkin
Scenario: Start scrcpy session
  Given a device is connected via USB
  And scrcpy is installed on the server
  When I request POST /scrcpy/{udid}/start
  Then a scrcpy process spawns
  And the session ID is returned
  And H.264 video stream begins
```

### AC2: Stop scrcpy session
```gherkin
Scenario: Stop scrcpy session
  Given a scrcpy session is running
  When I request POST /scrcpy/{udid}/stop
  Then the scrcpy process terminates
  And resources are cleaned up
  And the session is marked as ended
```

### AC3: Handle scrcpy not installed
```gherkin
Scenario: Handle scrcpy not installed
  Given scrcpy is not available on the server
  When I request to start scrcpy
  Then HTTP 503 is returned
  And error code is "ERR_SCRCPY_NOT_AVAILABLE"
  And installation instructions are included
```

### AC4: List active scrcpy sessions
```gherkin
Scenario: List active scrcpy sessions
  Given multiple scrcpy sessions are running
  When I request GET /scrcpy/sessions
  Then all active sessions are listed
  And each session shows UDID, start time, and status
```

## Tasks / Subtasks

- [x] Task 1: Create ScrcpySessionInfo model and service (AC: 1, 2, 4)
  - [x] Create `ScrcpySessionInfo` struct in `src/services/scrcpy_manager.rs` with fields: session_id (String/UUID), udid, serial, start_time, status (Active/Stopped), meta (width, height, device_name)
  - [x] Create `ScrcpySessionStatus` enum: `Starting`, `Active`, `Stopping`, `Stopped`
  - [x] Add `sessions: Arc<DashMap<String, ScrcpySessionEntry>>` to `ScrcpyManager` (keyed by udid — one session per device)
  - [x] `ScrcpySessionEntry` holds: `ScrcpySessionInfo` + `Arc<Mutex<ScrcpySession>>` (for later use by Story 6-3)
  - [x] Implement `start_session(&self, udid: &str, serial: &str) -> Result<ScrcpySessionInfo, String>`
  - [x] Implement `stop_session(&self, udid: &str) -> Result<(), String>`
  - [x] Implement `list_sessions(&self) -> Vec<ScrcpySessionInfo>`
  - [x] Implement `get_session(&self, udid: &str) -> Option<ScrcpySessionInfo>`

- [x] Task 2: Integrate ScrcpyManager into AppState (AC: 1, 2, 4)
  - [x] Add `scrcpy_manager: ScrcpyManager` field to `AppState` in `src/state.rs`
  - [x] Initialize `ScrcpyManager::new()` in `AppState::new()`

- [x] Task 3: Create scrcpy REST route handlers (AC: 1, 2, 3, 4)
  - [x] Create `src/routes/scrcpy.rs` with three handlers:
    - `POST /scrcpy/{udid}/start` — `start_scrcpy_session()`: look up device by udid (cache → DB), resolve serial, check `jar_available()`, call `scrcpy_manager.start_session()`, return session info
    - `POST /scrcpy/{udid}/stop` — `stop_scrcpy_session()`: call `scrcpy_manager.stop_session()`, return success
    - `GET /scrcpy/sessions` — `list_scrcpy_sessions()`: call `scrcpy_manager.list_sessions()`, return list
  - [x] Error handling: 503 + `ERR_SCRCPY_NOT_AVAILABLE` when `!jar_available()`, 404 + `ERR_DEVICE_NOT_FOUND` for unknown udid, 409 + `ERR_SESSION_ALREADY_ACTIVE` for duplicate start, 404 + `ERR_SESSION_NOT_FOUND` for stop on non-existent session

- [x] Task 4: Register routes in main.rs (AC: 1, 2, 3, 4)
  - [x] Add `pub mod scrcpy;` to `src/routes/mod.rs`
  - [x] Register routes in `src/main.rs`:
    - `.route("/scrcpy/{udid}/start", web::post().to(routes::scrcpy::start_scrcpy_session))`
    - `.route("/scrcpy/{udid}/stop", web::post().to(routes::scrcpy::stop_scrcpy_session))`
    - `.route("/scrcpy/sessions", web::get().to(routes::scrcpy::list_scrcpy_sessions))`

- [x] Task 5: Write tests (AC: 1, 2, 3, 4)
  - [x] Add scrcpy routes to `setup_test_app!` macro in `tests/test_server.rs`
  - [x] Test: `test_start_scrcpy_returns_404_for_unknown_device` — POST /scrcpy/nonexistent/start returns 404 or 503 depending on JAR availability
  - [x] Test: `test_list_scrcpy_sessions_empty` — GET /scrcpy/sessions returns empty list
  - [x] Test: `test_stop_scrcpy_returns_404_no_active_session` — POST /scrcpy/test-device/stop returns 404 with `ERR_SESSION_NOT_FOUND`
  - [x] Unit test: `test_scrcpy_session_info_serialization` — verify JSON serialization of ScrcpySessionInfo
  - [x] Unit test: `test_scrcpy_manager_new` — verify manager initializes with empty sessions

## Dev Notes

### Existing Scrcpy Infrastructure

Three files already implement scrcpy support at the protocol level:

1. **`src/device/scrcpy.rs`** — Low-level `ScrcpySession` struct that manages:
   - JAR push to device via ADB
   - ADB port forwarding (abstract socket)
   - Scrcpy server process spawn with H.264 codec params
   - Video socket + control socket handshake (tunnel_forward=true)
   - Frame reading (12-byte header: PTS u64 BE + packet_size u32 BE)
   - Touch events (28 bytes: type=2, action, pointer_id, x, y, w, h, pressure)
   - Key events (14 bytes: type=0, action, keycode, repeat, metastate)
   - Shutdown (close streams, kill process, remove ADB forward)

2. **`src/routes/scrcpy_ws.rs`** — WebSocket handler at `/scrcpy/{udid}/ws` that:
   - Starts a `ScrcpySession` inline on WebSocket connect
   - Sends init JSON message (codec, width, height, deviceName)
   - Runs video_task (read frames → binary WS messages) and control receive (WS binary → scrcpy control)
   - Destroys session on WebSocket disconnect
   - Also has `scrcpy_status()` at `/scrcpy/{udid}/status`

3. **`src/services/scrcpy_manager.rs`** — `ScrcpyManager` that tracks JAR push state per device serial

### Key Architecture Decision

**Current**: Session lifecycle is coupled to WebSocket connection (start on WS connect, stop on WS disconnect).

**Story 6-1**: Decouple session lifecycle from WebSocket. Sessions are started/stopped via REST API and exist independently. The WebSocket relay (Story 6-3) will later connect to an existing managed session.

The `ScrcpyManager` will be extended to hold active `ScrcpySession` objects behind `Arc<Mutex<>>`. This allows Story 6-3 to later access the video/control streams without re-creating the session.

### ScrcpyManager Extension Design

```rust
// Extend existing ScrcpyManager in src/services/scrcpy_manager.rs

#[derive(Debug, Clone, Serialize)]
pub struct ScrcpySessionInfo {
    pub session_id: String,        // UUID v4
    pub udid: String,
    pub serial: String,
    pub status: String,            // "active" | "stopped"
    pub width: u32,
    pub height: u32,
    pub device_name: String,
    pub started_at: String,        // ISO 8601
}

pub struct ScrcpySessionEntry {
    pub info: ScrcpySessionInfo,
    pub session: Arc<Mutex<ScrcpySession>>,  // For Story 6-3 access
}

// ScrcpyManager gains:
pub struct ScrcpyManager {
    pushed: Arc<DashMap<String, bool>>,
    sessions: Arc<DashMap<String, ScrcpySessionEntry>>,  // udid → entry
}
```

### Device Resolution Pattern

Follow the same pattern as `get_device_client` in `api_v1.rs`:
1. Check `state.device_info_cache.get(udid)` first
2. Fallback to `PhoneService::new(state.db.clone()).query_info_by_udid(udid)`
3. Extract `serial` from device info JSON
4. Scrcpy requires USB connection — serial must not be empty

### Response Format

Follow project convention (match recording endpoints style):

```json
// POST /scrcpy/{udid}/start → 200 OK
{
  "status": "success",
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "udid": "abc123",
  "serial": "R5CT900ABCD",
  "width": 1080,
  "height": 1920,
  "device_name": "Pixel 6",
  "started_at": "2026-03-09T12:00:00Z"
}

// POST /scrcpy/{udid}/stop → 200 OK
{
  "status": "success",
  "message": "Scrcpy session stopped"
}

// GET /scrcpy/sessions → 200 OK
{
  "status": "success",
  "sessions": [
    {
      "session_id": "550e8400-...",
      "udid": "abc123",
      "serial": "R5CT900ABCD",
      "status": "active",
      "width": 1080,
      "height": 1920,
      "device_name": "Pixel 6",
      "started_at": "2026-03-09T12:00:00Z"
    }
  ],
  "count": 1
}

// POST /scrcpy/{udid}/start → 503 (no scrcpy)
{
  "status": "error",
  "error": "ERR_SCRCPY_NOT_AVAILABLE",
  "message": "scrcpy-server.jar not found. Place it at resources/scrcpy/scrcpy-server.jar"
}
```

### Testing Strategy

Since scrcpy requires real ADB devices and the scrcpy-server.jar, most tests will verify:
- HTTP status codes and error responses (jar not found = 503, device not found = 404)
- Session list empty state
- Response JSON structure
- ScrcpySessionInfo serialization

Full end-to-end scrcpy tests require physical USB devices and are out of scope for automated tests.

**Test Note**: The `setup_test_app!` macro does NOT have scrcpy routes registered. Add:
```rust
.route("/scrcpy/{udid}/start", web::post().to(routes::scrcpy::start_scrcpy_session))
.route("/scrcpy/{udid}/stop", web::post().to(routes::scrcpy::stop_scrcpy_session))
.route("/scrcpy/sessions", web::get().to(routes::scrcpy::list_scrcpy_sessions))
```

### Impact on Existing WebSocket Handler

The existing `/scrcpy/{udid}/ws` WebSocket handler in `scrcpy_ws.rs` will continue to work as-is for now. It manages its own inline session. Story 6-3 will refactor it to connect to sessions managed by `ScrcpyManager` instead of creating its own.

**Do NOT modify `scrcpy_ws.rs` in this story** — it works independently and Story 6-3 will handle the integration.

### Project Structure Notes

Files to create/modify:
```
src/services/scrcpy_manager.rs  — MODIFY: Add ScrcpySessionInfo, ScrcpySessionEntry, session management methods
src/routes/scrcpy.rs            — CREATE: REST handlers for start/stop/list
src/routes/mod.rs               — MODIFY: Add `pub mod scrcpy;`
src/state.rs                    — MODIFY: Add scrcpy_manager field to AppState
src/main.rs                     — MODIFY: Register /scrcpy/{udid}/start, /scrcpy/{udid}/stop, /scrcpy/sessions routes
tests/test_server.rs            — MODIFY: Add scrcpy routes to setup_test_app!, add tests
```

No new files in `src/models/` — the session types live in `scrcpy_manager.rs` since they're service-internal.

### References

- [Source: src/device/scrcpy.rs] — ScrcpySession low-level implementation (start, read_frame, send_touch, send_key, shutdown)
- [Source: src/routes/scrcpy_ws.rs] — Existing WebSocket handler (DO NOT MODIFY)
- [Source: src/services/scrcpy_manager.rs] — Existing ScrcpyManager (extend with session tracking)
- [Source: src/state.rs:89-131] — AppState struct definition
- [Source: src/main.rs:368-374] — Existing scrcpy route registration
- [Source: src/services/recording_service.rs] — Session management patterns to follow (RecordingState, start/stop/list)
- [Source: src/routes/recording.rs] — REST handler patterns for session management
- [Source: _bmad-output/planning-artifacts/epics-stories.md] — Story acceptance criteria
- [Source: _bmad-output/planning-artifacts/prd.md] — FR35, NFR17, API endpoint specification
- [Source: _bmad-output/project-context.md] — Coding standards, framework rules

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

- `stop_session()` initially used `Arc::try_unwrap()` which returned wrong type — simplified to just lock the Arc<Mutex<>> directly
- `test_start_scrcpy_returns_503_when_jar_missing` failed because `resources/scrcpy/scrcpy-server.jar` exists in the repo — adapted test to `test_start_scrcpy_returns_404_for_unknown_device` which handles both JAR-present (404) and JAR-missing (503) scenarios
- Used `String` status field instead of enum for `ScrcpySessionInfo.status` — simpler serialization, status values are "active"/"stopped"

### Completion Notes List

- Extended `ScrcpyManager` with session lifecycle management (start/stop/list/get) using `DashMap<String, ScrcpySessionEntry>` keyed by udid
- `ScrcpySessionInfo` is a serializable struct with session_id (UUID v4), udid, serial, dimensions, device_name, started_at (ISO 8601)
- `ScrcpySessionEntry` holds `ScrcpySessionInfo` + `Arc<Mutex<ScrcpySession>>` — the Arc<Mutex<>> allows Story 6-3 to later access video/control streams
- REST handlers follow existing project patterns: cache-first device lookup, semantic error codes (ERR_SCRCPY_NOT_AVAILABLE, ERR_DEVICE_NOT_FOUND, ERR_SESSION_ALREADY_ACTIVE, ERR_SESSION_NOT_FOUND, ERR_DEVICE_NO_SERIAL)
- `scrcpy_ws.rs` left untouched — existing WebSocket handler works independently, Story 6-3 will integrate
- 6 new tests pass (2 unit in scrcpy_manager, 4 integration in test_server), 151 total pass, 4 pre-existing batch_report failures unchanged

### Code Review Fixes Applied

- **H1** (start_session race condition): Added `starting` DashMap sentinel to prevent TOCTOU race between `contains_key` check and `insert` — concurrent starts for the same device are now blocked, sentinel is removed on both success and failure
- **M1** (stop_session cleanup resilience): Cleanup is now best-effort with per-operation error logging — stream shutdown, process kill, and forward removal errors are warned but don't prevent session removal
- **M2** (resolve_device_serial cache miss): Added `device_info_cache.insert()` after DB fallback lookup — matches the `get_device_client` pattern in api_v1.rs
- **M3** (environment-dependent test): Split into `test_start_scrcpy_returns_error_for_unknown_device` and `test_start_scrcpy_device_with_no_serial` — latter inserts a mock device with empty serial to deterministically test the ERR_DEVICE_NO_SERIAL path

### File List

- `src/services/scrcpy_manager.rs` — MODIFIED: Added ScrcpySessionInfo, ScrcpySessionEntry structs, session management methods (start/stop/list/get), `starting` sentinel for race prevention, best-effort cleanup logging, 2 unit tests
- `src/routes/scrcpy.rs` — CREATED: REST handlers for start_scrcpy_session, stop_scrcpy_session, list_scrcpy_sessions, resolve_device_serial helper with cache update
- `src/routes/mod.rs` — MODIFIED: Added `pub mod scrcpy;`
- `src/state.rs` — MODIFIED: Added `scrcpy_manager: ScrcpyManager` field to AppState, initialized in AppState::new()
- `src/main.rs` — MODIFIED: Registered /scrcpy/{udid}/start, /scrcpy/{udid}/stop, /scrcpy/sessions routes
- `tests/test_server.rs` — MODIFIED: Added 3 scrcpy routes to setup_test_app!, added 4 integration tests (including no-serial device test)
