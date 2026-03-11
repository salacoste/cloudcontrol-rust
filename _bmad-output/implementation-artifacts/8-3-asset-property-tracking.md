# Story 8.3: Asset Property Tracking

Status: done

## Story

As a **device farm administrator**,
I want **to assign inventory/asset numbers to devices**,
so that **I can track physical device assets**.

## Acceptance Criteria

1. **Given** a device is connected **When** I open `/devices/{udid}/property` **Then** the property page loads showing the current asset number (or empty if none assigned) and input fields for setting a new one
2. **Given** the property page is open **When** I enter a full asset number (e.g., "HIH-PHO-12345") or just the numeric suffix **Then** I can submit the form to save the property ID
3. **Given** I submit the form **When** `POST /api/v1/devices/{udid}/property` is called with `{"property_id": "HIH-PHO-12345"}` **Then** the property_id is saved to the device record and a success response is returned
4. **Given** a device has a property_id assigned **When** I call `GET /devices/{udid}/info` **Then** the response includes a `property_id` field with the stored value
5. **Given** a device has no property_id **When** I call `GET /devices/{udid}/info` **Then** the response includes `property_id: null`
6. **Given** the `property.html` template uses `[[.]]` for the current value **When** the server renders the page **Then** the Tera template engine correctly injects the current property_id value
7. **Given** a non-existent device UDID **When** I call `POST /api/v1/devices/{udid}/property` **Then** a 404 error is returned

## Tasks / Subtasks

