# Story 12.1: API Authentication

Status: done

## Story

As a **system administrator**,
I want **API endpoints protected by API key authentication**,
so that **unauthorized users cannot control devices**.

## Acceptance Criteria

1. **Given** an API key is configured in the server config **When** a request is made without a valid `Authorization` header or `api_key` query param **Then** the server returns 401 Unauthorized
2. **Given** an API key is configured **When** a request includes a valid `Authorization: Bearer <key>` header **Then** the request proceeds normally
3. **Given** an API key is configured **When** a request includes a valid `?api_key=<key>` query parameter **Then** the request proceeds normally
4. **Given** an API key is configured **When** a WebSocket upgrade request is made without a valid API key **Then** the upgrade is rejected with 401 Unauthorized
5. **Given** an API key is configured **When** a user browses to a frontend page (e.g., `/`, `/devices/{udid}/remote`) **Then** the page loads without authentication **And** the page's JavaScript includes the API key for subsequent API calls
6. **Given** NO API key is configured (field is absent or empty) **When** any request is made **Then** all requests proceed without authentication (backward compatible)

## Tasks / Subtasks

- [x] Task 1: Configuration — add API key to server config (AC: #1, #6)
  - [x] 1.1 Add `api_key` field to `AppConfig` in `src/config.rs`:
    ```rust
    #[serde(default)]
    pub api_key: Option<String>,
    ```
    If `None` or empty string, auth is disabled. Add unit test `test_config_with_api_key` verifying deserialization.
  - [x] 1.2 Add `api_key` field to `config/default_dev.yaml` as a commented-out example:
    ```yaml
    # API authentication (optional — if not set, all endpoints are open)
    # api_key: "your-secret-key-here"
    ```
  - [x] 1.3 Add `api_key_enabled: bool` convenience field to `AppState` in `src/state.rs` — derived from `config.api_key.as_ref().map_or(false, |k| !k.is_empty())`. This avoids repeated Option checks in middleware.

- [x] Task 2: Auth middleware implementation (AC: #1, #2, #3, #4, #6)
  - [x] 2.1 Create `src/middleware.rs` — implement `ApiKeyAuth` middleware using actix-web's `Transform` + `Service` traits. The middleware:
    - Stores `api_key: Option<String>` (cloned from config)
    - If `api_key` is `None` → pass ALL requests through (auth disabled)
    - Checks if request path is EXEMPT (see exempt list below)
    - If not exempt, extracts API key from:
      - `Authorization: Bearer <key>` header (case-insensitive "Bearer " prefix)
      - `?api_key=<key>` query parameter (URL-decoded)
    - If key matches → pass request through
    - If key doesn't match or is missing → return `HttpResponse::Unauthorized` with JSON body: `{"status": "error", "error": "ERR_UNAUTHORIZED", "message": "Valid API key required", "timestamp": "..."}`
    - Use constant-time comparison (`subtle` crate or manual byte comparison) to prevent timing attacks
  - [x] 2.2 Register module in `src/lib.rs`: `pub mod middleware;`
  - [x] 2.3 Add `use cloudcontrol::middleware;` in `src/main.rs`

- [x] Task 3: Define exempt paths (AC: #5, #6)
  - [x] 3.1 The middleware exempts these path patterns (checked via `starts_with` or exact match):
    **HTML Page Routes (exempt):**
    - `GET /` (exact)
    - `/devices/` + contains `/remote` (remote control page)
    - `/devices/` + contains `/edit` (edit page)
    - `/devices/` + contains `/property` (property page)
    - `GET /async` (batch control page)
    - `GET /installfile` (file upload page)
    - `GET /test` (test page)
    - `GET /providers` (providers page)
    - `GET /files` (files listing page)

    **Static Assets (exempt):**
    - `/static/` (all static files)

    **Monitoring/Discovery (exempt):**
    - `GET /api/v1/health` (health check — monitoring systems need unauthenticated access)
    - `GET /api/v1/openapi.json` (API discovery)

    **Everything else requires auth** when enabled:
    - `/api/*` (REST API)
    - `/inspector/*` (device control)
    - `/scrcpy/*` (screen mirroring)
    - `/nio/*` (NIO WebSocket)
    - `/video/convert` (video recording WebSocket)
    - `/list`, `/heartbeat`, `/shell`, `/upload`, etc.

- [x] Task 4: Wire middleware into main.rs (AC: #1, #4)
  - [x] 4.1 Add middleware wrap in `HttpServer::new` closure, BEFORE ErrorHandlers:
    ```rust
    App::new()
        .app_data(web::Data::new(app_state.clone()))
        .app_data(tera_data.clone())
        .wrap(middleware::ApiKeyAuth::new(app_state.config.api_key.clone()))
        .wrap(ErrorHandlers::new()...)
        // ... routes
    ```
    Note: `.wrap()` order matters in actix-web — middleware wraps are applied in REVERSE order (last added = first executed). Since we want auth to run BEFORE error handling, add it BEFORE the ErrorHandlers wrap.
  - [x] 4.2 Log auth status at startup:
    ```rust
    if app_state.api_key_enabled {
        tracing::info!("API authentication enabled");
    } else {
        tracing::warn!("API authentication disabled — all endpoints are open");
    }
    ```

- [x] Task 5: Frontend API key injection (AC: #5)
  - N/A 5.1 ~~In page rendering handlers, inject `api_key` into Tera context~~ — Approach B chosen (5.4), no Tera injection needed
  - N/A 5.2 ~~Add shared JavaScript auth utility~~ — Approach B chosen (5.4), no JS utility needed
  - N/A 5.3 ~~Update `fetch()` calls to include auth headers~~ — Approach B chosen (5.4), no frontend changes needed
  - [x] 5.4 **Approach B chosen**: Exempt same-origin requests via `Sec-Fetch-Site: same-origin` header (set automatically by modern browsers). External clients (curl, CI/CD) must still provide API key. No frontend code changes needed.

- [x] Task 6: OpenAPI spec update (AC: #1)
  - [x] 6.1 Add security scheme to OpenAPI spec in `src/models/openapi.rs`:
    ```json
    "securityDefinitions": {
        "ApiKeyAuth": {
            "type": "apiKey",
            "in": "header",
            "name": "Authorization",
            "description": "Bearer token: 'Bearer <api_key>'"
        },
        "ApiKeyQuery": {
            "type": "apiKey",
            "in": "query",
            "name": "api_key"
        }
    },
    "security": [{"ApiKeyAuth": []}, {"ApiKeyQuery": []}]
    ```
  - [x] 6.2 Add `401` response to documented endpoints

- [x] Task 7: Integration tests (AC: #1-#6)
  - [x] 7.1 Add test helpers to `tests/common/mod.rs`:
    - `make_test_config_with_auth(api_key: &str) -> AppConfig` — creates config with API key set
    - Update `setup_test_app!` macro to support optional auth config OR create a `setup_test_app_with_auth!` macro variant
  - [x] 7.2 Add `test_auth_api_returns_401_without_key` — GET `/api/v1/devices` without API key → 401
  - [x] 7.3 Add `test_auth_api_succeeds_with_bearer_header` — GET `/api/v1/devices` with `Authorization: Bearer test-key` → 200
  - [x] 7.4 Add `test_auth_api_succeeds_with_query_param` — GET `/api/v1/devices?api_key=test-key` → 200
  - [x] 7.5 Add `test_auth_invalid_key_returns_401` — GET `/api/v1/devices` with wrong key → 401
  - [x] 7.6 Add `test_auth_page_routes_exempt` — GET `/` → 200 (no auth needed)
  - [x] 7.7 Add `test_auth_static_files_exempt` — GET `/static/js/common.js` → 200 (no auth needed)
  - [x] 7.8 Add `test_auth_health_exempt` — GET `/api/v1/health` → 200 (no auth needed)
  - [x] 7.9 Add `test_auth_openapi_exempt` — GET `/api/v1/openapi.json` → 200 (no auth needed)
  - [x] 7.10 Add `test_auth_disabled_when_no_key` — no API key configured → all requests succeed without auth
  - [x] 7.11 Add `test_auth_inspector_requires_key` — GET `/inspector/{udid}/screenshot` requires auth
  - [x] 7.12 Add `test_auth_openapi_includes_security_scheme` — verify OpenAPI spec includes security definitions

- [x] Task 8: Unit tests for middleware (AC: #1-#4)
  - [x] 8.1 Add unit tests in `src/middleware.rs`:
    - `test_exempt_paths` — verify correct paths are exempt
    - `test_extract_bearer_token` — verify header parsing
    - `test_extract_query_param` — verify query param parsing
    - `test_auth_disabled` — verify passthrough when no key

- [x] Task 9: Regression testing (AC: #1-#6)
  - [x] 9.1 Build succeeds — 0 new warnings
  - [x] 9.2 All existing tests pass (310 existing + new tests) — existing tests should still pass because they don't set an API key in config, so auth is disabled
  - [x] 9.3 No new regressions introduced

## Dev Notes

### Scope — API Key Authentication Middleware

This story adds **API key authentication** to protect device control endpoints from unauthorized external access. The design is deliberately simple (API key, not JWT/OAuth) since this is a device lab tool, not a public SaaS. The key architectural decisions:

| Decision | Rationale |
|----------|-----------|
| **API key, not JWT** | Device labs have a small number of API consumers (CI/CD pipelines). API key is simpler and sufficient. |
| **Middleware, not per-handler** | Centralized auth logic prevents forgotten checks on new endpoints. One place to audit. |
| **Config-based key, not DB** | Single API key in YAML config. No key management UI. Restart to change key. Simple. |
| **Backward compatible** | No key configured = no auth = existing deployments keep working. |
| **Constant-time comparison** | Prevents timing-based key extraction attacks. |

### Middleware Implementation Pattern

actix-web 4 middleware uses the `Transform` + `Service` trait pattern:

```rust
pub struct ApiKeyAuth {
    api_key: Option<String>,
}

impl ApiKeyAuth {
    pub fn new(api_key: Option<String>) -> Self {
        // Treat empty string as None
        let api_key = api_key.filter(|k| !k.is_empty());
        Self { api_key }
    }
}

impl<S, B> Transform<S, ServiceRequest> for ApiKeyAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = ApiKeyAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ApiKeyAuthMiddleware {
            service,
            api_key: self.api_key.clone(),
        })
    }
}
```

The `ApiKeyAuthMiddleware<S>` implements `Service` and performs the actual check. See `actix-web` middleware docs for the full pattern.

### Exempt Path Logic

```rust
fn is_exempt(path: &str, method: &Method) -> bool {
    // Static files
    if path.starts_with("/static/") { return true; }

    // Health + OpenAPI (monitoring/discovery)
    if path == "/api/v1/health" || path == "/api/v1/openapi.json" { return true; }

    // Page routes (only GET for HTML pages)
    if method == Method::GET {
        if path == "/" || path == "/async" || path == "/installfile"
           || path == "/test" || path == "/files" || path == "/providers" {
            return true;
        }
        // Dynamic page routes
        if path.starts_with("/devices/") &&
           (path.ends_with("/remote") || path.ends_with("/edit") || path.ends_with("/property")) {
            return true;
        }
    }

    false
}
```

### API Key Extraction

```rust
fn extract_api_key(req: &ServiceRequest) -> Option<&str> {
    // Try Authorization header first
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(key) = auth_str.strip_prefix("Bearer ") {
                return Some(key.trim());
            }
        }
    }

    // Try query parameter
    let query = req.query_string();
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == "api_key")
        .map(|(_, v)| v.as_ref())
}
```

### Constant-Time Comparison

Do NOT use `==` for key comparison — it short-circuits on first mismatch, leaking key length/content via timing. Use:

```rust
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
```

Or use the `subtle` crate's `ConstantTimeEq` trait. Check if `subtle` is worth adding vs inline implementation. The inline version is ~5 lines and avoids a dependency.

### Frontend API Key Injection Decision

There are TWO valid approaches for making frontend JavaScript work with auth. **Choose ONE during implementation:**

**Approach A: Explicit header injection** (more secure, more invasive)
- Inject API key into templates via Tera context
- Update all `fetch()` calls and WebSocket URLs to include the key
- Works even if browser security headers change

**Approach B: Sec-Fetch-Site exemption** (simpler, less secure)
- Middleware checks `Sec-Fetch-Site: same-origin` header (set by modern browsers)
- No frontend code changes needed
- External clients (curl, scripts) must still provide API key
- Less secure: header can be spoofed. But the threat model is "prevent unauthorized CI/CD access", not "defend against determined attackers on same network"
- Falls back gracefully: if header is missing (old browser, non-browser client), API key is required

**Recommendation**: Start with **Approach B** for minimal code changes. It delivers the AC requirements (external API consumers need API key, browser users don't) with zero frontend modifications. If the security requirement is later tightened, switch to Approach A.

### WebSocket Authentication

WebSocket auth is handled by the same middleware because WebSocket upgrade starts as a regular HTTP GET request. The middleware runs BEFORE the handler calls `actix_ws::handle()`. The exempt path list does NOT include WebSocket paths, so they require auth.

For WebSocket connections from the frontend, if using Approach A, the `api_key` query parameter is appended to the WebSocket URL:
```javascript
var ws = new WebSocket(wsURL + "?api_key=" + encodeURIComponent(API_KEY));
```

With Approach B, the browser's `Sec-Fetch-Site: same-origin` header is sent on WebSocket upgrade too, so no changes needed.

### 401 Response Format

Follow the existing `ApiResponse` pattern from `api_v1.rs`:

```json
{
    "status": "error",
    "error": "ERR_UNAUTHORIZED",
    "message": "Valid API key required. Provide via 'Authorization: Bearer <key>' header or '?api_key=<key>' query parameter.",
    "timestamp": "2026-03-11T12:00:00Z"
}
```

### Error Response Mapping

Add `ERR_UNAUTHORIZED` error code. The middleware returns `HttpResponse::Unauthorized()` (401) directly — it does NOT use the `error_response()` helper since the middleware doesn't have access to the route-level helpers. Build the JSON response inline.

### Existing Test Compatibility

All 310 existing tests use `make_test_config()` which creates a config WITHOUT an API key. Since auth is disabled when no key is configured, ALL existing tests continue to pass without modification. Only new tests explicitly set an API key to test auth behavior.

### What NOT to Implement

- Do NOT add JWT tokens or OAuth — API key is sufficient for device labs
- Do NOT add user management or multiple API keys — single key in config is enough
- Do NOT add role-based access control — all authenticated users have full access
- Do NOT add API key rotation or expiry — change the config and restart
- Do NOT add rate limiting — that's Story 12.2
- Do NOT add API key storage in database — config file only
- Do NOT add CORS headers — not needed for same-origin frontend, external API consumers don't need CORS

### Project Structure Notes

- **NEW**: `src/middleware.rs` — API key auth middleware (Transform + Service traits)
- Modified: `src/config.rs` — add `api_key: Option<String>` field
- Modified: `src/state.rs` — add `api_key_enabled: bool` derived field
- Modified: `src/lib.rs` — add `pub mod middleware;`
- Modified: `src/main.rs` — wire middleware, log auth status
- Modified: `src/routes/control.rs` — inject `api_key` into Tera template contexts (if Approach A)
- Modified: `src/models/openapi.rs` — add security scheme
- Modified: `config/default_dev.yaml` — add commented-out `api_key` example
- Modified: `resources/static/js/common.js` or templates — auth helper functions (if Approach A)
- Modified: `tests/test_server.rs` — add auth integration tests
- Modified: `tests/common/mod.rs` — add `make_test_config_with_auth` helper

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Story 12.1 — AC definition]
- [Source: src/config.rs — AppConfig struct (add api_key field)]
- [Source: src/state.rs:92-149 — AppState struct (add api_key_enabled)]
- [Source: src/main.rs:98-487 — Route registration and middleware wiring]
- [Source: src/routes/control.rs — Page handlers with Tera context injection]
- [Source: src/routes/api_v1.rs — API handler patterns, error_response(), ApiResponse]
- [Source: src/models/openapi.rs — OpenAPI spec generation]
- [Source: tests/test_server.rs — setup_test_app! macro, test patterns]
- [Source: tests/common/mod.rs — make_test_config(), create_temp_db() helpers]
- [Source: docs/project-context.md — Project architecture overview]
- [Source: _bmad-output/implementation-artifacts/epic-7-retro-2026-03-10.md — Previous retro action items]

### Git Context

Recent commits establish these patterns:
- Story 11.2 added SQLite persistence and startup recovery (async method patterns)
- Story 11.2 code review found `resp.success` vs `resp.status === 'success'` mismatch — verify API response format consistency
- Story 11.2 established `{% raw %}` pattern for Tera+Vue template escaping
- Story 10.4 established dead code cleanup patterns
- All stories consistently have code reviews catch: missing tests, missing OpenAPI entries, response format bugs

### Previous Story Intelligence (Story 11.2)

Critical lessons to apply:
- **Tera template escaping**: If injecting API key into templates containing Vue.js, use `{% raw %}` blocks around Vue expressions
- **API response format**: Use `{"status": "success|error", ...}` format — NOT `{"success": true/false}`
- **Test TempDir lifetime**: Keep TempDir alive as long as Database pool references the path
- **async method changes**: When changing method signatures, trace ALL call sites (`.await` additions)
- **Frontend JS changes**: Test response field names match actual API responses (`resp.status === 'success'` not `resp.success`)

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List

- **NEW** `src/middleware.rs` — API key auth middleware (Transform + Service traits, 16 unit tests)
- **Modified** `src/config.rs` — Added `api_key: Option<String>` field to AppConfig + 2 unit tests
- **Modified** `src/state.rs` — Added `api_key_enabled: bool` derived field to AppState
- **Modified** `src/lib.rs` — Added `pub mod middleware;`
- **Modified** `src/main.rs` — Wired middleware `.wrap()`, auth status logging
- **Modified** `src/models/openapi.rs` — Added `components`/`security` fields, security schemes
- **Modified** `config/default_dev.yaml` — Added commented-out `api_key` example
- **Modified** `tests/common/mod.rs` — Added `make_test_config_with_auth()` helper
- **Modified** `tests/test_server.rs` — Added `setup_test_app_with_auth!` macro + 12 integration tests
