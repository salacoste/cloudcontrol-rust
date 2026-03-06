# Story 3.1: Tap Command Execution

Status: ready-for-dev

## Story

As a **QA Engineer**, I want to execute tap commands on connected devices, so that I can interact with apps and UI elements during testing.

## Acceptance Criteria

1. **Execute tap at coordinates**
   - Given a device is connected with screen visible
   - When I send POST /api/tap with x=540, y=1200
   - Then a tap is executed at coordinates (540, 1200)
   - And the response confirms tap execution
   - And response time is under 100ms

2. **Validate coordinates within screen bounds**
   - Given a device has resolution 1080x2400
   - When I send tap with x=2000, y=3000 (out of bounds)
   - Then HTTP 400 is returned
   - And error code is "ERR_INVALID_REQUEST"
   - And error message indicates coordinates are out of bounds

3. **Handle device not found**
   - Given device UDID "nonexistent" does not exist
   - When I send POST /api/tap with udid="nonexistent"
   - Then HTTP 404 is returned
   - And error code is "ERR_DEVICE_NOT_FOUND"

4. **Handle disconnected device**
   - Given device was connected but is now unreachable
   - When I send POST /api/tap for that device
   - Then HTTP 503 is returned
   - And error code is "ERR_DEVICE_DISCONNECTED"

## Tasks / Subtasks

- [ ] Task 1: Add coordinate bounds validation (AC: 2)
  - [ ] Fetch device display info (width, height) before tap execution
  - [ ] Validate x coordinate is within [0, display_width)
  - [ ] Validate y coordinate is within [0, display_height)
  - [ ] Return HTTP 400 with ERR_INVALID_REQUEST for out-of-bounds coordinates

- [ ] Task 2: Improve error handling for device states (AC: 3, 4)
  - [ ] Return HTTP 404 for device not found (currently returns 500)
  - [ ] Return HTTP 503 for disconnected/unreachable devices
  - [ ] Include appropriate error codes in response

- [ ] Task 3: Add E2E tests
  - [ ] Test successful tap execution with valid coordinates
  - [ ] Test tap with x coordinate out of bounds
  - [ ] Test tap with y coordinate out of bounds
  - [ ] Test tap with both coordinates out of bounds
  - [ ] Test tap with negative coordinates
  - [ ] Test tap on nonexistent device
  - [ ] Test tap on disconnected device

## Dev Notes

### Existing Implementation

The tap functionality is **already implemented** in `src/routes/control.rs` at lines 659-720 (`inspector_touch` function). The existing implementation:

- Accepts POST `/api/tap` with JSON body containing `udid`, `x`, `y`
- Uses `atx_client.click(x, y)` to execute the tap
- Returns HTTP 200 on success with confirmation

**What's Missing**:
1. Coordinate bounds validation against device screen dimensions
2. Proper HTTP status codes for device not found (404) vs disconnected (503)
3. Standardized error codes in responses

### Architecture Constraints

- Use existing `PhoneService` and `DeviceService` patterns
- Leverage existing `atx_client.click()` method
- Follow existing error handling patterns with `Result<T, String>`
- Return standardized error codes: `ERR_INVALID_REQUEST`, `ERR_DEVICE_NOT_FOUND`, `ERR_DEVICE_DISCONNECTED`

### Existing Patterns to Follow

```rust
// Handler signature pattern (from project-context.md)
pub async fn handler_name(
    phone_service: Data<Mutex<PhoneService>>,
    body: Json<RequestType>,
) -> HttpResponse

// Error response pattern
HttpResponse::BadRequest().json(json!({
    "status": "error",
    "error": "ERR_INVALID_REQUEST",
    "message": "Coordinates out of bounds"
}))

// Device lookup pattern
let mut service = phone_service.lock().await;
let device = service.query_info_by_udid(&udid).await?;

// Display info access
let display_info = device.display_info.as_ref();
let width = display_info.width;
let height = display_info.height;
```

### API Design

```
POST /api/tap
Content-Type: application/json

{
  "udid": "device_udid_here",
  "x": 540,
  "y": 1200
}

Response (200 OK):
{
  "status": "success",
  "message": "Tap executed at (540, 1200)"
}

Response (400 Bad Request - out of bounds):
{
  "status": "error",
  "error": "ERR_INVALID_REQUEST",
  "message": "Coordinates (2000, 3000) out of bounds. Device resolution: 1080x2400"
}

Response (404 Not Found):
{
  "status": "error",
  "error": "ERR_DEVICE_NOT_FOUND",
  "message": "Device not found: nonexistent_udid"
}

Response (503 Service Unavailable):
{
  "status": "error",
  "error": "ERR_DEVICE_DISCONNECTED",
  "message": "Device is disconnected or unreachable"
}
```

### Performance Requirements

- NFR4: Command latency <100ms
- Response time under 100ms for tap execution

### Project Structure Notes

- Route handler: `src/routes/control.rs` (modify existing `inspector_touch`)
- Tests: `tests/test_server.rs`
- Device info: `PhoneService::query_info_by_udid()` returns `DeviceInfo` with `display_info`

### References

- [Source: src/routes/control.rs:659-720] - Existing `inspector_touch` implementation
- [Source: src/device/atx_client.rs] - `click()` method for tap execution
- [Source: src/services/phone_service.rs] - `query_info_by_udid()` for device lookup
- [Source: _bmad-output/planning-artifacts/epics-stories.md:547-578] - Story definition
- [Source: _bmad-output/project-context.md] - Project rules and patterns

## Dev Agent Record

### Agent Model Used

(To be filled during implementation)

### Debug Log References

(To be filled during implementation)

### Completion Notes List

(To be filled during implementation)

### File List

(To be filled during implementation)