- [x] Task 1: Add `property_id` column to devices table (AC: #3, #4, #5)
  - [x] 1.1 Add migration in `ensure_initialized()`: `ALTER TABLE devices ADD COLUMN property_id TEXT` (silently fail if exists — same pattern as tags/product_id migrations at `sqlite.rs:225-237`)
  - [x] 1.2 Place migration AFTER the product_id migration (line 237) and BEFORE the recordings table creation (line 239)
- [x] Task 2: Add device-property database method (AC: #3, #7)
  - [x] 2.1 `update_device_property(udid: &str, property_id: &str) -> Result<bool, sqlx::Error>` — `UPDATE devices SET property_id = ?1 WHERE udid = ?2`, return `rows_affected() > 0`
- [x] Task 3: Create `POST /api/v1/devices/{udid}/property` endpoint (AC: #3, #7)
  - [x] 3.1 Add handler `update_device_property` in `control.rs` — accepts `String` body (NOT `web::Json` — jQuery doesn't send Content-Type header), parses JSON manually, extracts `property_id` field, validates non-empty, calls `db.update_device_property(udid, property_id)`, returns 404 if device not found
  - [x] 3.2 Request body format: `{"property_id": "HIH-PHO-12345"}` or `{"id": "HIH-PHO-12345", "id_number": "12345"}`
  - [x] 3.3 Return `HttpResponse::Ok().json(json!({"status": "success"}))` on success
- [x] Task 4: Add property page route and fix template (AC: #1, #2, #6)
  - [x] 4.1 Add `GET /devices/{udid}/property` route handler in `control.rs` — fetch device from DB, get current `property_id`, render `property.html` with Tera context containing `Udid` and `CurrentPropertyId`
  - [x] 4.2 Fix `property.html`: replace Go template `[[.]]` with Tera `{{ CurrentPropertyId }}`, convert form to JavaScript-driven submission using `$.ajax` POST to `/api/v1/devices/{{ Udid }}/property`, add redirect back to `/` on success
  - [x] 4.3 The form has two input modes: full ID text field (`name="id"`) and numeric suffix field (`name="id_number"` with `HIH-PHO-` prefix). JavaScript should check: if `id` field has value use it; else if `id_number` has value prepend `HIH-PHO-` prefix; else show error
- [x] Task 5: Register routes in `main.rs` (AC: #1, #3)
  - [x] 5.1 Add route: `.route("/devices/{udid}/property", web::get().to(routes::control::property_page))`
  - [x] 5.2 Add route: `.route("/api/v1/devices/{udid}/property", web::post().to(routes::control::update_device_property))`
- [x] Task 6: Regression testing (AC: #1-#7)
  - [x] 6.1 Build succeeds — 0 new warnings (5 pre-existing)
  - [x] 6.2 All existing tests pass (168/177, 9 pre-existing failures)
  - [x] 6.3 No new regressions introduced

## Dev Notes

### Critical Architecture Constraint

This story adds asset/inventory tracking to devices. The `property.html` template **already exists** as a basic HTML form (no Vue.js data binding). Unlike `edit.html` (Story 8.2), property.html is a simple Bootstrap form — it needs to be converted from a traditional form POST to JavaScript-driven AJAX submission.

**Key difference from Story 8.2**: `property_id` is TEXT (not INTEGER like `product_id`), because it stores alphanumeric inventory codes like "HIH-PHO-12345".

### Database Migration Pattern — Follow Product ID Migration

The `property_id` column addition must follow the same safe migration pattern used for `tags` and `product_id` at `sqlite.rs:225-237`:

```rust
// Migration: Add property_id column to existing databases
// This will silently fail if column already exists, which is fine
let _ = sqlx::query("ALTER TABLE devices ADD COLUMN property_id TEXT")
    .execute(&self.pool)
    .await;
```

Place this AFTER the product_id migration (line 237) and BEFORE the recordings table creation (line 239). **No index needed** — property_id lookups are always by udid (which is already indexed), not by property_id itself.

### Device Row JSON Conversion — NO Changes Needed

`device_row_to_json()` at `sqlite.rs:362-415` handles `property_id` automatically — TEXT columns fall through to the **default string handler** (lines 405-411). Unlike `product_id` (INTEGER, needed explicit addition to integer fields list), `property_id` (TEXT) needs no special handling.

### Device Info — property_id Already Included

Since `device_row_to_json()` includes all columns from the devices table, `property_id` will automatically appear in the device info JSON once the column is added. **No changes to `device_info()` handler needed** — ACs #4 and #5 are satisfied automatically by the migration.

### POST /api/v1/devices/{udid}/property — Handler Pattern

Follow the `update_device_product` pattern at `control.rs:361-407`:

```rust
pub async fn update_device_property(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: String,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    // Parse JSON body manually — form JS sends without Content-Type header
    let body: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::BadRequest().json(json!({"error": "Invalid JSON body"}));
        }
    };

    // Extract property_id — support both "property_id" and "id" fields
    let property_id = body.get("property_id")
        .or_else(|| body.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if property_id.is_empty() {
        return HttpResponse::BadRequest().json(json!({"error": "Missing property_id"}));
    }

    match state.db.update_device_property(&udid, property_id).await {
        Ok(true) => HttpResponse::Ok().json(json!({"status": "success"})),
        Ok(false) => HttpResponse::NotFound().json(json!({"error": "Device not found"})),
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({"error": format!("Database error: {}", e)})),
    }
}
```

**CRITICAL**: Use `body: String` NOT `web::Json<Value>` — jQuery `$.ajax` does not set `Content-Type: application/json` by default. This was a code review finding (H1) from Story 8.2.

### Property Page Route — Render with Current Value

Unlike `edit_page()` (Story 8.2) which only passes UDID, `property_page()` should fetch the device and pass the current `property_id` to the template:

```rust
pub async fn property_page(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
    let current_property_id = match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(device)) => device.get("property_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };

    let mut ctx = tera::Context::new();
    ctx.insert("Udid", &udid);
    ctx.insert("CurrentPropertyId", &current_property_id);

    match state.tera.render("property.html", &ctx) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}
```

### property.html Template Conversion

The existing `property.html` is a basic Bootstrap form that POSTs to `/property`. It needs:

1. **Fix Go template syntax**: Line 28: `value="[[.]]"` → `value="{{ CurrentPropertyId }}"`
2. **Remove form action**: Change `<form ... method="post" action="/property">` to just `<form class="form" id="propertyForm">`
3. **Add JavaScript**: AJAX POST to `/api/v1/devices/{{ Udid }}/property` on submit
4. **Logic for two input modes**: The form has two inputs — full ID text field and numeric suffix with `HIH-PHO-` prefix. JavaScript should prioritize the full ID field, fall back to prefix + number
5. **No `{% raw %}` needed**: Unlike edit.html, property.html does NOT use Vue.js template expressions (`{{ }}`). The only `{{ }}` are Tera variables. No Vue/Tera conflict exists.
6. **Add success/error feedback**: Show message after submit

**Template conversion approach:**
```html
<script>
  var udid = "{{ Udid }}";

  document.getElementById('propertyForm').addEventListener('submit', function(e) {
    e.preventDefault();
    var fullId = document.querySelector('input[name="id"]').value.trim();
    var numId = document.querySelector('input[name="id_number"]').value.trim();
    var propertyId = fullId || (numId ? 'HIH-PHO-' + numId : '');

    if (!propertyId) {
      alert('Please enter an asset number');
      return;
    }

    $.ajax({
      url: '/api/v1/devices/' + udid + '/property',
      method: 'POST',
      data: JSON.stringify({property_id: propertyId}),
    }).then(function() {
      window.location = '/';
    });
  });
</script>
```

### Validation Requirements

- `property_id` is a non-empty string (validated in handler)
- Device must exist (return 404 if not found via `rows_affected()` check)
- `property_id` can be any text format — "HIH-PHO-12345", "ASSET-001", custom strings

### What NOT to Implement

- Do NOT add product catalog features (Story 8.1)
- Do NOT add device-product association logic (Story 8.2)
- Do NOT add property search/listing endpoints — only save/display per device
- Do NOT add a service layer — direct handler-to-database calls are fine

### Project Structure Notes

- Database migration: `src/db/sqlite.rs` — `ensure_initialized()` + `update_device_property()`
- Property endpoint: `src/routes/control.rs` (add 2 handlers)
- Route registration: `src/main.rs` (add 2 routes)
- Template fix: `resources/templates/property.html` (convert form + fix syntax)

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 8, Story 8.3]
- [Source: docs/project-context.md#Tech Stack — sqlx 0.8, actix-web 4]
- [Source: src/db/sqlite.rs — ensure_initialized():137, device_row_to_json():362, product_id migration:229-237]
- [Source: src/routes/control.rs — device_info():296, edit_page():341 (template pattern), update_device_product():361 (handler pattern)]
- [Source: src/main.rs — route registration]
- [Source: resources/templates/property.html — existing form, line 28 template fix]
- [Source: _bmad-output/implementation-artifacts/8-2-device-product-association.md — Story 8.2 patterns + code review fixes]

### Git Context

Recent commits establish these patterns:
- `ead09d9` — recording CRUD with SQLite
- Story 8.1 established product CRUD, API response format `json!({"status": "success"})`
- Story 8.2 established device-product association, `String` body parsing (code review H1 fix), `rows_affected()` device validation (code review H2 fix)

### Previous Epic Intelligence (Story 8.2)

Story 8.2 code review fixes critical to this story:
- **H1 fix**: Use `body: String` + `serde_json::from_str` instead of `web::Json<Value>` — jQuery doesn't send Content-Type header
- **H2 fix**: Check `rows_affected() > 0` and return 404 for non-existent devices
- **M1 fix**: Log errors with `tracing::warn!` when database operations fail silently
- **Template fix**: `{% raw %}` blocks needed for Vue.js templates — NOT needed for property.html (no Vue expressions)

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

### Completion Notes List

- All 6 tasks completed successfully — property_id migration, DB method, POST endpoint, property page route, route registration, regression testing
- Applied Story 8.2 code review lessons: `body: String` (not `web::Json`), `rows_affected() > 0` for 404 detection
- property.html converted from traditional form POST to AJAX submission with dual input modes (full ID / numeric suffix with HIH-PHO- prefix)
- No `{% raw %}` blocks needed — property.html has no Vue.js expressions, only Tera variables
- Build: 0 new warnings, Tests: 168/177 pass (9 pre-existing failures, 0 new regressions)

### Code Review Fixes (2026-03-10)

- **M1 fix**: Added property_id length validation (max 100 chars) in `update_device_property` handler (`control.rs`)
- **M2 fix**: `property_page` now returns 404 for non-existent devices instead of rendering empty form (`control.rs`)
- **M3 fix**: Added `tracing::warn!` for database errors in both `property_page` and `update_device_property` (`control.rs`)
- **L1 fix**: Added `.trim()` on property_id to strip leading/trailing whitespace (`control.rs`)
- **M4 note**: No new test coverage added — tracked as technical debt (L2 from review)
- **L2 note**: Inconsistent API path patterns between Story 8.2 (`PUT /devices/{udid}/product`) and Story 8.3 (`POST /api/v1/devices/{udid}/property`) — follows respective story specs, tracked as technical debt

### File List

- src/db/sqlite.rs (add property_id migration + update_device_property method)
- src/routes/control.rs (add property_page handler + update_device_property handler)
- src/main.rs (register 2 new routes)
- resources/templates/property.html (fix template syntax + convert to AJAX submission)
