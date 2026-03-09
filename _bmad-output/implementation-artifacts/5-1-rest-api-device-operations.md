# Story 5.1: REST API Device Operations

Epic: 5 (External API & CI/CD Integration)
Status: done
Priority: P2

## Story

As an **Automation Engineer**, I want comprehensive REST API documentation and standardized endpoints for all device operations, so that I can integrate cloudcontrol-rust with CI/CD pipelines and external tools.

## Acceptance Criteria

```gherkin
Feature: API Device List
  Given a valid API request
  When I request GET /api/v1/devices
  Then I receive a JSON list of all connected devices
  And each device includes udid, model, status, ip, port, battery, screen resolution
  And response follows OpenAPI 3.0 spec

Feature: API Device Info
  Given a device with UDID "abc123" is connected
  When I request GET /api/v1/devices/abc123
  Then I receive detailed device information
  And response includes all metadata (model, android_version, battery, display, serial)
  And response includes connection status and last_seen timestamp

Feature: API Screenshot Capture
  Given a device is connected
  When I request GET /api/v1/devices/{udid}/screenshot
  Then I receive a screenshot in JSON format
  And response includes base64-encoded JPEG data
  And I can optionally request PNG format via ?format=png
  And I can specify quality via ?quality=50

Feature: API Touch Command
  Given a device is connected
  When I POST to /api/v1/devices/{udid}/tap with {"x": 540, "y": 960}
  Then the tap is executed on the device
  And response confirms success with timestamp
  And response time is <100ms (NFR3)

Feature: API Error Handling
  Given a device with UDID "nonexistent" is not connected
  When I request GET /api/v1/devices/nonexistent
  Then I receive HTTP 404 Not Found
  And response body includes error code "ERR_DEVICE_NOT_FOUND"
  And response includes descriptive error message

Feature: API Batch Operations
  Given 3 devices are connected
  When I POST to /api/v1/batch/tap with {"udids": ["a", "b", "c"], "x": 100, "y": 200}
  Then tap is executed on all 3 devices
  And response includes per-device results
  And response includes success/failure count

Feature: OpenAPI Documentation
  Given the API is implemented
  When I request GET /api/v1/openapi.json
  Then I receive a valid OpenAPI 3.0 specification
  And spec includes all /api/v1/* endpoints
  And spec includes request/response schemas
```

## Tasks / Subtasks

