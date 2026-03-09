# Story 5.5: JSON-RPC WebSocket Interface

Epic: 5 (External API & CI/CD Integration)
Status: done
Priority: P2

## Story

As an **Automation Engineer**,
I want to send JSON-RPC commands over WebSocket,
so that I have a standardized protocol for real-time device automation without polling REST endpoints.

## Context & Dependencies

### Related Requirements
- **FR34**: JSON-RPC commands over WebSocket for standardized automation protocol
- **FR30**: REST API for device operations (implemented — this is the WebSocket equivalent)
- **FR31**: WebSocket screenshot streaming (implemented — this story extends the WebSocket surface)

### Dependencies
- **Story 5-1**: REST API Device Operations (done) — all device operation handlers exist
- **Story 5-2**: WebSocket Screenshot Streaming API (done) — JSON-RPC 2.0 structs and WebSocket patterns established
- **Story 5-3**: Device Status and Health API (done) — MetricsTracker for connection tracking

### Architecture Constraints
From `_bmad-output/project-context.md`:
- actix-web 4.x with actix-ws 0.3 for WebSocket handling
- All shared state via `Arc<...>` or `Clone`-able types
- Error handling: `Result<T, String>` in service layer
- Logging: `tracing::info!("[PREFIX]")` with context tags
- No new dependencies required — all needed crates already in Cargo.toml

## Acceptance Criteria

### AC1: Execute JSON-RPC tap command
```gherkin
Scenario: Execute JSON-RPC tap command
  Given a WebSocket connection to /api/v1/ws/nio is open
  When I send {"jsonrpc":"2.0","method":"tap","params":{"udid":"abc123","x":100,"y":200},"id":1}
  Then a tap executes at (100, 200) on device abc123
  And response {"jsonrpc":"2.0","result":"ok","id":1} is returned
```

### AC2: Batch operations via JSON-RPC
```gherkin
Scenario: Batch operations via JSON-RPC
  Given a WebSocket connection is open
  When I send {"jsonrpc":"2.0","method":"batchTap","params":{"udids":["a","b","c"],"x":100,"y":200},"id":3}
  Then the tap executes on all specified devices
  And the result includes per-device status
```

### AC3: Handle JSON-RPC errors
```gherkin
Scenario: Handle JSON-RPC errors
  Given a WebSocket connection is open
  When I send {"jsonrpc":"2.0","method":"invalidMethod","id":4}
  Then response {"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":4} is returned
```

## Tasks / Subtasks

- [x] Task 1: Create JSON-RPC NIO WebSocket handler (AC: 1, 2, 3)
  - [x] Create `/api/v1/ws/nio` WebSocket endpoint handler in `src/routes/api_v1.rs`
  - [x] Reuse and extend existing `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError` structs (kept in api_v1.rs)
  - [x] Implement WebSocket upgrade with actix-ws pattern matching existing `ws_screenshot` handler
  - [x] Validate `jsonrpc: "2.0"` on all requests (error code -32600 if invalid)
  - [x] Return -32601 for unknown methods

- [x] Task 2: Implement single-device JSON-RPC methods (AC: 1)
  - [x] `tap` — params: `{udid, x, y}` → execute tap, return `"ok"`
  - [x] `swipe` — params: `{udid, x1, y1, x2, y2, duration?}` → execute swipe, return `"ok"`
  - [x] `input` — params: `{udid, text}` → send text input, return `"ok"` (clear omitted — no ATX client method)
  - [x] `keyevent` — params: `{udid, key}` → send key event, return `"ok"`
  - [x] Validate required params, return -32602 on missing/invalid params
  - [x] Handle device not found with appropriate JSON-RPC error (-1)
  - [x] Handle operation failures with appropriate JSON-RPC error (-32603)

