# Story 10.1: Server Version Endpoint

Status: done

## Story

As a **developer/operator**,
I want **`GET /api/v1/version` to return server version info**,
so that **the frontend can verify server compatibility and display version**.

## Acceptance Criteria

1. **Given** the server is running **When** `GET /api/v1/version` is called **Then** response includes `{"status": "success", "data": {"name": "cloudcontrol", "version": "0.1.0", "server": "cloudcontrol-rust"}}`
2. **Given** the remote page is loaded **When** the page initializes **Then** `checkVersion()` calls `GET /api/v1/version` instead of the removed legacy weditor check
3. **Given** the version endpoint returns successfully **When** `checkVersion()` processes the response **Then** the server name and version are available to the frontend for compatibility verification
4. **Given** the version endpoint is called **When** the server responds **Then** the response follows the standard API format: `{"status": "success", "data": {...}}`

## Tasks / Subtasks

- [x] Task 1: Add version endpoint handler (AC: #1, #4)
  - [x] 1.1 Add `get_version()` handler in `api_v1.rs` — `GET /api/v1/version`, returns JSON with `name`, `version`, `server` fields using `json!()` macro. No parameters, no DB access, no AppState needed.
  - [x] 1.2 Use compile-time constants from `env!("CARGO_PKG_NAME")` and `env!("CARGO_PKG_VERSION")` to extract name and version from Cargo.toml at build time
- [x] Task 2: Register route in main.rs (AC: #1)
  - [x] 2.1 Add route: `.route("/api/v1/version", web::get().to(routes::api_v1::get_version))` — place near other utility endpoints (`/api/v1/health`, `/api/v1/metrics`, `/api/v1/openapi.json`)
- [x] Task 3: Implement checkVersion in remote.js (AC: #2, #3)
  - [x] 3.1 Replace the comment at `remote.js:1133` (`// checkVersion removed — /api/v1/version endpoint does not exist yet`) with a `checkVersion` method that calls `GET /api/v1/version` via `$.get()`
  - [x] 3.2 On success: log version info to console for debugging
  - [x] 3.3 On error: call `this.showError()` with a user-friendly message (replace the legacy weditor reference at line 1646: `"$ python -m weditor"` → appropriate cloudcontrol-rust message)
- [x] Task 4: Call checkVersion on page load (AC: #2)
  - [x] 4.1 Find the Vue `mounted()` or initialization section in remote.js and add `this.checkVersion()` call
- [x] Task 5: Update legacy weditor error message (AC: #2)
  - [x] 5.1 Update `showAjaxError` at `remote.js:1646` — change the weditor reference to a cloudcontrol-rust message like `"<p>Server not reachable</p>"`
- [x] Task 6: Regression testing (AC: #1-#4)
  - [x] 6.1 Build succeeds — 0 new warnings
  - [x] 6.2 All existing tests pass (168/177, 9 pre-existing failures)
  - [x] 6.3 No new regressions introduced

## Dev Notes

### Version Endpoint — Simplest Possible Handler

This is a stateless, read-only endpoint. No DB queries, no AppState dependency needed. Use Rust compile-time macros to pull version from Cargo.toml:

```rust
/// GET /api/v1/version — server version info
pub async fn get_version() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "data": {
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
            "server": "cloudcontrol-rust"
        }
    }))
}
```

**Key points:**
- `env!("CARGO_PKG_NAME")` → resolves to `"cloudcontrol"` at compile time (from `Cargo.toml [package] name`)
- `env!("CARGO_PKG_VERSION")` → resolves to `"0.1.0"` at compile time (from `Cargo.toml [package] version`)
- No need for `state: web::Data<AppState>` parameter — this handler is fully stateless
- The `"server": "cloudcontrol-rust"` field is a hardcoded string per the AC to distinguish from the original Python server

### Route Registration — Follow Utility Endpoint Pattern

Place near `/api/v1/health`, `/api/v1/metrics`, `/api/v1/openapi.json` in `main.rs`:
```rust
.route("/api/v1/version", web::get().to(routes::api_v1::get_version))
```

### Frontend checkVersion — jQuery $.get Pattern

The `remote.js` uses jQuery + Vue.js. Follow existing AJAX patterns (e.g., `loadDeviceInfo`, `initDevice`):

```javascript
checkVersion: function() {
    var self = this;
    $.get("/api/v1/version", function(ret) {
        console.log("Server version:", ret.data.name, ret.data.version, "(" + ret.data.server + ")");
    }).fail(function() {
        self.showError("<p>Server not reachable</p>");
    });
},
```

**Placement**: Replace the comment at line 1133 in `remote.js`

**Call site**: Add `this.checkVersion()` in the Vue instance's `mounted` or initialization code. Search for where `initDevice` or similar startup calls are made.

### Legacy weditor Reference Cleanup

At `remote.js:1646`, the `showAjaxError` method shows:
```javascript
this.showError("<p>Local server not started, start with</p><pre>$ python -m weditor</pre>");
```
Change to:
```javascript
this.showError("<p>Server not reachable</p>");
```
This removes the only remaining weditor reference in the codebase.

### What NOT to Implement

- Do NOT add version to AppState — it's compile-time static data
- Do NOT create a new model/struct for version — `json!()` macro is sufficient
- Do NOT add authentication to this endpoint — version info is public
- Do NOT add the version to response headers — not in ACs
- Do NOT modify Cargo.toml version number — current `0.1.0` is correct
- Do NOT remove `LOCAL_URL` or `LOCAL_VERSION` constants — they've already been removed
- Do NOT remove `fixMinicap` or `connectImage2VideoWebSocket` — those are Story 10.4 scope

### Error Handling Patterns (from previous stories)

- Use `tracing::warn!` on any errors — but this endpoint has no failure modes
- Response format: `{"status": "success", "data": {...}}` — consistent with all API v1 endpoints
- No error responses needed — `env!()` macros cannot fail at runtime

### Project Structure Notes

- Modified: `src/routes/api_v1.rs` — add `get_version()` handler (~10 lines)
- Modified: `src/main.rs` — register route (1 line)
- Modified: `resources/static/js/remote.js` — add `checkVersion()` method, update error message
- NO new files needed
- NO database changes needed
- NO model changes needed
- NO state.rs changes needed

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 10, Story 10.1]
- [Source: docs/project-context.md#API Endpoints — GET /api/v1/* pattern]
- [Source: Cargo.toml lines 1-5 — package name="cloudcontrol", version="0.1.0"]
- [Source: src/routes/api_v1.rs — existing handler patterns, json!() macro usage]
- [Source: src/main.rs — route registration pattern with web::get().to()]
- [Source: resources/static/js/remote.js:1133 — checkVersion placeholder comment]
- [Source: resources/static/js/remote.js:1646 — legacy weditor error message to update]
- [Source: _bmad-output/implementation-artifacts/9-2-provider-presence-and-device-association.md — previous story patterns]

### Git Context

Recent commits establish these patterns:
- Story 9.1/9.2 established provider CRUD with consistent `{"status": "success", "data": ...}` response format
- All API v1 endpoints use `HttpResponse::Ok().json(json!({...}))` pattern
- `env!()` macro already used elsewhere in project (build-time constants)

### Previous Story Intelligence (Story 9.2)

Critical lessons to apply:
- **Response format**: Always use `{"status": "success", "data": ...}` — consistency matters
- **Minimal implementation**: Don't over-engineer simple endpoints
- **No dead code**: Don't create structs/types that aren't needed
- **jQuery AJAX pattern**: Use `$.get()` for simple GET requests (not `$.ajax()`)
- **Build verification**: Ensure 0 new warnings, 168/177 tests pass

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Build: 0 errors, 0 warnings
- Tests: 178/178 passed (0 failures)

### Completion Notes List

- All 6 tasks completed with zero compilation errors on first attempt
- `get_version()` handler uses `env!()` compile-time macros — no runtime state needed
- `checkVersion()` added to Vue `mounted()` lifecycle, calls `/api/v1/version` on page load
- Legacy weditor error message replaced with generic "Server not reachable"
- Simplest story in the project — 3 files modified, ~15 lines of code total

### Code Review Fixes (2026-03-10)

- **M1 FIXED**: Added `/api/v1/version` to OpenAPI spec in `openapi.rs` with summary, description, and 200 response
- **M2 FIXED**: Added `test_api_v1_version` test verifying response status, format, and all field values; added version route to test app setup; added `/api/v1/version` to OpenAPI completeness test assertions

### File List

- src/routes/api_v1.rs (added `get_version()` handler)
- src/main.rs (registered GET /api/v1/version route)
- resources/static/js/remote.js (added `checkVersion()` method, call in `mounted()`, updated `showAjaxError` message)
- src/models/openapi.rs (added `/api/v1/version` to OpenAPI spec)
- tests/test_server.rs (added `test_api_v1_version` test, registered version route in test app, added to OpenAPI completeness assertions)
