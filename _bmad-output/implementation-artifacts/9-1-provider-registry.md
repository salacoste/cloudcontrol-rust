# Story 9.1: Provider Registry

Status: done

## Story

As a **device farm administrator**,
I want **to register and manage provider nodes in the farm**,
so that **I can see which server hosts which devices**.

## Acceptance Criteria

1. **Given** the server has a `providers` SQLite table **When** I call `GET /api/v1/providers` (or `GET /providers?json`) **Then** all registered providers are returned with IP, notes, status, device count
2. **Given** a provider exists **When** I call `PUT /api/v1/providers/{id}` with `{"notes": "new notes"}` **Then** the provider's notes field is updated and a success response is returned
3. **Given** the server is running **When** I call `POST /api/v1/providers` with `{"ip": "192.168.1.100"}` **Then** a new provider is registered and returned with its assigned ID
4. **Given** the providers.html page is loaded **When** providers exist in the database **Then** the provider list table displays correctly with present status, IP, notes, uptime, devices, and ID columns
5. **Given** a non-existent provider ID **When** I call `PUT /api/v1/providers/{id}` **Then** a 404 error is returned
6. **Given** the `providers.html` template uses Vue.js `{{ }}` expressions **When** the server renders the page **Then** the Tera template engine does NOT process Vue expressions (they are wrapped in `{% raw %}` blocks)
7. **Given** a provider is registered **When** I call `GET /api/v1/providers` **Then** the provider's `device_count` field reflects the number of devices with matching `provider` field in the devices table

## Tasks / Subtasks