- [x] Task 3: Implement batch JSON-RPC methods (AC: 2)
  - [x] `batchTap` — params: `{udids: [...], x, y}` → parallel tap, return per-device results
  - [x] `batchSwipe` — params: `{udids: [...], x1, y1, x2, y2, duration?}` → parallel swipe
  - [x] `batchInput` — params: `{udids: [...], text}` → parallel input
  - [x] Return result object with `total`, `succeeded`, `failed`, `results` array matching REST batch format
  - [x] Enforce MAX_BATCH_SIZE (20 devices per batch)

- [x] Task 4: Implement utility JSON-RPC methods (AC: 1)
  - [x] `listDevices` — no params → return connected device list
  - [x] `getDevice` — params: `{udid}` → return device info
  - [x] `screenshot` — params: `{udid, quality?, scale?}` → return base64 screenshot data
  - [x] `getStatus` — no params → return device farm status summary

- [x] Task 5: Register route and update OpenAPI spec (AC: 1, 2, 3)
  - [x] Register `/api/v1/ws/nio` route in `src/main.rs`
  - [x] Add `/api/v1/ws/nio` path to OpenAPI spec in `src/models/openapi.rs` with JSON-RPC method descriptions
  - [x] Update `docs/ci-cd-integration.md` WebSocket section to include the NIO endpoint

- [x] Task 6: Write tests (AC: 1, 2, 3)
  - [x] Add integration test for WebSocket upgrade at `/api/v1/ws/nio` (verify error on non-WS request)
  - [x] Add unit tests for JSON-RPC request parsing and validation
  - [x] Add unit tests for method dispatch (known methods via OpenAPI spec coverage)
  - [x] Add unit test for batch result formatting

## Dev Notes

### Existing JSON-RPC 2.0 Foundation (Story 5-2)

The WebSocket screenshot streaming handler at `/api/v1/ws/screenshot/{udid}` already implements JSON-RPC 2.0 with these patterns:

```rust
// Already defined in src/routes/api_v1.rs (lines 737-769):
#[derive(Debug, serde::Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
    id: u64,
}

#[derive(Debug, serde::Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: u64,
}

#[derive(Debug, serde::Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}
```

**IMPORTANT**: These structs are currently **private** to `api_v1.rs`. For story 5-5, either:
1. Keep them in `api_v1.rs` since the new handler is also in that file (simplest), OR
2. Extract to `models/api_response.rs` if they'll be needed elsewhere

Recommendation: Keep in `api_v1.rs` since both WebSocket handlers live there.

### Key Difference from Screenshot WebSocket

The screenshot WebSocket (`/api/v1/ws/screenshot/{udid}`) is **device-specific** — the UDID is in the URL path, and the connection streams binary frames for that one device.

The NIO WebSocket (`/api/v1/ws/nio`) is **device-agnostic** — the UDID is passed in JSON-RPC params. One connection can control any device. No binary frames — only JSON-RPC text messages.

### Existing NIO WebSocket (Legacy)

There's an existing NIO handler at `src/routes/nio.rs` that uses a custom `{type, data, id}` format (NOT JSON-RPC). The new `/api/v1/ws/nio` endpoint is the standardized replacement. Do NOT modify the legacy `nio.rs` — create the new handler in `api_v1.rs`.

### Device Client Resolution Pattern

The REST API handlers use this helper to get a device client:

```rust
// In api_v1.rs (line 29):
async fn get_device_client(
    state: &AppState,
    udid: &str,
) -> Result<(serde_json::Value, Arc<AtxClient>), HttpResponse>
```

For the WebSocket handler, you need a version that returns a JSON-RPC error instead of HttpResponse. Create a helper like:

```rust
async fn get_device_client_jsonrpc(
    state: &AppState,
    udid: &str,
    request_id: u64,
) -> Result<(Value, Arc<AtxClient>), String>
// Returns serialized JsonRpcError string on failure
```

Or better: reuse `get_device_client` logic but translate errors to JSON-RPC format inline.

### JSON-RPC Error Codes (Standard)

| Code | Meaning | When |
|------|---------|------|
| -32600 | Invalid Request | `jsonrpc` field is not `"2.0"` |
| -32601 | Method not found | Unknown method name |
| -32602 | Invalid params | Missing required params or wrong types |
| -32603 | Internal error | Device operation failed |

