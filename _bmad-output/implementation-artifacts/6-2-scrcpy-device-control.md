# Story 6.2: Scrcpy Device Control

Epic: 6 (High-Fidelity Screen Mirroring)
Status: done
Priority: P3

## Story

As a **QA Engineer**,
I want to control devices through the scrcpy stream via REST endpoints,
so that I can send tap, key, and swipe inputs while viewing high-quality video.

## Context & Dependencies

### Related Requirements
- **FR36**: Users can control devices through scrcpy video stream
- **NFR17**: Scrcpy video stream latency <100ms

### Dependencies
- **Story 6-1**: Scrcpy Session Management (done) — provides `ScrcpyManager` with session lifecycle, `ScrcpySessionEntry` holding `Arc<Mutex<ScrcpySession>>`
- **Epic 3**: Remote Device Control (done) — ATX-based control endpoints as reference pattern

### Architecture Constraints
From `_bmad-output/project-context.md`:
- actix-web 4.x REST handlers with `web::Data<AppState>`
- Error handling: `Result<T, String>` in service layer
- Logging: `tracing::info!("[Scrcpy]")` with context tags
- JSON request bodies via `web::Json<T>` with serde `Deserialize`
- No new dependencies required — all needed crates already in Cargo.toml

## Acceptance Criteria

### AC1: Tap through scrcpy stream
```gherkin
Scenario: Tap through scrcpy stream
  Given a scrcpy session is active for a device
  When I send POST /scrcpy/{udid}/tap with x, y coordinates
  Then the tap executes on the device via the scrcpy control socket
  And a success response is returned with latency info
  And latency is under 100ms (NFR17)
```

### AC2: Keyboard input through scrcpy
```gherkin
Scenario: Keyboard input through scrcpy
  Given a scrcpy session is active for a device
  When I send POST /scrcpy/{udid}/key with a keycode
  Then the key event is sent to the device via the scrcpy control socket
  And special keys (Enter=66, Backspace=67, Home=3) work correctly
```

### AC3: Handle input when no active session
```gherkin
Scenario: Handle input when no active session
  Given no scrcpy session is active for a device
  When I send a tap or key event
  Then HTTP 404 is returned
  And error code is "ERR_SESSION_NOT_FOUND"
```

### AC4: Swipe through scrcpy stream
```gherkin
Scenario: Swipe through scrcpy stream
  Given a scrcpy session is active for a device
  When I send POST /scrcpy/{udid}/swipe with start/end coordinates and duration
  Then a touch-down, sequence of touch-move, and touch-up events are sent
  And the swipe executes smoothly on the device
```

## Tasks / Subtasks

- [x] Task 1: Add session access helper to ScrcpyManager (AC: 1, 2, 3, 4)
  - [x] Add `pub fn get_session_handle(&self, udid: &str) -> Result<Arc<Mutex<ScrcpySession>>, String>` method to `ScrcpyManager` that returns a cloned Arc handle — returns `ERR_SESSION_NOT_FOUND` if no session exists
  - [x] Changed from closure-based `with_session` to `get_session_handle` due to Rust async closure lifetime constraints

- [x] Task 2: Add control request types (AC: 1, 2, 4)
  - [x] Create `TapRequest` struct: `x: u32, y: u32, width: Option<u16>, height: Option<u16>` (width/height default to session dimensions)
  - [x] Create `KeyRequest` struct: `keycode: u32, action: Option<String>` (action defaults to "press" = down+up, also supports "down"/"up")
  - [x] Create `SwipeRequest` struct: `start_x: u32, start_y: u32, end_x: u32, end_y: u32, duration_ms: Option<u64>, steps: Option<u32>` (duration defaults to 300ms, steps defaults to 20)
  - [x] All structs derive `Deserialize` and live in `src/routes/scrcpy.rs`

- [x] Task 3: Implement tap endpoint (AC: 1, 3)
  - [x] Add `pub async fn scrcpy_tap()` handler in `src/routes/scrcpy.rs`
  - [x] Route: `POST /scrcpy/{udid}/tap` with JSON body `TapRequest`
  - [x] Look up session via `scrcpy_manager.get_session_handle()`, get session dimensions from `ScrcpySessionInfo` for defaults
  - [x] Send touch-down (action=0, pressure=0xFFFF) then touch-up (action=1, pressure=0) via `session.send_touch()`
  - [x] Return `{"status": "success", "action": "tap", "x": ..., "y": ...}`
  - [x] Return 404 with `ERR_SESSION_NOT_FOUND` if no active session

