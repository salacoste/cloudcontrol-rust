# Story 10.4: Legacy Endpoint Cleanup

Status: done

## Story

As a **developer**,
I want **legacy endpoints to have `/api/v1/` equivalents**,
so that **the API is consistent and versioned**.

## Acceptance Criteria

1. **Given** legacy endpoints exist (e.g., `/inspector/{udid}/hierarchy`, `/inspector/{udid}/upload`, `/inspector/{udid}/rotation`) **When** `/api/v1/` equivalents are needed **Then** missing v1 endpoints are created with standardized `ApiResponse` format
2. **Given** legacy endpoints exist **When** v1 equivalents are created **Then** legacy endpoints remain functional for backwards compatibility
3. **Given** `remote.js` contains `LOCAL_URL` and `LOCAL_VERSION` constants **When** cleanup is performed **Then** these constants are removed (if they still exist — verify first)
4. **Given** `remote.js` contains dead methods `fixMinicap` and `connectImage2VideoWebSocket` **When** cleanup is performed **Then** these methods and all their callers (`startLowQualityScreenRecord`, `startVideoRecord`, `stopVideoRecord`) are removed

## Tasks / Subtasks

- [x] Task 1: Create `/api/v1/` equivalents for missing legacy endpoints (AC: #1, #2)
  - [x] 1.1 Add `GET /api/v1/devices/{udid}/hierarchy` handler in `api_v1.rs` — wraps `DeviceService::dump_hierarchy()` with `ApiResponse` format. Reference existing `inspector_hierarchy()` at `control.rs:1713` for logic. Mock device handling must be preserved.
  - [x] 1.2 Add `POST /api/v1/devices/{udid}/upload` handler in `api_v1.rs` — wraps file upload with multipart handling and `ApiResponse` format. Reference existing `inspector_upload()` at `control.rs:1771` for logic (auto-path by extension, media scan for images).
  - [x] 1.3 Add `POST /api/v1/devices/{udid}/rotation` handler in `api_v1.rs` — wraps rotation fix via ATX agent with `ApiResponse` format. Reference existing `inspector_rotation()` at `control.rs:1838` for logic.
  - [x] 1.4 Register all 3 new routes in `src/main.rs` in the `/api/v1/` route group (after line ~322)
  - [x] 1.5 Add OpenAPI spec entries for all 3 new endpoints in `src/models/openapi.rs`
  - [x] 1.6 Verify legacy `/inspector/` endpoints still function unchanged — NO modifications to existing handlers
- [x] Task 2: Remove dead frontend code from `remote.js` (AC: #3, #4)
  - [x] 2.1 Verify `LOCAL_URL` and `LOCAL_VERSION` — grep `remote.js` for these constants. If already removed (Epic 7 may have cleaned them), document as pre-cleaned. If present, remove them.
  - [x] 2.2 Remove `connectImage2VideoWebSocket` method (line ~1401) — connects to `/video/convert` which doesn't exist as a route
  - [x] 2.3 Remove `startLowQualityScreenRecord` method (line ~1415) — calls `connectImage2VideoWebSocket`, dead code
  - [x] 2.4 Remove `startVideoRecord` method (line ~1446) — calls `connectImage2VideoWebSocket`, dead code
  - [x] 2.5 Remove `stopVideoRecord` method (line ~1472) — cleanup for removed recording methods
  - [x] 2.6 Remove `fixMinicap` method (line ~1559) — legacy minicap cleanup (NIO/scrcpy replaced minicap)
  - [x] 2.7 Verify no HTML templates reference the removed methods — confirmed: `startLowQualityScreenRecord`, `startVideoRecord`, `stopVideoRecord`, `fixMinicap` appear ONLY in `remote.js`, not in any `.html` template
- [x] Task 3: Integration tests for new v1 endpoints (AC: #1)
  - [x] 3.1 Add `test_v1_hierarchy_device_not_found` — GET `/api/v1/devices/nonexistent/hierarchy` returns 404 with `ERR_DEVICE_NOT_FOUND`
  - [x] 3.2 Add `test_v1_hierarchy_mock_device` — GET `/api/v1/devices/{udid}/hierarchy` for mock device returns mock hierarchy JSON
  - [x] 3.3 Add `test_v1_upload_device_not_found` — POST `/api/v1/devices/nonexistent/upload` returns 404
  - [x] 3.4 Add `test_v1_rotation_device_not_found` — POST `/api/v1/devices/nonexistent/rotation` returns 404
  - [x] 3.5 Register all 3 new routes in `setup_test_app!` macro in `tests/test_server.rs`
- [x] Task 4: Regression testing (AC: #1-#4)
  - [x] 4.1 Build succeeds — 0 new warnings
  - [x] 4.2 All existing tests pass (181/181 + new tests)
  - [x] 4.3 No new regressions introduced

## Dev Notes

### Scope — Focused Endpoint Parity + Dead Code Removal

This story creates v1 equivalents for 3 legacy endpoints that are missing from the API v1 layer, and removes dead frontend code. **No legacy endpoints are removed** — they remain for backwards compatibility.

### Endpoint Gap Analysis

| Legacy Endpoint | v1 Equivalent | Status |
|----------------|---------------|--------|
| `GET /inspector/{udid}/screenshot` | `GET /api/v1/devices/{udid}/screenshot` | **Already exists** |
| `POST /inspector/{udid}/touch` | `POST /api/v1/devices/{udid}/tap` | **Already exists** |
| `POST /inspector/{udid}/input` | `POST /api/v1/devices/{udid}/input` | **Already exists** |
| `POST /inspector/{udid}/keyevent` | `POST /api/v1/devices/{udid}/keyevent` | **Already exists** |
| `GET /inspector/{udid}/hierarchy` | `GET /api/v1/devices/{udid}/hierarchy` | **MISSING — Create** |
| `POST /inspector/{udid}/upload` | `POST /api/v1/devices/{udid}/upload` | **MISSING — Create** |
| `POST /inspector/{udid}/rotation` | `POST /api/v1/devices/{udid}/rotation` | **MISSING — Create** |

### v1 Handler Pattern — Follow Existing api_v1.rs Conventions

All v1 handlers in `api_v1.rs` follow this pattern:

```rust
pub async fn handler_name(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    // Validate device exists via get_device_client()
    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    // Business logic...
    // Return with ApiResponse format
    HttpResponse::Ok().json(ApiResponse {
        status: "success".to_string(),
        data: Some(result),
        error: None,
        message: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}
```

**IMPORTANT**: `api_v1.rs` has its OWN `get_device_client()` helper (lines 29-74) that is DIFFERENT from the one in `control.rs`. Use the `api_v1.rs` version — it includes device info caching and returns standardized error responses.

### Hierarchy Handler Design

The v1 hierarchy handler should:
1. Use `get_device_client()` from `api_v1.rs`
2. Check `is_mock` flag — return mock hierarchy (same as `control.rs:1732-1755`)
3. Call `DeviceService::dump_hierarchy(&client)` — returns `Result<Value, String>`
4. Wrap result in `ApiResponse` format

**Required imports for hierarchy**: `DeviceService` is already imported in `api_v1.rs`.

### Upload Handler Design

The v1 upload handler should:
1. Accept `Multipart` payload (add `actix_multipart::Multipart` import if not present)
2. Use `get_device_client()` from `api_v1.rs`
3. Follow same file path logic as `control.rs:1798-1808` (extension-based path selection)
4. Call `client.push_file()` and trigger media scan for images
5. Return `ApiResponse` with upload path in data

**Note**: `actix_multipart` is already a dependency — check `Cargo.toml`. If `Multipart` is not imported in `api_v1.rs`, add: `use actix_multipart::Multipart;`

### Rotation Handler Design

The v1 rotation handler should:
1. Use `get_device_client()` from `api_v1.rs`
2. POST to `{base_url}/info/rotation` on the ATX agent
3. Return `ApiResponse` with success/error

**Note**: The existing `inspector_rotation()` returns the raw ATX agent response body. The v1 version should wrap it in `ApiResponse` format for consistency.

### Dead Frontend Code — Confirmed Dead

| Method | Line | Why Dead |
|--------|------|----------|
| `connectImage2VideoWebSocket` | ~1401 | Connects to `/video/convert` — **route doesn't exist** |
| `startLowQualityScreenRecord` | ~1415 | Calls `connectImage2VideoWebSocket` |
| `startVideoRecord` | ~1446 | Calls `connectImage2VideoWebSocket` |
| `stopVideoRecord` | ~1472 | Cleanup for above methods |
| `fixMinicap` | ~1559 | Legacy minicap cleanup — minicap replaced by NIO/scrcpy |

These methods are ONLY in `remote.js` — no HTML template references them. They are safe to remove.

**`LOCAL_URL` and `LOCAL_VERSION`**: Grep shows these do NOT exist in current `remote.js` — already cleaned in Epic 7. Task 2.1 should verify and document as pre-cleaned.

### What NOT to Implement

- Do NOT remove legacy `/inspector/` endpoints — they must remain for backwards compatibility (AC#2)
- Do NOT modify existing `control.rs` handlers — v1 handlers are NEW functions in `api_v1.rs`
- Do NOT remove `/feeds` WebSocket stub — out of scope for this story (documented as legacy but still registered)
- Do NOT add frontend migration to use v1 endpoints — that's Epic 13 (Frontend Modernization)
- Do NOT add shell endpoint v1 equivalent — shell already has both GET and POST variants in control.rs with proper JSON responses
- Do NOT clean up `/api/batch/*` duplicate endpoints — out of scope
- Do NOT add authentication or rate limiting — that's Epic 12

### Route Registration Pattern

In `src/main.rs`, new routes go in the api_v1 block (lines ~295-331):
```rust
.route("/api/v1/devices/{udid}/hierarchy", web::get().to(routes::api_v1::hierarchy))
.route("/api/v1/devices/{udid}/upload", web::post().to(routes::api_v1::upload))
.route("/api/v1/devices/{udid}/rotation", web::post().to(routes::api_v1::rotation))
```

### OpenAPI Spec Pattern

In `src/models/openapi.rs`, add entries following existing patterns (e.g., the `/api/v1/devices/{udid}/screenshot` entry). Each new endpoint needs:
- Path entry with method, summary, description
- Parameters section with UDID path parameter
- Response section with 200 success and error codes

### Test App Macro Pattern

In `tests/test_server.rs`, the `setup_test_app!` macro registers routes. Add new routes after existing api_v1 routes:
```rust
.route("/api/v1/devices/{udid}/hierarchy", web::get().to(routes::api_v1::hierarchy))
.route("/api/v1/devices/{udid}/upload", web::post().to(routes::api_v1::upload))
.route("/api/v1/devices/{udid}/rotation", web::post().to(routes::api_v1::rotation))
```

### Project Structure Notes

- Modified: `src/routes/api_v1.rs` — add 3 new v1 handler functions (hierarchy, upload, rotation)
- Modified: `src/main.rs` — register 3 new routes in api_v1 block
- Modified: `src/models/openapi.rs` — add OpenAPI spec entries for 3 new endpoints
- Modified: `resources/static/js/remote.js` — remove 5 dead methods
- Modified: `tests/test_server.rs` — add integration tests + register routes in test macro
- NO new files needed
- NO database changes needed
- NO new dependencies needed

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 10, Story 10.4]
- [Source: src/routes/control.rs:1713-1766 — inspector_hierarchy handler (reference for v1)]
- [Source: src/routes/control.rs:1771-1833 — inspector_upload handler (reference for v1)]
- [Source: src/routes/control.rs:1838-1867 — inspector_rotation handler (reference for v1)]
- [Source: src/routes/api_v1.rs:29-74 — get_device_client helper (USE THIS for v1 handlers)]
- [Source: src/main.rs:295-331 — api_v1 route registration block]
- [Source: src/models/openapi.rs — OpenAPI spec entries for existing v1 endpoints]
- [Source: resources/static/js/remote.js:1401-1414 — connectImage2VideoWebSocket (dead code)]
- [Source: resources/static/js/remote.js:1415-1471 — startLowQuality/startVideoRecord (dead code)]
- [Source: resources/static/js/remote.js:1559-1573 — fixMinicap (dead code)]
- [Source: _bmad-output/implementation-artifacts/10-3-diagnostic-test-page.md — Story 10.3 patterns]

### Git Context

Recent commits establish these patterns:
- Story 10.3 established diagnostic enhancement patterns, code review found status tracking bugs
- Story 10.2 established DashMap entry API for atomic operations, DB error logging
- Story 10.1 established version endpoint with OpenAPI spec + test coverage requirements
- Code reviews consistently catch: missing tests, missing OpenAPI entries, silent error handling

### Previous Story Intelligence (Story 10.3)

Critical lessons to apply:
- **Verify before implementing**: Check if cleanup targets (LOCAL_URL, LOCAL_VERSION) already exist
- **Build verification**: Ensure 0 new warnings, all tests pass
- **Minimal changes**: Story 10.3 only modified 1 file — keep scope tight
- **Code review patterns**: Always include OpenAPI entries for new endpoints, always include tests

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Build: 0 errors, 0 new warnings
- Tests: 293 total (186 integration + 107 unit), 0 failures — 5 new tests added

### Completion Notes List

- Task 1: Created 3 new v1 API handlers in `api_v1.rs` — `hierarchy()` wraps `DeviceService::dump_hierarchy()` with mock device handling and `ApiResponse` format, `upload()` wraps multipart file upload with extension-based path selection and media scan, `rotation()` wraps ATX agent rotation fix with `ApiResponse` format. All use `api_v1.rs` `get_device_client()` helper and `success_response()`/`error_response()` patterns. Registered routes in `main.rs`, added 3 OpenAPI spec entries in `openapi.rs`. Legacy `/inspector/` endpoints untouched.
- Task 2: Removed 5 dead methods from `remote.js` — `connectImage2VideoWebSocket` (connected to nonexistent `/video/convert` route), `startLowQualityScreenRecord`, `startVideoRecord`, `stopVideoRecord` (all called dead WebSocket method), `fixMinicap` (legacy minicap cleanup, replaced by NIO/scrcpy). Verified `LOCAL_URL`/`LOCAL_VERSION` already removed in Epic 7. Verified no HTML templates reference removed methods.
- Task 3: Added 5 integration tests — hierarchy device-not-found (404), hierarchy mock device (200 with mock tree), upload device-not-found (404), upload no-file for valid device (ERR_INVALID_REQUEST), rotation device-not-found (404). Updated OpenAPI completeness test to include 3 new endpoints. Registered routes in `setup_test_app!` macro.
- Task 4: Build clean, 293 tests pass (181→186 integration), 0 regressions.
- Code Review Fixes: M1 — added 100MB upload size limit to prevent OOM. M2 — added path traversal protection via `Path::file_name()` sanitization. L1 — removed orphaned `videoReceiver: null` data property. L2 — added multipart/form-data requestBody to upload OpenAPI spec. L3 — added `test_v1_upload_no_file` integration test.

### File List

- src/routes/api_v1.rs (added 3 new v1 handlers: hierarchy, upload, rotation)
- src/main.rs (registered 3 new routes in api_v1 block)
- src/models/openapi.rs (added OpenAPI spec entries for 3 new endpoints)
- resources/static/js/remote.js (removed 5 dead methods: connectImage2VideoWebSocket, startLowQualityScreenRecord, startVideoRecord, stopVideoRecord, fixMinicap)
- tests/test_server.rs (added 4 integration tests, updated OpenAPI test, registered routes in test macro)