Custom application error codes (negative, outside -32600 to -32699 range):
| Code | Meaning | When |
|------|---------|------|
| -1 | Device not found | UDID not in system |
| -2 | Device disconnected | Device exists but connection lost |

### Batch Result Format

Match the REST API batch response format for consistency:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "total": 3,
    "succeeded": 2,
    "failed": 1,
    "results": [
      {"udid": "a", "status": "success"},
      {"udid": "b", "status": "success"},
      {"udid": "c", "status": "error", "error": "Device not found"}
    ]
  },
  "id": 3
}
```

**Note**: The `result` field in `JsonRpcResponse` is currently `Option<String>`. For batch results and device info responses, you'll need to change it to `Option<serde_json::Value>` to support structured responses. This is a necessary change.

### WebSocket Handler Pattern

Follow the established `ws_screenshot` pattern:

```rust
pub async fn ws_nio(
    state: web::Data<AppState>,
    req: HttpRequest,
    stream: web::Payload,
) -> HttpResponse {
    // No UDID in path — device-agnostic endpoint
    let (resp, mut session, mut msg_stream) = match actix_ws::handle(&req, stream) {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().json(json!({...})),
    };

    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                actix_ws::Message::Text(text) => {
                    // Parse JSON-RPC, dispatch, respond
                }
                actix_ws::Message::Close(_) => break,
                actix_ws::Message::Ping(bytes) => { let _ = session.pong(&bytes).await; }
                _ => {}
            }
        }
    });

    resp
}
```

### MetricsTracker Integration

Increment `websocket_count` on connect, decrement on disconnect:

```rust
state.metrics.websocket_count.fetch_add(1, Ordering::Relaxed);
// ... on disconnect:
state.metrics.websocket_count.fetch_sub(1, Ordering::Relaxed);
```

### REST Handler Reuse Strategy

The device operation logic already exists in the REST handlers. However, the REST handlers return `HttpResponse` directly. For JSON-RPC, extract the core logic or call the ATX client directly:

```rust
// Direct ATX client call pattern (preferred for WebSocket):
let client = AtxClient::new(&ip, port, &udid);
client.tap(x, y).await.map_err(|e| format!("Tap failed: {}", e))?;
```

The ATX client methods:
- `client.tap(x, y)` → `Result<(), String>`
- `client.swipe(x1, y1, x2, y2, duration)` → `Result<(), String>`
- `client.input_text(text)` → `Result<(), String>`
- `client.clear_text()` → `Result<(), String>`
- `client.keyevent(key)` → `Result<(), String>`
- `client.screenshot_scaled(scale, quality)` → `Result<Vec<u8>, String>`

### Project Structure Notes

Files to create/modify:
```
src/routes/api_v1.rs        — MODIFY: Add ws_nio handler, extend JsonRpcResponse.result to Value
src/models/openapi.rs       — MODIFY: Add /api/v1/ws/nio path
src/main.rs                 — MODIFY: Register /api/v1/ws/nio route
tests/test_server.rs        — MODIFY: Add ws_nio route to setup_test_app!, add tests
docs/ci-cd-integration.md   — MODIFY: Add NIO WebSocket docs
```

No new files needed — this extends the existing API v1 module.

### References

- [Source: src/routes/api_v1.rs:737-769] — JSON-RPC 2.0 structs
- [Source: src/routes/api_v1.rs:781-1144] — WebSocket screenshot handler (pattern to follow)
- [Source: src/routes/nio.rs] — Legacy NIO handler (do NOT modify, reference only)
- [Source: src/models/api_response.rs] — Request/response types (TapRequest, SwipeRequest, etc.)
- [Source: src/device/atx_client.rs] — ATX client methods (tap, swipe, input, keyevent, screenshot)
- [Source: src/state.rs] — AppState with MetricsTracker, connection_pool, device_info_cache
- [Source: src/main.rs:259-279] — API v1 route registration
- [Source: _bmad-output/planning-artifacts/epics-stories.md:1225-1249] — Story acceptance criteria
- [Source: _bmad-output/planning-artifacts/prd.md:202-233] — API specification, WebSocket endpoints
- [Source: _bmad-output/project-context.md] — Coding standards, testing rules

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

- `error_response` function name conflict with existing HTTP helper — renamed JSON-RPC versions to `rpc_ok`, `rpc_result`, `rpc_error`
- `DeviceService::new()` doesn't exist — it's a unit struct; replaced with `PhoneService::new(state.db.clone())` + `query_device_list_by_present()`
- `clear_text()` method doesn't exist on AtxClient — removed `clear` param from `input`/`batchInput` methods
- `execute_single_tap`/`execute_single_swipe` take `u32` coordinates, not `i32` — fixed batch methods
- `execute_single_input` returns `Result<(), (String, String)>` not `Result<(), String>` — adapted `build_batch_result`
- `swipe()` duration is `f64` not `i32` — fixed
- `screenshot_scaled()` scale is `f64` not `f32` — fixed
- Used `futures::future::join_all` instead of `tokio::spawn` for batch operations (avoids `'static` lifetime issues, matches REST batch handler pattern)
- OpenAPI `Operation` struct: `description` is `Option<String>`, `parameters` is `Option<Vec<>>`, `responses` is `Option<HashMap<>>`, no `tags` field