- [x] Task 4: Implement key endpoint (AC: 2, 3)
  - [x] Add `pub async fn scrcpy_key()` handler in `src/routes/scrcpy.rs`
  - [x] Route: `POST /scrcpy/{udid}/key` with JSON body `KeyRequest`
  - [x] For action="press" (default): send key-down (action=0) then key-up (action=1) via `session.send_key()`
  - [x] For action="down" or "up": send single key event
  - [x] Return `{"status": "success", "action": "key", "keycode": ..., "key_action": ...}`
  - [x] Return 404 with `ERR_SESSION_NOT_FOUND` if no active session

- [x] Task 5: Implement swipe endpoint (AC: 4, 3)
  - [x] Add `pub async fn scrcpy_swipe()` handler in `src/routes/scrcpy.rs`
  - [x] Route: `POST /scrcpy/{udid}/swipe` with JSON body `SwipeRequest`
  - [x] Implementation: send touch-down at start, interpolate `steps` touch-move events from start to end with `tokio::time::sleep()` between steps (duration_ms / steps per step), then touch-up at end
  - [x] Use session dimensions for width/height parameters in `send_touch()` calls
  - [x] Return `{"status": "success", "action": "swipe", "start": {"x":..., "y":...}, "end": {"x":..., "y":...}, "duration_ms": ..., "steps": ...}`
  - [x] Return 404 with `ERR_SESSION_NOT_FOUND` if no active session

- [x] Task 6: Register routes in main.rs (AC: 1, 2, 3, 4)
  - [x] Register in `src/main.rs`:
    - `.route("/scrcpy/{udid}/tap", web::post().to(routes::scrcpy::scrcpy_tap))`
    - `.route("/scrcpy/{udid}/key", web::post().to(routes::scrcpy::scrcpy_key))`
    - `.route("/scrcpy/{udid}/swipe", web::post().to(routes::scrcpy::scrcpy_swipe))`

- [x] Task 7: Write tests (AC: 1, 2, 3, 4)
  - [x] Add 3 new routes to `setup_test_app!` macro in `tests/test_server.rs`
  - [x] Test: `test_scrcpy_tap_returns_404_no_session` — POST /scrcpy/nonexistent/tap with JSON body returns 404 + ERR_SESSION_NOT_FOUND
  - [x] Test: `test_scrcpy_key_returns_404_no_session` — POST /scrcpy/nonexistent/key with JSON body returns 404 + ERR_SESSION_NOT_FOUND
  - [x] Test: `test_scrcpy_swipe_returns_404_no_session` — POST /scrcpy/nonexistent/swipe with JSON body returns 404 + ERR_SESSION_NOT_FOUND
  - [x] Unit test in `scrcpy_manager.rs`: `test_get_session_handle_not_found` — verify `get_session_handle()` returns ERR_SESSION_NOT_FOUND for non-existent udid

## Dev Notes

### Scrcpy Control Protocol

The low-level protocol is already implemented in `src/device/scrcpy.rs`:

**Touch events** (`send_touch`) — 28 bytes:
- type: u8 = 2 (INJECT_TOUCH_EVENT)
- action: u8 — 0=ACTION_DOWN, 1=ACTION_UP, 2=ACTION_MOVE
- pointer_id: u64 BE = 0xFFFFFFFFFFFFFFFF (finger/mouse)
- x: u32 BE, y: u32 BE (device coordinates)
- width: u16 BE, height: u16 BE (screen dimensions)
- pressure: u16 BE — 0xFFFF for touch, 0 for release
- action_button: u32 BE = 0, buttons: u32 BE = 0

**Key events** (`send_key`) — 14 bytes:
- type: u8 = 0 (INJECT_KEYCODE)
- action: u8 — 0=ACTION_DOWN, 1=ACTION_UP
- keycode: u32 BE (Android KEYCODE_*)
- repeat: u32 BE = 0, metastate: u32 BE = 0

### Session Access Pattern

