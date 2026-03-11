# Story 8.2: Device-Product Association

Status: done

## Story

As a **device farm administrator**,
I want **to link a connected device to a product catalog entry via the edit page**,
so that **each device has standardized hardware specifications**.

## Acceptance Criteria

1. **Given** a device is connected and a product exists in the catalog **When** I open `/devices/{udid}/edit` **Then** the edit page loads with the device info and a product selector dropdown
2. **Given** the edit page is open **When** I select a product from the dropdown **Then** the product fields (name, cpu, gpu, link, coverage) are populated in the form
3. **Given** I have selected a product **When** I call `PUT /devices/{udid}/product` with the product data **Then** the product_id is saved to the device record and the updated device (with product) is returned
4. **Given** a device has a linked product **When** I call `GET /devices/{udid}/info` **Then** the response includes a `product` field with the full Product object
5. **Given** a device has no linked product **When** I call `GET /devices/{udid}/info` **Then** the response includes `product: null` or `product: {}`
6. **Given** the edit page loads **When** device has brand and model **Then** `GET /products/{brand}/{model}` fetches matching products for the dropdown (already implemented in Story 8.1)
7. **Given** the `edit.html` template uses `[[.]]` for UDID **When** the server renders the page **Then** the Tera template engine correctly injects the device UDID

## Tasks / Subtasks