- [ ] Task 1: Create API versioning structure (AC: all)
  - [ ] Add /api/v1 prefix to all new API endpoints
  - [ ] Create `src/routes/api_v1.rs` module
  - [ ] Add ApiV1Router with versioned endpoints
  - [ ] Maintain backward compatibility with existing /inspector/* endpoints

- [ ] Task 2: Standardize API response format (AC: 4, 5)
  - [ ] Create consistent success response wrapper
  - [ ] Create consistent error response with codes
  - [ ] Add error codes: ERR_DEVICE_NOT_FOUND, ERR_DEVICE_DISCONNECTED, ERR_INVALID_REQUEST, ERR_OPERATION_FAILED
  - [ ] Ensure all API v1 endpoints use standardized format

- [ ] Task 3: Implement device API endpoints (AC: 1, 2)
  - [ ] GET /api/v1/devices - List all devices
  - [ ] GET /api/v1/devices/{udid} - Get single device info
  - [ ] Include full metadata in responses

- [ ] Task 4: Implement screenshot API endpoint (AC: 3)
  - [ ] GET /api/v1/devices/{udid}/screenshot
  - [ ] Add ?format=jpeg|png query parameter
  - [ ] Add ?quality=1-100 query parameter
  - [ ] Return standardized JSON response with base64 data

- [ ] Task 5: Implement control API endpoints (AC: 4)
  - [ ] POST /api/v1/devices/{udid}/tap
  - [ ] POST /api/v1/devices/{udid}/swipe
  - [ ] POST /api/v1/devices/{udid}/input
  - [ ] POST /api/v1/devices/{udid}/keyevent
  - [ ] Return standardized success/error responses

- [ ] Task 6: Implement batch API endpoints (AC: 6)
  - [ ] POST /api/v1/batch/tap
  - [ ] POST /api/v1/batch/swipe
  - [ ] POST /api/v1/batch/input
  - [ ] Wrap existing batch endpoints with v1 prefix and standardized responses

- [ ] Task 7: Create OpenAPI specification (AC: 7)
  - [ ] Create `src/models/openapi.rs` with schema definitions
  - [ ] Implement GET /api/v1/openapi.json endpoint
  - [ ] Include all request/response schemas
  - [ ] Add examples for each endpoint

- [ ] Task 8: Write E2E tests
  - [ ] Test device list API
  - [ ] Test device info API
  - [ ] Test screenshot API with format options
  - [ ] Test control operations via API
  - [ ] Test error responses (404, 400, 500)
  - [ ] Test batch operations via API
  - [ ] Test OpenAPI spec is valid JSON

## Dev Notes

### Architecture Context

This story creates the **versioned REST API** for external application integration. The MVP phase implemented all device operations via the `/inspector/*` endpoints designed for browser clients. Epic 5 creates API-first endpoints under `/api/v1/*` with:

1. **Consistent Response Format**: Standard JSON structure for all responses
2. **Error Codes**: Machine-parseable error codes
3. **OpenAPI Spec**: Self-documenting API
4. **Versioning**: Future-proof API evolution

### Existing Endpoints to Wrap

The following endpoints already exist in `src/routes/control.rs` and will be re-implemented under `/api/v1/*` with standardized responses:

| Existing Endpoint | New API V1 Endpoint | Notes |
|-------------------|---------------------|-------|
| GET /list | GET /api/v1/devices | Same functionality, standardized response |
| GET /devices/{udid}/info | GET /api/v1/devices/{udid} | Standardized response |
| GET /inspector/{udid}/screenshot | GET /api/v1/devices/{udid}/screenshot | Add format/quality params |
| POST /inspector/{udid}/touch | POST /api/v1/devices/{udid}/tap | Rename touch→tap for clarity |
| POST /inspector/{udid}/swipe | POST /api/v1/devices/{udid}/swipe | Same path |
| POST /inspector/{udid}/input | POST /api/v1/devices/{udid}/input | Same path |
| POST /inspector/{udid}/keyevent | POST /api/v1/devices/{udid}/keyevent | Same path |
| POST /api/batch/tap | POST /api/v1/batch/tap | Standardized response |
| POST /api/batch/swipe | POST /api/v1/batch/swipe | Standardized response |
| POST /api/batch/input | POST /api/v1/batch/input | Standardized response |

### API Response Format

All `/api/v1/*` endpoints follow this standardized format:

**Success Response:**
```json
{
  "status": "success",
  "data": { ... },
  "timestamp": "2026-03-08T12:00:00Z"
}
```

**Error Response:**
```json
{
  "status": "error",
  "error": "ERR_DEVICE_NOT_FOUND",
  "message": "Device with UDID 'abc123' not found",
  "timestamp": "2026-03-08T12:00:00Z"
}
```

### Error Code Definitions

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `ERR_DEVICE_NOT_FOUND` | 404 | Device UDID not in system |
| `ERR_DEVICE_DISCONNECTED` | 503 | Device exists but connection lost |
| `ERR_INVALID_REQUEST` | 400 | Malformed request body/parameters |
| `ERR_OPERATION_FAILED` | 500 | Device operation failed |
| `ERR_NO_DEVICES_SELECTED` | 400 | Batch operation with empty device list |
| `ERR_BATCH_PARTIAL_FAILURE` | 207 | Batch operation partially succeeded |

### File Structure

```
src/
├── routes/
│   ├── api_v1.rs       # NEW - API v1 endpoints
│   ├── control.rs      # EXISTING - Keep for backward compatibility
│   └── mod.rs          # UPDATE - Add api_v1 module
├── models/
│   ├── api_response.rs # NEW - Standardized response types
│   └── openapi.rs      # NEW - OpenAPI spec model
└── main.rs             # UPDATE - Register API v1 routes
```

### Testing Requirements

- NFR3: API response time <100ms (non-streaming)
- NFR18: REST API compatibility - OpenAPI 3.0 spec compliant
- All existing tests must pass (backward compatibility)
- New tests for API v1 endpoints

### Previous Story Learnings (4-2 Synchronized Batch Operations)

1. **Batch pattern**: Use `futures::future::join_all` for parallel execution
2. **Error handling**: Return per-device error codes in batch responses
3. **HTTP status**: Returns 200 OK even for partial failures (errors in response body)
4. **MAX_BATCH_SIZE**: 20 devices per batch operation

### Implementation Pattern

```rust
// src/routes/api_v1.rs

use actix_web::{web, HttpResponse};
use serde_json::json;

/// GET /api/v1/devices - List all devices
pub async fn list_devices(state: web::Data<AppState>) -> HttpResponse {
    let phone_service = PhoneService::new(state.db.clone());
    match phone_service.query_device_list_by_present().await {
        Ok(devices) => HttpResponse::Ok().json(json!({
            "status": "success",
            "data": devices,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => error_response(500, "ERR_OPERATION_FAILED", &e),
    }
}

/// GET /api/v1/devices/{udid} - Get device info
pub async fn get_device(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    let phone_service = PhoneService::new(state.db.clone());
    match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(device)) => HttpResponse::Ok().json(json!({
            "status": "success",
            "data": device,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Ok(None) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_DEVICE_NOT_FOUND",
            "message": format!("Device '{}' not found", udid),
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => error_response(500, "ERR_OPERATION_FAILED", &e),
    }
}
```

### Route Registration in main.rs

```rust
// Add API v1 routes
.route("/api/v1/devices", web::get().to(routes::api_v1::list_devices))
.route("/api/v1/devices/{udid}", web::get().to(routes::api_v1::get_device))
.route("/api/v1/devices/{udid}/screenshot", web::get().to(routes::api_v1::get_screenshot))
.route("/api/v1/devices/{udid}/tap", web::post().to(routes::api_v1::tap))
.route("/api/v1/devices/{udid}/swipe", web::post().to(routes::api_v1::swipe))
.route("/api/v1/devices/{udid}/input", web::post().to(routes::api_v1::input))
.route("/api/v1/devices/{udid}/keyevent", web::post().to(routes::api_v1::keyevent))
.route("/api/v1/batch/tap", web::post().to(routes::api_v1::batch_tap))
.route("/api/v1/batch/swipe", web::post().to(routes::api_v1::batch_swipe))
.route("/api/v1/batch/input", web::post().to(routes::api_v1::batch_input))
.route("/api/v1/openapi.json", web::get().to(routes::api_v1::openapi_spec))
```

### References

- [Source: src/routes/control.rs](./routes/control.rs) - Existing control endpoints
- [Source: src/routes/batch_report.rs](./routes/batch_report.rs) - Batch report patterns
- [Source: docs/api-endpoints.md](../../../docs/api-endpoints.md) - Existing API documentation
- [Source: _bmad-output/implementation-artifacts/4-2-synchronized-batch-operations.md](./4-2-synchronized-batch-operations.md) - Previous batch story
- [Source: _bmad-output/planning-artifacts/prd.md](../../../planning-artifacts/prd.md) - PRD requirements FR30-FR34

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

- Initial implementation: Tasks 1-8 to be completed
- Error handling: Standardized error codes from NFR20
- OpenAPI spec: Required by NFR18

### Completion Notes List

### File List

- `src/routes/api_v1.rs` - NEW - API v1 endpoints (list_devices, get_device, get_screenshot, tap, swipe, input, keyevent, batch_*, openapi_spec)
- `src/routes/mod.rs` - UPDATE - Add api_v1 module
- `src/models/api_response.rs` - NEW - Standardized API response types
- `src/models/openapi.rs` - NEW - OpenAPI 3.0 specification model
- `src/main.rs` - UPDATE - Register API v1 routes
- `tests/test_server.rs` - UPDATE - Add API v1 endpoint tests