Story 6-1 designed `ScrcpySessionEntry` to hold `Arc<Mutex<ScrcpySession>>` specifically for this use case. The `with_session` helper pattern:

```rust
pub async fn with_session<F, Fut, R>(&self, udid: &str, f: F) -> Result<R, String>
where
    F: FnOnce(&mut ScrcpySession) -> Fut,
    Fut: std::future::Future<Output = Result<R, String>>,
{
    let entry = self.sessions.get(udid)
        .ok_or_else(|| "ERR_SESSION_NOT_FOUND".to_string())?;
    let mut session = entry.session.lock().await;
    f(&mut session).await
}
```

This gives the REST handler exclusive access to the scrcpy control socket while holding the lock, then releases it immediately after.

### Swipe Implementation

Swipe is a sequence of touch events with timing:
1. `send_touch(action=0, start_x, start_y, w, h, pressure=0xFFFF)` — finger down
2. For i in 1..=steps: interpolate (x, y), `send_touch(action=2, x, y, w, h, pressure=0xFFFF)` — finger move, sleep(duration/steps)
3. `send_touch(action=1, end_x, end_y, w, h, pressure=0)` — finger up

Linear interpolation: `x = start_x + (end_x - start_x) * i / steps`

### Existing WebSocket Control (Reference Only)

`src/routes/scrcpy_ws.rs` (lines 160-198) shows the existing WS-based control pattern. The browser sends raw binary touch/key events over WebSocket, which are forwarded to the scrcpy control socket. This story adds the equivalent capability via REST endpoints for programmatic/API access.

**Do NOT modify `scrcpy_ws.rs`** — it continues to work independently.

### Response Format

Follow project conventions from Story 6-1 and recording endpoints:

```json
// POST /scrcpy/{udid}/tap → 200 OK
{
  "status": "success",
  "action": "tap",
  "x": 540,
  "y": 960
}

// POST /scrcpy/{udid}/key → 200 OK
{
  "status": "success",
  "action": "key",
  "keycode": 66,
  "key_action": "press"
}

// POST /scrcpy/{udid}/swipe → 200 OK
{
  "status": "success",
  "action": "swipe",
  "start": {"x": 540, "y": 1600},
  "end": {"x": 540, "y": 400},
  "duration_ms": 300,
  "steps": 20
}

// Any control endpoint → 404 (no session)
{
  "status": "error",
  "error": "ERR_SESSION_NOT_FOUND",
  "message": "No active scrcpy session for device 'abc123'"
}
```

### Testing Strategy

Since scrcpy requires real ADB devices and active sessions, tests verify:
- HTTP 404 responses when no session is active (deterministic, no device needed)
- JSON request deserialization (valid request bodies)
- `with_session` helper returns correct error for non-existent sessions
- Response JSON structure

Full end-to-end control tests require physical USB devices and are out of scope for automated tests.

### Common Android Keycodes (Reference)
- KEYCODE_HOME = 3
- KEYCODE_BACK = 4
- KEYCODE_DPAD_UP = 19, DOWN = 20, LEFT = 21, RIGHT = 22
- KEYCODE_ENTER = 66
- KEYCODE_DEL (Backspace) = 67
- KEYCODE_POWER = 26
- KEYCODE_VOLUME_UP = 24, KEYCODE_VOLUME_DOWN = 25
- KEYCODE_APP_SWITCH (Recents) = 187

### Project Structure Notes

Files to create/modify:
```
src/services/scrcpy_manager.rs  — MODIFY: Add with_session() helper method, unit test
src/routes/scrcpy.rs            — MODIFY: Add TapRequest, KeyRequest, SwipeRequest structs; add scrcpy_tap, scrcpy_key, scrcpy_swipe handlers
src/main.rs                     — MODIFY: Register /scrcpy/{udid}/tap, /scrcpy/{udid}/key, /scrcpy/{udid}/swipe routes
tests/test_server.rs            — MODIFY: Add 3 control routes to setup_test_app!, add 4 tests
```

No new files needed — all additions go into existing files from Story 6-1.

### References

