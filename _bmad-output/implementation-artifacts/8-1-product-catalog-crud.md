# Story 8.1: Product Catalog CRUD

Status: done

## Story

As a **device farm administrator**,
I want **to manage a catalog of device products (brand, model, CPU, GPU, specs)**,
so that **I can standardize device information across the farm**.

## Acceptance Criteria

1. **Given** the server is running **When** the database is initialized **Then** a `products` table exists with fields: id (INTEGER PK AUTOINCREMENT), brand (TEXT NOT NULL), model (TEXT NOT NULL), name (TEXT), cpu (TEXT), gpu (TEXT), link (TEXT), coverage (INTEGER)
2. **Given** the server is running **When** I call `GET /api/v1/products` **Then** all products are returned as a JSON array
3. **Given** products exist in the database **When** I call `GET /api/v1/products?brand=Samsung&model=Galaxy` **Then** only matching products are returned (partial match on both fields)
4. **Given** valid product data **When** I call `POST /api/v1/products` with `{brand, model, name, cpu, gpu, link, coverage}` **Then** a new product is created and the created product (with id) is returned
5. **Given** a product exists with id=1 **When** I call `PUT /api/v1/products/1` with updated fields **Then** the product is updated and the updated product is returned
6. **Given** a product exists with id=1 **When** I call `GET /api/v1/products/1` **Then** the product is returned as JSON
7. **Given** a product with id=1 exists **When** I call `DELETE /api/v1/products/1` **Then** the product is deleted and success is returned

## Tasks / Subtasks