- [x] Task 1: Create `providers` table in database (AC: #1, #3)
  - [x] 1.1 Add `CREATE TABLE IF NOT EXISTS providers` in `ensure_initialized()` after the products table creation (`sqlite.rs:361`): `id INTEGER PRIMARY KEY AUTOINCREMENT, ip TEXT NOT NULL UNIQUE, notes TEXT DEFAULT '', present INTEGER DEFAULT 0, presence_changed_at INTEGER, created_at INTEGER NOT NULL`
  - [x] 1.2 Add index: `CREATE INDEX IF NOT EXISTS idx_providers_ip ON providers(ip)`
- [x] Task 2: Create Provider model (AC: #1, #3)
  - [x] 2.1 Create `src/models/provider.rs` with `Provider` struct (derives: `Debug, Clone, Serialize, Deserialize, sqlx::FromRow`) — fields: `id: i64, ip: String, notes: Option<String>, present: bool, presence_changed_at: Option<i64>, created_at: i64`
  - [x] 2.2 Add `CreateProviderRequest` struct (derives: `Debug, Deserialize`) — fields: `ip: String, notes: Option<String>`
  - [x] 2.3 Add `UpdateProviderRequest` struct (derives: `Debug, Deserialize`) — fields: `notes: Option<String>`
  - [x] 2.4 Add `ProviderWithDevices` struct for API responses — extends Provider with `device_count: i64`
  - [x] 2.5 Register module: add `pub mod provider;` to `src/models/mod.rs`
- [x] Task 3: Add provider database CRUD methods (AC: #1, #2, #3, #5, #7)
  - [x] 3.1 `create_provider(ip: &str, notes: Option<&str>) -> Result<Provider>` — INSERT with `created_at` as Unix timestamp, return created provider
  - [x] 3.2 `list_providers() -> Result<Vec<Provider>>` — SELECT all ordered by id
  - [x] 3.3 `get_provider(id: i64) -> Result<Option<Provider>>` — SELECT by id
  - [x] 3.4 `update_provider_notes(id: i64, notes: &str) -> Result<Option<Provider>>` — UPDATE notes WHERE id, return updated provider (None if not found)
  - [x] 3.5 `count_devices_by_provider(ip: &str) -> Result<i64>` — `SELECT COUNT(*) FROM devices WHERE provider = ?1` to compute device_count per provider
- [x] Task 4: Add API endpoints in `api_v1.rs` (AC: #1, #2, #3, #5, #7)
  - [x] 4.1 `list_providers` — GET handler, fetches all providers, enriches each with device_count via `count_devices_by_provider()`, returns `{"status": "success", "data": [...]}`
  - [x] 4.2 `create_provider` — POST handler with `web::Json<CreateProviderRequest>`, validates IP non-empty, returns `HttpResponse::Created` with provider data
  - [x] 4.3 `get_provider` — GET by id handler, returns 404 if not found
  - [x] 4.4 `update_provider` — PUT handler with `body: String` (NOT `web::Json` — jQuery `$.ajax` issue from Story 8.2 code review H1), parses JSON manually, updates notes, returns 404 if not found
- [x] Task 5: Add legacy-compatible page route + fix template (AC: #4, #6)
  - [x] 5.1 Add `GET /providers` page route in `control.rs` — render `providers.html` with Tera (no context variables needed — data loaded via AJAX)
  - [x] 5.2 Fix `providers.html` Tera/Vue conflict: wrap `<div id="app">...</div>` in `{% raw %}`/`{% endraw %}` blocks (same pattern as `edit.html` from Story 8.2)
  - [x] 5.3 Update AJAX URLs in providers.html: change `GET /providers?json` to `GET /api/v1/providers` and `PUT /providers/{id}` to `PUT /api/v1/providers/{id}`
- [x] Task 6: Register routes in `main.rs` (AC: #1, #2, #3, #4)
  - [x] 6.1 Add page route: `.route("/providers", web::get().to(routes::control::providers_page))`
  - [x] 6.2 Add API routes: `.route("/api/v1/providers", web::get().to(routes::api_v1::list_providers))`, `.route("/api/v1/providers", web::post().to(routes::api_v1::create_provider))`, `.route("/api/v1/providers/{id}", web::get().to(routes::api_v1::get_provider))`, `.route("/api/v1/providers/{id}", web::put().to(routes::api_v1::update_provider))`
- [x] Task 7: Regression testing (AC: #1-#7)
  - [x] 7.1 Build succeeds — 0 new warnings
  - [x] 7.2 All existing tests pass (168/177, 9 pre-existing failures)
  - [x] 7.3 No new regressions introduced

## Dev Notes

### Critical Architecture Constraint — Tera/Vue Template Conflict

**THIS IS THE #1 PITFALL** — `providers.html` already exists and uses Vue.js `{{ }}` expressions (e.g., `{{p.ip}}`, `{{presentCount}}`). Tera also uses `{{ }}`. Without mitigation, Tera will try to process Vue expressions and error.

**Fix**: Wrap the `<div id="app">` section in `{% raw %}`/`{% endraw %}` blocks, identical to what was done for `edit.html` in Story 8.2:

```html
{% raw %}
<div class="container-fluid" id="app">
    <!-- Vue.js template content here -->
</div>
{% endraw %}
```

The page route handler does NOT need to pass any Tera context variables — all data is loaded via AJAX. So the Tera render is trivially:
```rust
match state.tera.render("providers.html", &tera::Context::new()) {
    Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
    Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
}
```

### jQuery AJAX Content-Type Issue — CRITICAL for PUT endpoint

**Lesson from Story 8.2 code review (H1)**: The `providers.html` template uses jQuery `$.ajax` with `data: JSON.stringify({...})` but does NOT set `contentType: "application/json"`. This means:
- jQuery sends `Content-Type: application/x-www-form-urlencoded` by default
- actix-web's `web::Json<Value>` extractor requires `Content-Type: application/json`
- **The PUT handler MUST use `body: String` + `serde_json::from_str`**, NOT `web::Json<T>`

```rust
pub async fn update_provider(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: String,  // NOT web::Json — jQuery doesn't send Content-Type header
) -> HttpResponse {
    let id = path.into_inner();
    let body: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => { return HttpResponse::BadRequest().json(json!({...})); }
    };
    // ... extract notes, update ...
}
```

**Exception**: The `POST /api/v1/providers` create endpoint CAN use `web::Json<CreateProviderRequest>` because it will be called programmatically (not from the existing template), and programmatic callers set Content-Type properly.

### Database Table Schema

```sql
CREATE TABLE IF NOT EXISTS providers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ip TEXT NOT NULL UNIQUE,
    notes TEXT DEFAULT '',
    present INTEGER DEFAULT 0,
    presence_changed_at INTEGER,
    created_at INTEGER NOT NULL
)
```

- `present` is INTEGER (SQLite boolean pattern — 0=false, 1=true), defaults to 0 (offline). Story 9.2 will add heartbeat-driven presence updates.
- `presence_changed_at` is Unix timestamp (seconds), nullable. Story 9.2 will populate this.
- `ip` is UNIQUE — each provider has a distinct IP
- `created_at` is Unix timestamp when the provider was registered

### Provider Model — Follow Product Pattern

Model file: `src/models/provider.rs`. Follow `src/models/product.rs` exactly:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Provider {
    pub id: i64,
    pub ip: String,
    pub notes: Option<String>,
    pub present: bool,
    pub presence_changed_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub ip: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderWithDevices {
    #[serde(flatten)]
    pub provider: Provider,
    pub device_count: i64,
}
```

**IMPORTANT**: The `present` field is stored as INTEGER in SQLite but the `sqlx::FromRow` derive will map it to `bool` automatically (0→false, 1→true). The template expects `p.present` as a boolean for `v-if="p.present"`.

### Device Count — Computed via SQL

The template shows device icons (`fa-mobile`) for each device associated with a provider. For Story 9.1, we compute `device_count` by counting devices in the `devices` table that have a matching `provider` field:

```rust
pub async fn count_devices_by_provider(&self, ip: &str) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices WHERE provider = ?1")
        .bind(ip)
        .fetch_one(&self.pool)
        .await?;
    Ok(row.0)
}
```

The list endpoint enriches each Provider with its device_count before returning.

### API Response Pattern — Follow api_v1.rs Product Endpoints

Use the same response format as product catalog endpoints (`api_v1.rs:750-925`):

```rust
// Success
HttpResponse::Ok().json(json!({"status": "success", "data": providers_with_devices}))

// Created
HttpResponse::Created().json(json!({"status": "success", "data": provider}))

// Not Found
HttpResponse::NotFound().json(json!({"status": "error", "error": "ERR_PROVIDER_NOT_FOUND", "message": "Provider not found"}))

// Error
HttpResponse::InternalServerError().json(json!({"status": "error", "error": "ERR_DATABASE", "message": format!(...)}))
```

### Template URL Updates

The existing `providers.html` calls:
- `GET /providers?json` → Change to `GET /api/v1/providers`
- `PUT /providers/{id}` → Change to `PUT /api/v1/providers/{id}`

The response format changes too — the template currently expects a raw array from `GET /providers?json`, but our API returns `{"status": "success", "data": [...]}`. Update the AJAX success handler:

```javascript
// OLD: this.providers = ret;
// NEW: this.providers = ret.data;
```

### What NOT to Implement

- Do NOT add heartbeat/presence tracking — that's Story 9.2
- Do NOT add provider deletion endpoint — not in ACs (providers are long-lived nodes)
- Do NOT modify the device model or device_row_to_json — the `provider` field on devices already exists
- Do NOT add provider-device association logic — Story 9.2 handles this
- Do NOT add navigation link to index.html — not in ACs (can be done later)

### Project Structure Notes

- New model: `src/models/provider.rs` (add `pub mod provider;` to `src/models/mod.rs`)
- Database: `src/db/sqlite.rs` — table creation in `ensure_initialized()` + 5 CRUD methods
- API endpoints: `src/routes/api_v1.rs` — 4 handlers (list, create, get, update)
- Page route: `src/routes/control.rs` — 1 handler (providers_page)
- Route registration: `src/main.rs` — 5 routes
- Template fix: `resources/templates/providers.html` — `{% raw %}` blocks + URL updates

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 9, Story 9.1]
- [Source: docs/project-context.md#Tech Stack — sqlx 0.8, actix-web 4]
- [Source: src/db/sqlite.rs — ensure_initialized():137, products table:337-361, product CRUD:1468-1635]
- [Source: src/routes/api_v1.rs — product catalog endpoints:750-925 (handler patterns)]
- [Source: src/models/product.rs — model struct patterns, derives, request types]
- [Source: src/main.rs — product route registration:306-313]
- [Source: resources/templates/providers.html — existing Vue.js template, AJAX calls]
- [Source: _bmad-output/implementation-artifacts/8-2-device-product-association.md — Story 8.2 Tera/Vue conflict fix, jQuery Content-Type H1 fix]
- [Source: _bmad-output/implementation-artifacts/8-3-asset-property-tracking.md — Story 8.3 code review patterns]

### Git Context

Recent commits establish these patterns:
- `ead09d9` — recording CRUD with SQLite
- Story 8.1 established product CRUD, API response format `json!({"status": "success"})`
- Story 8.2 established Tera/Vue conflict fix (`{% raw %}`), `String` body parsing (H1), `rows_affected()` validation (H2)
- Story 8.3 established property tracking, `tracing::warn!` on errors (M3)

### Previous Epic Intelligence (Epic 8)

Critical lessons to apply:
- **H1 fix**: Use `body: String` + `serde_json::from_str` for any endpoint called from jQuery `$.ajax` templates
- **H2 fix**: Check `rows_affected() > 0` and return 404 for non-existent records
- **M1 fix**: Log errors with `tracing::warn!` when database operations fail
- **M2 fix**: Validate entity existence on GET page routes (return 404 if not found) — though providers page lists all providers so this is less relevant
- **Template fix**: `{% raw %}`/`{% endraw %}` blocks required for ALL templates with Vue.js expressions
- **L1 fix**: Trim whitespace on user inputs before validation

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Build: 0 errors, 0 new warnings
- Tests: 168/177 passed (9 pre-existing failures, 0 new regressions)

### Completion Notes List

- All 7 tasks completed with zero compilation errors on first attempt
- Applied Epic 8 lessons: `body: String` for jQuery PUT endpoint, `{% raw %}` for Tera/Vue conflict, `rows_affected()` for 404 detection, `tracing::warn!` on errors
- Template updated to use new API response format (`ret.data` instead of raw `ret`)
- Device count computed via SQL COUNT on devices table (no N+1 — one query per provider in list)
- `ProviderWithDevices` uses `#[serde(flatten)]` to merge Provider fields with device_count

### Code Review Fixes (2026-03-10)

- **H1 FIXED**: Added `#[serde(rename = "presenceChangedAt")]` to `presence_changed_at` field in Provider struct — template expects camelCase
- **M1 FIXED**: Removed unused `UpdateProviderRequest` struct (dead code — handler uses `body: String` + manual JSON parse)
- **M2 FIXED**: `update_provider` now returns `ProviderWithDevices` (with `device_count`) for API consistency with list/get
- **M3 FIXED**: `create_provider` now detects UNIQUE constraint violation and returns 409 Conflict with `ERR_DUPLICATE_IP`
- **M4 FIXED**: `create_provider` now trims `notes` input, consistent with `update_provider`
- **L1 TRACKED**: N+1 query in `list_providers` — acceptable for small provider counts, track as debt
- **L2 TRACKED**: No IP format validation — accepts any non-empty string, track as debt

### File List

- src/models/provider.rs (new — Provider struct, request types, ProviderWithDevices)
- src/models/mod.rs (added `pub mod provider;`)
- src/db/sqlite.rs (added providers table creation in ensure_initialized() + 5 CRUD methods)
- src/routes/api_v1.rs (added 4 provider endpoint handlers: list, create, get, update)
- src/routes/control.rs (added providers_page handler)
- src/main.rs (registered 5 new routes: 1 page + 4 API)
- resources/templates/providers.html (fixed Tera/Vue conflict with {% raw %}, updated AJAX URLs to /api/v1/providers)
