# Story 12.5: Crash Protection & Error Handling

Status: done

## Story

As a **system administrator**,
I want **the server to never crash from malformed client input**,
so that **the service remains stable under all conditions**.

## Acceptance Criteria

1. **Given** the server is running **When** a malformed binary WebSocket message is received by scrcpy_ws **Then** the connection is closed with an error message, not a panic/server crash
2. **Given** the scrcpy_ws receives touch event data **When** the byte array is too short for `try_into()` conversion **Then** the error is logged and the message is skipped (no panic)
3. **Given** the control.rs endpoints receive requests **When** extracting device UDID **Then** the `X-Device-UDID` header is used (not `Access-Control-Allow-Origin`)
4. **Given** the device_service performs image resizing **When** processing screenshots **Then** `tokio::task::spawn_blocking` is used to avoid blocking the async runtime
5. **Given** any WebSocket handler serializes JSON **When** `serde_json::to_string()` is called **Then** `.unwrap_or_default()` is used instead of `.unwrap()` to prevent panics

## Tasks / Subtasks

- [x] Task 1: Fix scrcpy_ws binary message parsing (AC: #1, #2)
  - [x] 1.1 Create helper function `parse_touch_bytes(data: &[u8]) -> Option<TouchEvent>` that safely parses touch event bytes
  - [x] 1.2 Replace `data[10..14].try_into().unwrap()` with checked conversion returning Option
  - [x] 1.3 Replace all 5 `try_into().unwrap()` calls in touch parsing with safe alternatives
  - [x] 1.4 Create helper function `parse_key_bytes(data: &[u8]) -> Option<KeyEvent>` for key event parsing
  - [x] 1.5 Replace `data[2..6].try_into().unwrap()` with checked conversion
  - [x] 1.6 Log warning and skip message on parse failure (don't close connection for single bad message)
  - [x] 1.7 Add tests for malformed binary message handling

- [x] Task 2: Fix UDID header extraction (AC: #3)
  - [x] 2.1 In `control.rs`, change `get("Access-Control-Allow-Origin")` to `get("X-Device-UDID")` at line ~1965
  - [x] 2.2 In `control.rs`, change `get("Access-Control-Allow-Origin")` to `get("X-Device-UDID")` at line ~2223
  - [x] 2.3 Add comment explaining the header purpose
  - [x] 2.4 Update any frontend code that sets this header (if applicable)

- [x] Task 3: Non-blocking image processing (AC: #4)
  - [x] 3.1 Create `resize_jpeg_blocking(data: &[u8], quality: u8, scale: f64) -> Result<Vec<u8>, String>` function
  - [x] 3.2 Wrap `resize_jpeg` call in `tokio::task::spawn_blocking()` in `screenshot_base64`
  - [x] 3.3 Wrap `resize_jpeg` call in `tokio::task::spawn_blocking()` in `screenshot_jpeg`
  - [x] 3.4 Wrap `resize_jpeg` call in `tokio::task::spawn_blocking()` in `screenshot_usb_base64`
  - [x] 3.5 Wrap `resize_jpeg` call in `tokio::task::spawn_blocking()` in `screenshot_usb_jpeg`
  - [x] 3.6 Add performance tracing to measure blocking time

- [x] Task 4: Safe JSON serialization in WebSocket handlers (AC: #5)
  - [x] 4.1 In `scrcpy_ws.rs`, replace `.unwrap()` on `serde_json::to_string()` with `.unwrap_or_default()`
  - [x] 4.2 In `scrcpy_ws.rs`, replace lines 44, 61, 78, 124 with safe serialization
  - [x] 4.3 Audit other WebSocket handlers for similar patterns
  - [x] 4.4 Add helper macro or function for safe WS JSON serialization

- [x] Task 5: Update tests and regression testing (AC: #1-#5)
  - [x] 5.1 Add unit test for `parse_touch_bytes` with valid and invalid data
  - [x] 5.2 Add unit test for `parse_key_bytes` with valid and invalid data
  - [x] 5.3 Add integration test for malformed scrcpy WebSocket messages
  - [x] 5.4 Verify all existing tests pass
  - [x] 5.5 Run `cargo build` and `cargo test` to verify no regressions

## Dev Notes

### Scope — Crash Protection & Error Handling

This story eliminates panic paths from malformed external input. The server must remain stable even when receiving malicious or corrupted data.

| Decision | Rationale |
|----------|-----------|
| **Graceful degradation for bad messages** | Skip bad messages, log warning, keep connection alive |
| **spawn_blocking for image processing** | Image encoding is CPU-bound, blocks async runtime |
| **X-Device-UDID header** | Semantic correctness - CORS header is wrong place for UDID |
| **unwrap_or_default for JSON** | Empty string is safe fallback for WS messages |

### Key Code Locations

**scrcpy_ws.rs binary parsing (AC #1, #2):**
```rust
// SAFE PATTERN (implemented)
fn parse_touch_bytes(data: &[u8]) -> Option<(u8, u32, u32, u16, u16, u16)> {
    if data.len() < 24 {
        tracing::warn!("[Scrcpy WS] Touch event too short: {} bytes", data.len());
        return None;
    }
    Some((
        data[1],
        u32::from_be_bytes(data[10..14].try_into().ok()?),
        u32::from_be_bytes(data[14..18].try_into().ok()?),
        u16::from_be_bytes(data[18..20].try_into().ok()?),
        u16::from_be_bytes(data[20..22].try_into().ok()?),
        u16::from_be_bytes(data[22..24].try_into().ok()?),
    ))
}
```

**control.rs UDID extraction (AC #3):**
```rust
// CORRECT (implemented)
let udid = req
    .headers()
    .get("X-Device-UDID")  // Semantic correctness
    .and_then(|v| v.to_str().ok())
    .unwrap_or("");
```

**device_service.rs non-blocking image processing (AC #4):**
```rust
// NON-BLOCKING PATTERN (implemented)
pub async fn resize_jpeg_async(
    data: Vec<u8>,
    quality: u8,
    scale: f64,
) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        resize_jpeg_blocking(&data, quality, scale)
    })
    .await
    .map_err(|e| format!("spawn_blocking error: {}", e))?
}
```

**Safe JSON serialization (AC #5):**
```rust
// SAFE (implemented)
fn safe_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| {
        tracing::warn!("[Scrcpy WS] JSON serialization failed: {}", e);
        r#"{"type":"error","message":"internal serialization error"}"#.to_string()
    })
}
```

### What NOT to Implement

- Do NOT add custom error types for this story (belongs in Story 13.2)
- Do NOT add retry logic for failed parses - just skip and log
- Do NOT close the WebSocket connection for a single bad message
- Do NOT add rate limiting for parse errors (already have global rate limiting)
- Do NOT change the binary protocol format

### Files to Modify

| File | Changes |
|------|---------|
| `src/routes/scrcpy_ws.rs` | Safe binary parsing, safe JSON serialization |
| `src/routes/control.rs` | Fix UDID header extraction (2 locations) |
| `src/services/device_service.rs` | Add `spawn_blocking` wrapper for image processing |

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Story 12.5 — AC definition, FR-C5, FR-C6, FR-C9]
- [Source: src/routes/scrcpy_ws.rs — Safe binary parsing with parse_touch_bytes, parse_key_bytes]
- [Source: src/routes/control.rs — X-Device-UDID header extraction]
- [Source: src/services/device_service.rs — spawn_blocking for image processing]
- [Source: NFR4 — No panics from malformed external input]

### Previous Story Learnings (12-4)

From Story 12-4 (Configurable Server Settings):
- All 389 tests pass after changes
- Config validation pattern: bounds checking with helpful error messages
- Test coverage for edge cases is essential
- Use `unwrap_or_default()` for non-critical serialization
- Build succeeded with no warnings

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

None

### Completion Notes List

- All 5 tasks completed: safe binary parsing, UDID header fix, spawn_blocking, safe JSON, tests
- Added `parse_touch_bytes()` and `parse_key_bytes()` helper functions for safe binary parsing
- Added `safe_json()` helper function for panic-free JSON serialization
- Fixed UDID header extraction: `Access-Control-Allow-Origin` → `X-Device-UDID`
- Added `resize_jpeg_async()` using `tokio::task::spawn_blocking()` for non-blocking image processing
- Added 8 new unit tests for safe parsing functions
- All 397 tests pass (389 existing + 8 new)
- Build succeeds with no warnings

### File List

- `src/routes/scrcpy_ws.rs` — Added parse_touch_bytes, parse_key_bytes, safe_json helpers; replaced unsafe unwrap calls; added 8 tests
- `src/routes/control.rs` — Fixed 2 locations using wrong header for UDID extraction
- `src/services/device_service.rs` — Added resize_jpeg_async with spawn_blocking; removed deprecated blocking functions

## Change Log

- 2026-03-11: Initial implementation complete — all AC satisfied, 397 tests pass
- 2026-03-11: Code review fixes — corrected doc comment (28→24 bytes), added tracing to resize_jpeg_async