- [x] Task 1: Add `product_id` column to devices table (AC: #3, #4)
  - [x] 1.1 Add migration in `ensure_initialized()`: `ALTER TABLE devices ADD COLUMN product_id INTEGER` (silently fail if exists — same pattern as tags migration at `sqlite.rs:225-227`)
  - [x] 1.2 Add index: `CREATE INDEX IF NOT EXISTS idx_devices_product_id ON devices(product_id)`
- [x] Task 2: Add device-product database methods (AC: #3, #4, #5)
  - [x] 2.1 `update_device_product(udid: &str, product_id: i64) -> Result<(), sqlx::Error>` — UPDATE devices SET product_id = ? WHERE udid = ?
  - [x] 2.2 Modify `device_row_to_json()` — add `product_id` to integer fields list (like `port` and `sdk`)
  - [x] 2.3 `get_device_with_product(udid: &str) -> Result<Option<Value>, sqlx::Error>` — product merge handled in device_info() handler instead of separate DB method (cleaner — avoids duplicating device query logic)
- [x] Task 3: Create `PUT /devices/{udid}/product` endpoint (AC: #3)
  - [x] 3.1 Add handler `update_device_product` in `control.rs` — accepts JSON body with product `id` field, validates product exists via `db.get_product(id)`, updates device's `product_id` via `db.update_device_product(udid, id)`, returns updated device with product
  - [x] 3.2 Request body: accept `{"id": 123, ...}` — extract `id` field from the Product object that `edit.html` sends (line 112: `JSON.stringify(this.product)`)
  - [x] 3.3 Return `HttpResponse::Ok().json(json!({"status": "success"}))` on success
- [x] Task 4: Enhance `GET /devices/{udid}/info` to include product data (AC: #4, #5)
  - [x] 4.1 Modify `device_info()` handler in `control.rs` — after fetching device, check if `product_id` is set, if so fetch Product and insert as `"product"` field into the device JSON response
  - [x] 4.2 If no product_id or product not found, set `"product"` to `null` in response
- [x] Task 5: Add edit page route and fix template (AC: #1, #7)
  - [x] 5.1 Add `GET /devices/{udid}/edit` route handler in `control.rs` — render `edit.html` with Tera context containing UDID
  - [x] 5.2 Fix `edit.html` line 52: change `var udid = "[[.]]"` to `var udid = "{{ Udid }}"` — also added `{% raw %}`/`{% endraw %}` blocks around Vue template to prevent Tera/Vue `{{ }}` delimiter conflict
  - [x] 5.3 Register route in `main.rs`: `.route("/devices/{udid}/edit", web::get().to(routes::control::edit_page))`
- [x] Task 6: Register `PUT /devices/{udid}/product` route (AC: #3)
  - [x] 6.1 Add route in `main.rs`: `.route("/devices/{udid}/product", web::put().to(routes::control::update_device_product))`
- [x] Task 7: Regression testing (AC: #1-#7)
  - [x] 7.1 Build succeeds — 0 new warnings (5 pre-existing)
  - [x] 7.2 All existing tests pass (168/177, 9 pre-existing failures)
  - [x] 7.3 No new regressions introduced

## Dev Notes

### Critical Architecture Constraint

This story bridges the backend product catalog (Story 8.1) with the frontend edit page. The `edit.html` template **already has all the frontend code written** — it expects:
1. `GET /devices/{udid}/info` to return device data with `.product` field
2. `GET /products/{brand}/{model}` to return product array (already working from Story 8.1)
3. `PUT /devices/{udid}/product` to save the association
4. A route to serve the edit page itself

**DO NOT modify `edit.html` beyond fixing the template syntax** (`[[.]]` → `{{ Udid }}`). The Vue.js app is complete and functional.

### Database Migration Pattern — Follow Tags Column Migration

The `product_id` column addition must follow the same safe migration pattern used for `tags` at `sqlite.rs:225-227`:

```rust
// Migration: Add product_id column to existing databases
// This will silently fail if column already exists, which is fine
let _ = sqlx::query("ALTER TABLE devices ADD COLUMN product_id INTEGER")
    .execute(&self.pool)
    .await;
```

Place this AFTER the existing tags migration (line 227) and BEFORE the recordings table creation (line 229).

### Device Row JSON Conversion — Critical Pattern

`device_row_to_json()` at `sqlite.rs:326-380` handles column-to-JSON conversion. The `product_id` field must be added to the **integer fields** section alongside `port` and `sdk` (line 362):

```rust
else if col == "port" || col == "sdk" || col == "product_id" {
    let v: Option<i64> = row.try_get(col).ok().flatten();
    // ...
}
```

Without this, `product_id` would be treated as a string field and serialized incorrectly.

### Device Info Enhancement — Merge Product Object

The `device_info()` handler at `control.rs:296-311` currently returns raw device JSON. To include the product, modify it to:

1. Fetch device via `phone_service.query_info_by_udid()`
2. Check if device has `product_id` (non-null integer)
3. If yes, fetch product via `db.get_product(product_id)`
4. Insert `"product"` key into device JSON object
5. If no product_id or product not found, insert `"product": null`

```rust
// After getting device JSON:
let mut device_obj = device.clone();
if let Some(product_id) = device.get("product_id").and_then(|v| v.as_i64()) {
    if let Ok(Some(product)) = state.db.get_product(product_id).await {
        if let Some(obj) = device_obj.as_object_mut() {
            obj.insert("product".to_string(), serde_json::to_value(&product).unwrap_or_default());
        }
    }
}
```

### PUT /devices/{udid}/product — Request Body Format

`edit.html` sends the full Product object at line 112: `JSON.stringify(this.product)`. The body will look like:

```json
{
  "id": 42,
  "brand": "Samsung",
  "model": "Galaxy S24",
  "name": "Galaxy S24 Ultra",
  "cpu": "Snapdragon 8 Gen 3",
  "gpu": "Adreno 750",
  "link": "https://...",
  "coverage": 85
}
```

The handler should:
1. Parse the body to extract the `id` field (the product ID)
2. Validate the product exists via `db.get_product(id)`
3. Update the device: `db.update_device_product(udid, id)`
4. Return success response

Use `serde_json::Value` for the body to flexibly extract the `id` field without needing a dedicated request struct.

### Edit Page Route — Follow Remote Page Pattern

The edit page handler should follow the `remote()` pattern at `control.rs:120-145`:

```rust
pub async fn edit_page(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let mut ctx = tera::Context::new();
    ctx.insert("Udid", &udid);

    match state.tera.render("edit.html", &ctx) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}
```

### edit.html Template Syntax Fix

Line 52 has Go template syntax that won't work with Tera:
```javascript
// BEFORE (Go template — won't work):
var udid = "[[.]]";

// AFTER (Tera template):
var udid = "{{ Udid }}";
```

### Validation Requirements

- `product_id` must reference a valid product (check `db.get_product()` before saving)
- Return 404 if device not found
- Return 404 if product not found
- `product_id` is nullable — devices can exist without a product association

### What NOT to Implement

- Do NOT modify the Vue.js app in `edit.html` (only fix `[[.]]` → `{{ Udid }}`)
- Do NOT add asset property tracking (Story 8.3)
- Do NOT add product creation from the edit page
- Do NOT add a service layer — direct handler-to-database calls are fine

### Project Structure Notes

- Database schema/migration: `src/db/sqlite.rs` — `ensure_initialized()` + `device_row_to_json()`
- Device-product endpoint: `src/routes/control.rs` (add handler)
- Edit page route: `src/routes/control.rs` (add handler)
- Route registration: `src/main.rs` (add 2 routes)
- Template fix: `resources/templates/edit.html` (line 52 only)
- Existing model: `src/models/product.rs` (already has Product struct with `sqlx::FromRow`)

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 8, Story 8.2]
- [Source: docs/project-context.md#Tech Stack — sqlx 0.8, actix-web 4]
- [Source: src/db/sqlite.rs — ensure_initialized():137, device_row_to_json():326, tags migration:225]
- [Source: src/routes/control.rs — device_info():296, remote():120 (template pattern)]
- [Source: src/main.rs — route registration]
- [Source: resources/templates/edit.html — complete frontend, line 52 template fix, line 110 PUT call]
- [Source: src/models/product.rs — Product struct with sqlx::FromRow]
- [Source: _bmad-output/implementation-artifacts/8-1-product-catalog-crud.md — Story 8.1 patterns]

### Git Context

Recent commits establish these patterns:
- `ead09d9` — recording CRUD with SQLite (closest pattern for device-product association)
- Story 8.1 established product CRUD, API response format `json!({"status": "success", "data": ...})`
- Tags migration pattern: `ALTER TABLE ... ADD COLUMN` with silent failure (sqlite.rs:225-227)

### Previous Epic Intelligence (Story 8.1)

Story 8.1 code review fixes relevant to this story:
- **Trim brand/model** — Product data is trimmed before storage. The edit page sends product data through PUT; this should be consistent
- **LIKE wildcard escaping** — Not directly relevant but shows attention to SQL safety
- **Empty string validation** — Brand/model cannot be empty on create/update. The PUT endpoint here saves product_id, not product fields, so this is less relevant
- **Legacy endpoint returns raw array** — `GET /products/{brand}/{model}` returns `[...]` not `{"status":"success","data":[...]}`. edit.html expects this format at line 86: `this.products = ret`

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

N/A — no debug issues encountered.

### Completion Notes List

1. `product_id` INTEGER column added to devices table via safe ALTER TABLE migration (silent fail if exists), with index `idx_devices_product_id` (Task 1)
2. `update_device_product(udid, product_id)` DB method added for saving device-product association (Task 2.1)
3. `device_row_to_json()` updated — `product_id` added to integer fields alongside `port` and `sdk` (Task 2.2)
4. Product merge into device JSON handled in `device_info()` handler rather than separate DB method — cleaner approach avoiding duplicate device query logic (Task 2.3)
5. `device_info()` enhanced — fetches Product by `product_id` if set, inserts as `"product"` field; returns `null` if no product linked (Tasks 4.1, 4.2)
6. `update_device_product()` handler — extracts `id` from JSON body, validates product exists via `get_product()`, updates device's `product_id`, returns `{"status": "success"}` (Task 3)
7. `edit_page()` handler — renders `edit.html` with Tera context containing `Udid` variable (Task 5.1)
8. Template fix: `[[.]]` → `{{ Udid }}` + added `{% raw %}`/`{% endraw %}` blocks to prevent Tera from processing Vue.js `{{ }}` expressions (Task 5.2)
9. Both routes registered in `main.rs`: `/devices/{udid}/edit` (GET) and `/devices/{udid}/product` (PUT) (Tasks 5.3, 6.1)
10. Build: 0 new warnings (5 pre-existing), 168/177 tests pass (9 pre-existing failures), 0 regressions (Task 7)

### Code Review Fixes

- **H1 fix**: Changed `update_device_product` from `web::Json<Value>` to `String` body with manual `serde_json::from_str` — jQuery `$.ajax` sends `JSON.stringify()` without `Content-Type: application/json`, which actix-web's `web::Json` extractor rejects
- **H2 fix**: `update_device_product` DB method now returns `bool` via `rows_affected() > 0`; handler returns 404 if device not found instead of silent success
- **M1 fix**: `device_info` now logs `tracing::warn!` when `get_product()` fails, instead of silently falling back to `null`

### File List

- src/db/sqlite.rs (add product_id migration + update_device_product method + device_row_to_json integer field fix)
- src/routes/control.rs (add edit_page handler + update_device_product handler + enhance device_info with product merge)
- src/main.rs (register 2 new routes: /devices/{udid}/edit, /devices/{udid}/product)
- resources/templates/edit.html (fix line 52: [[.]] → {{ Udid }}, add {% raw %}/{% endraw %} for Vue.js compatibility)
