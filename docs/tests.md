# Tests

## Test Suite Overview

**Total Tests:** 90

| Category | Count | Location |
|----------|-------|----------|
| Unit Tests | 52 | src/**/*.rs (#[cfg(test)]) |
| Integration - Config | 3 | tests/test_config.rs |
| Integration - Database | 8 | tests/test_database.rs |
| Integration - Services | 7 | tests/test_services.rs |
| E2E - HTTP | 20 | tests/test_server.rs |

---

## Running Tests

```bash
# All tests
cargo test

# Unit tests only
cargo test --lib

# Specific integration test
cargo test --test test_database

# E2E tests
cargo test --test test_server

# With output
cargo test -- --nocapture

# Single test
cargo test test_upsert_insert
```

---

## Test Utilities

**File:** `tests/common/mod.rs`

Key functions:
- `create_temp_db()` - Creates temporary SQLite database
- `make_device_json(udid, present, is_mock)` - Builds test device JSON
- `create_test_app_state()` - Creates full AppState for testing

---

## Unit Test Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_pool() {
        let pool = ConnectionPool::new(10, Duration::from_secs(60));
        let client = pool.get_or_create("d1", "10.0.0.1", 7912).await;
        assert_eq!(client.udid, "d1");
    }
}
```

---

## Integration Test Pattern

```rust
mod common;
use common::{create_temp_db, make_device_json};

#[tokio::test]
async fn test_phone_service_crud() {
    let (_tmp, db) = create_temp_db().await;
    let svc = PhoneService::new(db);

    svc.update_field("dev-1", &make_device_json("dev-1", true, false)).await.unwrap();
    let result = svc.query_info_by_udid("dev-1").await.unwrap();
    assert!(result.is_some());
}
```

---

## E2E Test Pattern

```rust
#[actix_web::test]
async fn test_device_list_endpoint() {
    let (tmp, state) = create_test_app_state().await;
    let app = test::init_service(
        App::new().app_data(web::Data::new(state.clone()))
                  .route("/list", web::get().to(device_list))
    ).await;

    let req = test::TestRequest::get().uri("/list").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}
```

---

## Test Coverage by File

| File | Tests | Purpose |
|------|-------|---------|
| sqlite.rs | 12 | Database CRUD, field mapping |
| device.rs | 4 | Device model, serde |
| connection_pool.rs | 4 | Pool operations |
| batch_processor.rs | 3 | Batch processing |
| phone_service.rs | 5 | Device lifecycle |
| test_config.rs | 3 | Config loading |
| test_database.rs | 8 | Database operations |
| test_services.rs | 7 | Service layer |
| test_server.rs | 20 | HTTP endpoints |