### Completion Notes List

- Changed `JsonRpcResponse.result` from `Option<String>` to `Option<serde_json::Value>` to support structured responses (batch results, device info, screenshots)
- 11 JSON-RPC methods implemented: tap, swipe, input, keyevent, batchTap, batchSwipe, batchInput, listDevices, getDevice, screenshot, getStatus
- WebSocket metrics tracking via `MetricsTracker.websocket_count` (increment on connect, decrement on disconnect)
- `clear` parameter omitted from input/batchInput since ATX client has no clear_text method
- `screenshot` returns base64-encoded JPEG with format/quality/scale metadata
- Batch operations reuse existing `execute_single_tap`, `execute_single_swipe`, `execute_single_input` helpers
- 5 new tests pass, 147 total pass, 4 pre-existing batch_report failures unchanged

### Code Review Fixes Applied

- **H1** (nio_get_client): Added device info cache check matching `get_device_client` pattern — avoids DB query on every JSON-RPC command for cached devices
- **M1** (coordinate types): Batch methods now parse as `i32` then cast to `u32` (`v as i32 as u32`) — consistent truncation behavior between single and batch operations
- **M2** (test_ws_nio_json_rpc_request_validation): Expanded test to validate param structure, missing fields, and batch request format
- **M3** (test_ws_nio_batch_result_format_matches_rest_api): Rewrote test to call actual REST `/api/batch/tap` and verify field structure matches JSON-RPC batch format
- **L1** (nio_get_status): Fixed status counting — `query_device_list_by_present()` returns only present devices, so all are connected; removed broken `"online"` filter
- **L2** (DeviceService import): False alarm — `DeviceService` used at line 875 for USB screenshot fallback
- **L3** (docs): Added JSON-RPC error code reference table to `docs/ci-cd-integration.md`

### File List

- `src/routes/api_v1.rs` — MODIFIED: Changed JsonRpcResponse.result to Value, added ws_nio handler + all 11 JSON-RPC methods (~500 lines added)
- `src/main.rs` — MODIFIED: Registered `/api/v1/ws/nio` route
- `src/models/openapi.rs` — MODIFIED: Added `/api/v1/ws/nio` path to OpenAPI spec
- `docs/ci-cd-integration.md` — MODIFIED: Added NIO WebSocket section with JSON-RPC method and error code reference
- `tests/test_server.rs` — MODIFIED: Added ws/nio route to test setup, added `/api/v1/ws/nio` to OpenAPI completeness test, added 4 new JSON-RPC tests