- [Source: src/device/scrcpy.rs:267-313] — `send_touch()` and `send_key()` method signatures and byte formats
- [Source: src/services/scrcpy_manager.rs] — ScrcpyManager with sessions DashMap, ScrcpySessionEntry with Arc<Mutex<ScrcpySession>>
- [Source: src/routes/scrcpy.rs] — Existing REST handlers from Story 6-1 (start/stop/list)
- [Source: src/routes/scrcpy_ws.rs:160-198] — Existing WebSocket control forwarding (reference pattern, DO NOT MODIFY)
- [Source: _bmad-output/planning-artifacts/epics-stories.md:1295-1325] — Story acceptance criteria
- [Source: _bmad-output/planning-artifacts/prd.md] — FR36, NFR17

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

- Changed from closure-based `with_session` to `get_session_handle` returning `Arc<Mutex<ScrcpySession>>` — Rust async closure lifetime constraints prevent `FnOnce(&mut T) -> Future` pattern from compiling
- Used `result.err().unwrap()` instead of `result.unwrap_err()` in test because `ScrcpySession` doesn't implement `Debug`

### Completion Notes List

- Added `get_session_handle()` method to `ScrcpyManager` — returns cloned `Arc<Mutex<ScrcpySession>>` for direct control access, drops DashMap ref before caller acquires Mutex lock
- Added `get_session_with_info()` method — returns both `ScrcpySessionInfo` and `Arc<Mutex<ScrcpySession>>` in one DashMap access to avoid TOCTOU race
- Added 3 request types: `TapRequest` (x, y, optional width/height), `KeyRequest` (keycode, optional action: press/down/up), `SwipeRequest` (start/end coords, optional duration_ms/steps)
- Implemented `scrcpy_tap` handler: sends touch-down (action=0, pressure=0xFFFF) + touch-up (action=1, pressure=0) via scrcpy control socket
- Implemented `scrcpy_key` handler: supports "press" (down+up, default), "down", and "up" actions with input validation (rejects invalid action strings with 400)
- Implemented `scrcpy_swipe` handler: touch-down at start, interpolated move steps with `tokio::time::sleep()` between each, touch-up at end. Default 300ms duration, 20 steps, max 5000ms cap
- All handlers use session dimensions from `ScrcpySessionInfo` as defaults for width/height with safe `u16::try_from` conversion
- All handlers return 404 + ERR_SESSION_NOT_FOUND when no active session exists
- 4 new tests pass (1 unit, 3 integration), 250 total pass (0 failures — batch_report SQL bug also fixed)

### Code Review Fixes Applied

- **H1** (TOCTOU race in tap/swipe): Added `get_session_with_info()` to ScrcpyManager — returns info + handle in single DashMap access, eliminating race between separate `get_session()` and `get_session_handle()` calls
- **M1** (u32→u16 truncation): Changed `info.width as u16` to `u16::try_from(info.width).unwrap_or(u16::MAX)` in tap and swipe handlers
- **M2** (invalid key action silently accepted): Added `matches!` validation before session lock — returns 400 + ERR_INVALID_ACTION for unrecognized action strings
- **M3** (batch_report SQL bug): Fixed `add_batch_report_result` in `src/db/sqlite.rs` — VALUES clause had 9 placeholders (`?1`-`?9`) for 8 columns, changed to `?1`-`?8`. All 4 batch_report tests now pass
- **M4** (swipe lock duration): Added `MAX_SWIPE_DURATION_MS = 5000` cap and comment explaining lock is held intentionally to prevent interleaved touch events

### File List

- `src/services/scrcpy_manager.rs` — MODIFIED: Added `get_session_handle()` and `get_session_with_info()` methods, `test_get_session_handle_not_found` unit test
- `src/routes/scrcpy.rs` — MODIFIED: Added `TapRequest`, `KeyRequest`, `SwipeRequest` structs; added `scrcpy_tap`, `scrcpy_key`, `scrcpy_swipe` handlers; key action validation; `MAX_SWIPE_DURATION_MS` constant
- `src/main.rs` — MODIFIED: Registered /scrcpy/{udid}/tap, /scrcpy/{udid}/key, /scrcpy/{udid}/swipe routes
- `src/db/sqlite.rs` — MODIFIED: Fixed `add_batch_report_result` SQL placeholder count (9→8)
- `tests/test_server.rs` — MODIFIED: Added 3 control routes to setup_test_app!, added 3 integration tests (tap/key/swipe 404 no session)