- [x] Task 1: Create `products` table in database schema (AC: #1)
  - [x] 1.1 Add `CREATE TABLE IF NOT EXISTS products` to `ensure_initialized()` in `src/db/sqlite.rs`
  - [x] 1.2 Add indexes: `idx_products_brand` on `brand`, `idx_products_brand_model` on `(brand, model)`
- [x] Task 2: Add product database methods to `sqlite.rs` (AC: #2, #3, #4, #5, #6, #7)
  - [x] 2.1 `create_product(brand, model, name, cpu, gpu, link, coverage) -> Result<Product>`
  - [x] 2.2 `get_product(id) -> Result<Option<Product>>`
  - [x] 2.3 `list_products(brand_filter, model_filter) -> Result<Vec<Product>>` — optional WHERE clauses with LIKE for partial match
  - [x] 2.4 `update_product(id, fields) -> Result<Option<Product>>`
  - [x] 2.5 `delete_product(id) -> Result<bool>`
- [x] Task 3: Create Product model struct (AC: #4, #5, #6)
  - [x] 3.1 Create `src/models/product.rs` with `Product` struct (Serialize, Deserialize, sqlx::FromRow)
  - [x] 3.2 Create `CreateProductRequest` and `UpdateProductRequest` structs for API input validation
  - [x] 3.3 Add `mod product;` to `src/models/mod.rs`
- [x] Task 4: Create API endpoints in `api_v1.rs` (AC: #2, #3, #4, #5, #6, #7)
  - [x] 4.1 `GET /api/v1/products` — list all products, with optional `?brand=X&model=Y` query filters
  - [x] 4.2 `GET /api/v1/products/{id}` — get single product by ID
  - [x] 4.3 `POST /api/v1/products` — create product from JSON body
  - [x] 4.4 `PUT /api/v1/products/{id}` — update product from JSON body
  - [x] 4.5 `DELETE /api/v1/products/{id}` — delete product by ID
- [x] Task 5: Register routes in `main.rs` (AC: #2-#7)
  - [x] 5.1 Add all 5 product routes under the `/api/v1/` section
- [x] Task 6: Add legacy-compatible endpoints for `edit.html` (AC: #3)
  - [x] 6.1 Add `GET /products/{brand}/{model}` route that queries products by exact brand+model match — `edit.html` already calls this URL (see Story 8.2 context)
- [x] Task 7: Regression testing (AC: #1-#7)
  - [x] 7.1 Build succeeds — 0 new warnings (5 pre-existing)
  - [x] 7.2 All existing tests pass (168/177, 9 pre-existing failures)
  - [x] 7.3 No new regressions introduced

## Dev Notes

### Critical Architecture Constraint

This is a **backend-only story** — do NOT modify `edit.html` or any frontend files. Story 8.2 handles the frontend integration. This story creates the data layer and API endpoints.

### Database Pattern — Follow Existing Schema Style

The `ensure_initialized()` method in `sqlite.rs:137` uses `CREATE TABLE IF NOT EXISTS` for schema creation. Follow this exact pattern:

```sql
CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    brand TEXT NOT NULL,
    model TEXT NOT NULL,
    name TEXT,
    cpu TEXT,
    gpu TEXT,
    link TEXT,
    coverage INTEGER DEFAULT 0
)
```

**Index pattern** (see `sqlite.rs:191-198`):
```sql
CREATE INDEX IF NOT EXISTS idx_products_brand ON products(brand);
CREATE INDEX IF NOT EXISTS idx_products_brand_model ON products(brand, model);
```

### Database Method Pattern — Follow `sqlite.rs` Conventions

The database uses `sqlx 0.8` with `SqlitePool`. Existing patterns:

**Insert pattern** (see `recordings` in sqlite.rs):
```rust
let result = sqlx::query("INSERT INTO products (brand, model, name, cpu, gpu, link, coverage) VALUES (?, ?, ?, ?, ?, ?, ?)")
    .bind(&brand)
    .bind(&model)
    // ...
    .execute(&self.pool)
    .await?;
let id = result.last_insert_rowid();
```

**Query with FromRow** (preferred for typed results):
```rust
let products: Vec<Product> = sqlx::query_as::<_, Product>("SELECT * FROM products WHERE brand LIKE ? AND model LIKE ?")
    .bind(format!("%{}%", brand))
    .bind(format!("%{}%", model))
    .fetch_all(&self.pool)
    .await?;
```

**Pool access**: The `Database` struct holds `pool: SqlitePool` directly. Methods are `impl Database`.

### Model Pattern — Follow Existing Convention

See `src/models/recording.rs` for the closest pattern:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Product {
    pub id: i64,
    pub brand: String,
    pub model: String,
    pub name: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    pub link: Option<String>,
    pub coverage: Option<i64>,
}
```

Request structs don't need `sqlx::FromRow`:
```rust
#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub brand: String,
    pub model: String,
    pub name: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    pub link: Option<String>,
    pub coverage: Option<i64>,
}
```

### API Response Pattern — Follow `api_v1.rs` Convention

All API v1 endpoints return JSON with status wrapper:

```rust
// Success
HttpResponse::Ok().json(json!({
    "status": "success",
    "data": product_or_list
}))

// Error - not found
HttpResponse::NotFound().json(json!({
    "status": "error",
    "error": "ERR_PRODUCT_NOT_FOUND",
    "message": format!("Product {} not found", id)
}))

// Error - bad request
HttpResponse::BadRequest().json(json!({
    "status": "error",
    "error": "ERR_INVALID_REQUEST",
    "message": "Brand and model are required"
}))
```

### Route Registration Pattern — Follow `main.rs` Convention

Routes are registered in `main.rs` inside the `HttpServer::new` closure. Add product routes near the existing `/api/v1/` block (lines 275-296):

```rust
// ── Product Catalog API ──
.route("/api/v1/products", web::get().to(routes::api_v1::list_products))
.route("/api/v1/products", web::post().to(routes::api_v1::create_product))
.route("/api/v1/products/{id}", web::get().to(routes::api_v1::get_product))
.route("/api/v1/products/{id}", web::put().to(routes::api_v1::update_product))
.route("/api/v1/products/{id}", web::delete().to(routes::api_v1::delete_product))
// Legacy endpoint for edit.html compatibility (Story 8.2)
.route("/products/{brand}/{model}", web::get().to(routes::api_v1::list_products_by_brand_model))
```

### edit.html Context (Story 8.2 — DO NOT MODIFY NOW)

`edit.html` already expects these calls — this is context for future reference:
- `GET /products/{brand}/{model}` → returns JSON array of products (line 81)
- `PUT /devices/{udid}/product` → saves product association (line 110, handled in Story 8.2)

Adding the legacy `GET /products/{brand}/{model}` endpoint in this story enables the existing `edit.html` to start working with real data once Story 8.2 adds the device-product association.

### Validation Requirements

- `brand` and `model` are **required** (NOT NULL) on create
- All other fields are optional
- `coverage` is an integer (percentage, 0-100)
- `id` is auto-generated, never accepted from client on create

### What NOT to Implement

- Do NOT modify `edit.html` (Story 8.2)
- Do NOT add device-product association (Story 8.2)
- Do NOT add property/asset tracking (Story 8.3)
- Do NOT add a product service layer — direct database calls from route handlers is fine for CRUD

### Project Structure Notes

- Database schema: `src/db/sqlite.rs` — `ensure_initialized()` method
- Model: `src/models/product.rs` (new file)
- Models module: `src/models/mod.rs` (add `mod product;`)
- API routes: `src/routes/api_v1.rs` (add product handlers)
- Route registration: `src/main.rs` (add product routes near line 296)
- Existing model patterns: `src/models/recording.rs`, `src/models/device.rs`

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 8, Story 8.1]
- [Source: docs/project-context.md#Tech Stack — sqlx 0.8, actix-web 4]
- [Source: src/db/sqlite.rs — ensure_initialized():137, Database methods]
- [Source: src/routes/api_v1.rs — API v1 endpoint patterns]
- [Source: src/main.rs — route registration at lines 275-296]
- [Source: src/models/recording.rs — model struct pattern with sqlx::FromRow]
- [Source: resources/templates/edit.html — existing product UI expectations (Story 8.2 context)]

### Git Context

Recent commits establish these patterns:
- `ead09d9` — recording CRUD with SQLite (closest pattern for product CRUD)
- `d74fd69` — code review fixes, consistent error response patterns
- API v1 uses `json!({ "status": "success", "data": ... })` wrapper

### Previous Epic Intelligence (Epic 7)

Epic 7 retrospective action items relevant to this story:
- **Trace data flow** — When creating the products table, verify the schema matches what `edit.html` expects (fields: id, brand, model, name, cpu, gpu, link, coverage)
- **Remove dead code in same commit** — Don't leave placeholder or TODO code
- **Verify against actual usage** — The `edit.html` template uses `v.id` as option value, expects array response from product list

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

N/A — no debug issues encountered.

### Completion Notes List

1. Products table created in `ensure_initialized()` with all 7 fields + 2 indexes (Task 1)
2. 6 database methods added: `create_product`, `get_product`, `list_products`, `update_product`, `delete_product`, `list_products_by_brand_model` (Task 2)
3. `Product`, `CreateProductRequest`, `UpdateProductRequest` structs created with `sqlx::FromRow` (Task 3)
4. 6 API handlers added to `api_v1.rs` with `json!({"status": "success", "data": ...})` wrapper (Task 4)
5. All 6 routes registered in `main.rs` including legacy `/products/{brand}/{model}` (Tasks 5 & 6)
6. Build: 0 new warnings (5 pre-existing), 168/177 tests pass (9 pre-existing failures), 0 regressions (Task 7)
7. `list_products` supports partial match via LIKE with optional `?brand=X&model=Y` query params
8. `list_products_by_brand_model` uses exact match for legacy `edit.html` compatibility
9. `create_product` validates brand/model are non-empty, returns 400 on empty values
10. `update_product` uses dynamic SET clause — only updates fields present in request body

### Code Review Fixes

- **H1 fix**: Added brand/model empty-string validation to `update_product` handler — returns 400 if brand or model is present but empty/whitespace-only
- **M1 fix**: Escaped `%` and `_` LIKE wildcards in `list_products` filter values with `ESCAPE '\'` clause
- **M2 fix**: Empty string query params (`?brand=`) now treated as absent via `.filter(|s| !s.is_empty())`
- **L1 fix**: `create_product` now trims brand/model before storing to prevent whitespace in exact-match lookups

### File List

- src/db/sqlite.rs (add products table in ensure_initialized + 6 CRUD methods + LIKE escape fix)
- src/models/product.rs (new — Product, CreateProductRequest, UpdateProductRequest structs)
- src/models/mod.rs (add mod product)
- src/routes/api_v1.rs (add 6 product CRUD + legacy handlers + validation fixes)
- src/main.rs (register 6 product routes)
