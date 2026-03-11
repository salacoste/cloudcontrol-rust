# Story 10.2: Device Reservation System

Status: done

## Story

As a **device farm operator**,
I want **to reserve a device so only one user controls it at a time**,
so that **concurrent access doesn't cause conflicts**.

## Acceptance Criteria

1. **Given** a device is available **When** a user opens the remote page and the WebSocket to `/devices/{udid}/reserved` connects **Then** the device is marked as reserved in AppState and `using_device` is set to `true` in the database
2. **Given** a device is reserved **When** another user views the device list **Then** the device shows `"using": true` indicating it is in use
3. **Given** a device is reserved via WebSocket **When** the WebSocket disconnects (browser close, network loss, manual close) **Then** the device is released — `using_device` is set to `false` in the database and removed from AppState reservation tracking
4. **Given** a device is already reserved by another WebSocket **When** a second user attempts to connect to `/devices/{udid}/reserved` **Then** the WebSocket connection is accepted but an error message is sent and the connection is closed, leaving the original reservation intact

## Tasks / Subtasks

- [x] Task 1: Add reservation tracking to AppState (AC: #1, #3)
  - [x] 1.1 Add `reserved_devices: Arc<DashMap<String, String>>` to `AppState` in `state.rs` — maps device UDID → remote IP/identifier of the reserving client
  - [x] 1.2 Initialize in `AppState::new()`: `reserved_devices: Arc::new(DashMap::new())`
- [x] Task 2: Implement reservation WebSocket handler (AC: #1, #3, #4)
  - [x] 2.1 Rewrite the stub `reserved()` handler in `control.rs` to:
    - Accept `state: web::Data<AppState>` parameter (currently missing)
    - Extract UDID from path parameter (change `_path` to `path`, use `path.into_inner()`)
    - Validate device exists via `state.db.find_by_udid(&udid)` — close with error if not found
    - Check `state.reserved_devices` for existing reservation — if reserved, send error JSON message and close WebSocket
    - If available: insert into `state.reserved_devices`, set `using_device = true` in DB via `state.db.update(&udid, &json!({"using": true}))`
    - Echo pings back as pongs (frontend sends "ping" every 5s via `setInterval`)
    - On WebSocket close/disconnect: remove from `state.reserved_devices`, set `using_device = false` in DB
  - [x] 2.2 Handle all WebSocket message types: Text (ping echo), Binary (ignore), Close (cleanup), Ping/Pong (protocol-level)
- [x] Task 3: Verify frontend integration (AC: #1, #2)
  - [x] 3.1 Verify `remote.js:reserveDevice()` (lines 1379-1400) connects to the correct WebSocket URL `/devices/{udid}/reserved` — NO changes needed, already implemented
  - [x] 3.2 Verify device list API (`/list` or `/api/v1/devices`) returns `"using": true` for reserved devices — this works automatically via existing `device_row_to_json()` which reads `using_device` from DB
- [x] Task 4: Regression testing (AC: #1-#4)
  - [x] 4.1 Build succeeds — 0 new warnings
  - [x] 4.2 All existing tests pass (178/178)
  - [x] 4.3 No new regressions introduced

## Dev Notes

### Existing Infrastructure — Most Plumbing Already Exists

This story has a significant head start — the frontend, route, database column, and device model field ALL already exist:

| Component | Status | Location |
|-----------|--------|----------|
| DB column `using_device` | Exists | `src/db/sqlite.rs:148` — `using_device INTEGER DEFAULT 0` |
| Device model field | Exists | `src/models/device.rs:20-22` — `#[serde(rename = "using")]` |
| Field mapping | Exists | `src/db/sqlite.rs:28` — `("using", "using_device")` |
| DB update capability | Exists | `src/db/sqlite.rs:529` — `update()` handles `using` → `using_device` |
| WebSocket route | Exists | `src/main.rs:386-388` — `/devices/{query}/reserved` |
| WebSocket handler | **Stub** | `src/routes/control.rs:3137-3165` — echoes messages, no reservation logic |
| Frontend client | Exists | `resources/static/js/remote.js:1379-1400` — `reserveDevice()` |
| Frontend call site | Exists | `resources/static/js/remote.js:252-255` — called in `mounted()` |

**The only real work is rewriting the stub handler in `control.rs` and adding the DashMap to AppState.**

### Current Stub Handler — What to Replace

The current handler at `control.rs:3137-3165`:
```rust
/// GET /devices/{query}/reserved → legacy WebSocket heartbeat
pub async fn reserved(
    req: HttpRequest,
    stream: web::Payload,
    _path: web::Path<String>,
) -> HttpResponse {
    match actix_ws::handle(&req, stream) {
        Ok((resp, mut session, mut msg_stream)) => {
            actix_web::rt::spawn(async move {
                while let Some(Ok(msg)) = msg_stream.next().await {
                    match msg {
                        actix_ws::Message::Text(text) => {
                            let _ = session.text(format!("Hello, {}", text)).await;
                        }
                        actix_ws::Message::Binary(data) => {
                            let _ = session.binary(data).await;
                        }
                        actix_ws::Message::Close(_) => break,
                        _ => {}
                    }
                }
            });
            resp
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
```

**Problems with current handler:**
1. Ignores `_path` — doesn't extract UDID
2. No `state` parameter — can't access DB or AppState
3. No reservation tracking — just echoes messages
4. No cleanup on disconnect

### Replacement Handler Design

```rust
/// GET /devices/{udid}/reserved → WebSocket device reservation
pub async fn reserved(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    // Validate device exists
    let device = match state.db.find_by_udid(&udid).await {
        Ok(Some(d)) => d,
        _ => return HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_DEVICE_NOT_FOUND",
            "message": format!("Device {} not found", udid)
        })),
    };

    // Check if already reserved
    if state.reserved_devices.contains_key(&udid) {
        // Still upgrade WebSocket, send error, then close
        // (Can't return HTTP error after WebSocket upgrade attempt from frontend)
    }

    // Upgrade to WebSocket
    match actix_ws::handle(&req, stream) {
        Ok((resp, mut session, mut msg_stream)) => {
            // Check reservation conflict AFTER upgrade
            if state.reserved_devices.contains_key(&udid) {
                let _ = session.text(json!({"error": "Device already reserved"}).to_string()).await;
                let _ = session.close(None).await;
                return resp;
            }

            // Reserve the device
            let remote_addr = req.peer_addr().map(|a| a.to_string()).unwrap_or_default();
            state.reserved_devices.insert(udid.clone(), remote_addr);
            let _ = state.db.update(&udid, &json!({"using": true})).await;

            // Spawn message handler with cleanup
            let db = state.db.clone();
            let reserved = state.reserved_devices.clone();
            let udid_clone = udid.clone();
            actix_web::rt::spawn(async move {
                while let Some(Ok(msg)) = msg_stream.next().await {
                    match msg {
                        actix_ws::Message::Text(_) => {
                            let _ = session.text("pong").await;
                        }
                        actix_ws::Message::Close(_) => break,
                        actix_ws::Message::Ping(data) => {
                            let _ = session.pong(&data).await;
                        }
                        _ => {}
                    }
                }
                // Cleanup on disconnect
                reserved.remove(&udid_clone);
                let _ = db.update(&udid_clone, &json!({"using": false})).await;
                tracing::info!("[RESERVED] Device {} released", udid_clone);
            });

            tracing::info!("[RESERVED] Device {} reserved", udid);
            resp
        }
        Err(_) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_WEBSOCKET_UPGRADE_FAILED",
            "message": "WebSocket upgrade failed"
        })),
    }
}
```

### Frontend Client — Already Complete, No Changes Needed

The `remote.js:reserveDevice()` (lines 1379-1400):
- Creates WebSocket to `ws://{host}/devices/{udid}/reserved`
- Sends "ping" every 5 seconds via `setInterval`
- Logs messages to console
- Resolves deferred on `onopen`, rejects on `onclose`
- Called in `mounted()` (line 252) with `.catch()` — failure doesn't block page

### Device "in use" Visibility — Already Works

When a device's `using_device` is set to `true` in the DB:
- `device_row_to_json()` serializes it as `"using": true`
- The device list endpoints (`/list`, `/api/v1/devices`) return this field
- The dashboard template already shows device status based on these fields

### AppState Extension — Follow Existing DashMap Pattern

Add to `state.rs` (follow `heartbeat_sessions` and `provider_heartbeats` pattern):
```rust
/// Device reservation tracking: UDID → remote client address (Story 10-2)
pub reserved_devices: Arc<DashMap<String, String>>,
```

Initialize in `AppState::new()`:
```rust
reserved_devices: Arc::new(DashMap::new()),
```

### What NOT to Implement

- Do NOT modify `remote.js:reserveDevice()` — it already works correctly
- Do NOT add new routes — `/devices/{query}/reserved` is already registered at `main.rs:386-388`
- Do NOT create a new database table — `using_device` column already exists
- Do NOT add a REST API for reservation — the AC specifies WebSocket only
- Do NOT add reservation timeout/expiry — the WebSocket connection IS the lease (disconnection = release)
- Do NOT persist reservations across server restarts — AppState DashMap is in-memory, DB `using_device` field handles persistence
- Do NOT block the page load if reservation fails — frontend already handles this with `.catch()`

### Edge Case: Server Restart While Devices Reserved

If the server restarts, `reserved_devices` DashMap is empty but `using_device` may still be `true` in the DB from the previous session. Consider clearing all `using_device = true` on startup, but this is NOT required by the ACs and can be addressed later.

### Error Handling Patterns (from previous stories)

- Use `tracing::info!` for reservation/release events (not warn — these are normal operations)
- Use `tracing::warn!` on database errors
- Return consistent JSON error messages via WebSocket text frames
- Follow the existing WebSocket error pattern from `ws_screenshot` (api_v1.rs:1286)

### Project Structure Notes

- Modified: `src/state.rs` — add `reserved_devices: Arc<DashMap<String, String>>` to AppState
- Modified: `src/routes/control.rs` — rewrite `reserved()` handler with reservation logic
- NO new files needed
- NO new routes needed (route already exists)
- NO frontend changes needed (client already exists)
- NO database schema changes needed (column already exists)
- NO model changes needed (field already exists)

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 10, Story 10.2]
- [Source: docs/project-context.md#WebSocket Channels — /devices/{udid}/reserved]
- [Source: src/routes/control.rs:3137-3165 — current stub handler to rewrite]
- [Source: src/main.rs:386-388 — existing route registration]
- [Source: resources/static/js/remote.js:1379-1400 — existing frontend WebSocket client]
- [Source: resources/static/js/remote.js:252-255 — reserveDevice() call in mounted()]
- [Source: src/models/device.rs:20-22 — using_device field with serde rename]
- [Source: src/db/sqlite.rs:28,48,53 — using/using_device field mapping and bool handling]
- [Source: src/db/sqlite.rs:529-554 — update() method handles using field]
- [Source: src/state.rs:100,108 — DashMap pattern for heartbeat_sessions, provider_heartbeats]
- [Source: _bmad-output/implementation-artifacts/10-1-server-version-endpoint.md — Story 10.1 patterns]

### Git Context

Recent commits establish these patterns:
- Story 10.1 established version endpoint with OpenAPI spec + test coverage requirements
- Code review found missing OpenAPI entry and test — always include these for new endpoints
- All API v1 endpoints use `HttpResponse::Ok().json(json!({...}))` pattern
- WebSocket handlers use `actix_ws::handle()` + `actix_web::rt::spawn()` pattern

### Previous Story Intelligence (Story 10.1)

Critical lessons to apply:
- **OpenAPI spec**: New endpoints need entries in `openapi.rs` — code review will catch this
- **Test coverage**: Add integration tests, register routes in test app `setup_test_app!` macro
- **Response format**: Use `{"status": "success"/"error", "data"/"message": ...}` consistently
- **Build verification**: Ensure 0 new warnings, all 178 tests pass

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Build: 0 errors, 0 new warnings
- Tests: 181/181 passed (0 failures)

### Completion Notes List

- Task 1: Added `reserved_devices: Arc<DashMap<String, String>>` to AppState struct and initialized in `AppState::new()`, following the existing `provider_heartbeats` DashMap pattern
- Task 2: Rewrote stub `reserved()` handler with full reservation logic — device validation via `find_by_udid`, reservation conflict detection after WebSocket upgrade (sends error JSON and closes), reservation insert + DB `using_device=true` on connect, cleanup (DashMap remove + DB `using_device=false`) on disconnect, ping/pong echo for frontend heartbeat, structured tracing for reservation/release events
- Task 3: Verified frontend `reserveDevice()` connects to correct WebSocket URL and sends ping every 5s; verified device list returns `"using": true` via existing `device_row_to_json()` DB read
- Task 4: Build succeeds with 0 new warnings, all 181 tests pass with 0 regressions
- Only 3 files modified — minimal, focused implementation leveraging existing infrastructure

### Code Review Fixes (2026-03-10)

- **M1 FIXED**: Race condition — replaced `contains_key` + `insert` with atomic DashMap `entry()` API to prevent TOCTOU in concurrent WebSocket reservations
- **M2 FIXED**: Added 3 integration tests: `test_reserved_device_not_found_returns_404`, `test_reserved_device_exists_but_no_ws_upgrade`, `test_reserved_device_not_marked_using_without_ws`; registered `/devices/{query}/reserved` route in test app
- **M3 FIXED**: Changed `let _ = db.update(...)` to `if let Err(e) = db.update(...)` with `tracing::warn!` on both reserve and release DB update calls

### File List

- src/state.rs (added `reserved_devices` field to AppState struct and initialization)
- src/routes/control.rs (rewrote `reserved()` handler with full reservation logic, atomic entry API, DB error logging)
- tests/test_server.rs (added 3 reservation tests, registered reserved route in test app)
