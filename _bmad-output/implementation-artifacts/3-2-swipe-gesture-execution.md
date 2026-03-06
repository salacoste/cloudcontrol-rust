# Story 3.2: Swipe Gesture Execution

Status: done

## Story

As a **QA Engineer**, I want to send swipe gestures with direction and duration, so that I can scroll and navigate through apps.

## Acceptance Criteria

1. **Execute swipe gesture**
   - Given a device is connected
   - When I send POST /inspector/{udid}/touch with action="swipe", x=100, y=500, x2=100, y2=200, duration=300
   - Then a swipe gesture is executed from (100,500) to (100,200)
   - And the gesture takes approximately 300ms
   - And the response confirms swipe execution
   - And response time is under 100ms

2. **Common swipe patterns**
   - Given I need to scroll quickly
   - When I send POST /inspector/{udid}/touch with pattern="scroll_up"
   - Then a swipe from bottom-center to top-center executes
   - And the duration is 200ms (optimized for scrolling)

3. **Swipe for navigation**
   - Given I need to go back via gesture
   - When I send POST /inspector/{udid}/touch with pattern="back"
   - Then a swipe from left edge to right executes
   - And the Android back gesture is triggered

4. **Invalid swipe parameters**
   - Given I send a swipe request
   - When duration is negative or zero
   - Then HTTP 400 is returned
   - And error code is "ERR_INVALID_REQUEST"

5. **Validate swipe coordinates within bounds**
   - Given a device has resolution 1080x1920
   - When I send swipe with x2=2000, y2=3000 (out of bounds)
   - Then HTTP 400 is returned
   - And error code is "ERR_INVALID_REQUEST"
   - And error message indicates coordinates are out of bounds

6. **Handle device not found**
   - Given device UDID "nonexistent" does not exist
   - When I send POST /inspector/{udid}/touch with swipe action
   - Then HTTP 404 is returned
   - And error code is "ERR_DEVICE_NOT_FOUND"

## Tasks / Subtasks

- [x] Task 1: Add predefined swipe patterns (AC: 2, 3)
  - [x] Add `pattern` parameter to touch request
  - [x] Implement "scroll_up" pattern: bottom-center to top-center
  - [x] Implement "scroll_down" pattern: top-center to bottom-center
  - [x] Implement "back" pattern: left edge swipe right
  - [x] Implement "forward" pattern: right edge swipe left
  - [x] Calculate coordinates based on device display dimensions

- [x] Task 2: Add swipe parameter validation (AC: 4, 5)
  - [x] Validate duration is positive (> 0)
  - [x] Validate x2 coordinate is within [0, display_width)
  - [x] Validate y2 coordinate is within [0, display_height)
  - [x] Return HTTP 400 with ERR_INVALID_REQUEST for invalid parameters

- [x] Task 3: Add E2E tests
  - [x] Test successful swipe with valid coordinates
  - [x] Test swipe with pattern="scroll_up"
  - [x] Test swipe with pattern="scroll_down"
  - [x] Test swipe with pattern="back"
  - [x] Test swipe with pattern="forward"
  - [x] Test swipe with negative duration
  - [x] Test swipe with zero duration
  - [x] Test swipe with x2 out of bounds
  - [x] Test swipe with y2 out of bounds
  - [x] Test swipe on nonexistent device
  - [x] Test invalid pattern name

## Dev Notes

### Existing Implementation

Swipe functionality is **already implemented** in `src/routes/control.rs` in the `inspector_touch` function (lines 677-769). The existing implementation:

- Accepts POST `/inspector/{udid}/touch` with JSON body
- Uses `action` parameter to distinguish "click" vs "swipe"
- Swipe parameters: `x`, `y` (start), `x2`, `y2` (end), `duration` (ms)
- Uses `atx_client.swipe(x, y, x2, y2, duration_seconds)` for execution
- Has ADB fallback via `Adb::input_swipe()`
- Already has coordinate bounds validation for x, y (from Story 3-1)

**What's Missing**:
1. Predefined swipe patterns (scroll_up, scroll_down, back, forward)
2. Duration validation (must be > 0)
3. x2, y2 coordinate bounds validation
4. E2E tests for swipe-specific scenarios

### Architecture Constraints

- Use existing `inspector_touch` handler - extend, don't create new endpoint
- Pattern parameter should calculate coordinates based on device display dimensions
- Follow existing error handling patterns with `ERR_INVALID_REQUEST`
- Maintain fire-and-forget pattern for <100ms response time
- Reuse `get_device_client` for device lookup (already handles 404/503)

### Existing Patterns to Follow

```rust
// Existing swipe handling (control.rs:754-756)
let result = if action == "swipe" {
    client.swipe(x, y, x2, y2, duration.max(0.05).min(2.0)).await
} else {
    client.click(x, y).await
};

// Coordinate validation pattern (from Story 3-1)
let display = device.get("display").cloned().unwrap_or(json!({"width":1080,"height":1920}));
let display_width = display.get("width").and_then(|v| v.as_i64()).unwrap_or(1080) as i32;
let display_height = display.get("height").and_then(|v| v.as_i64()).unwrap_or(1920) as i32;

if x < 0 || x >= display_width {
    return HttpResponse::BadRequest().json(json!({
        "status": "error",
        "error": "ERR_INVALID_REQUEST",
        "message": format!("X coordinate {} out of bounds. Device resolution: {}x{}", x, display_width, display_height)
    }));
}
```

### API Design

```
POST /inspector/{udid}/touch
Content-Type: application/json

// Direct swipe
{
  "action": "swipe",
  "x": 100,
  "y": 500,
  "x2": 100,
  "y2": 200,
  "duration": 300
}

// Pattern-based swipe
{
  "action": "swipe",
  "pattern": "scroll_up"
}

Response (200 OK):
{
  "status": "ok"
}

Response (400 Bad Request - invalid duration):
{
  "status": "error",
  "error": "ERR_INVALID_REQUEST",
  "message": "Duration must be positive"
}

Response (400 Bad Request - invalid pattern):
{
  "status": "error",
  "error": "ERR_INVALID_REQUEST",
  "message": "Unknown swipe pattern: invalid_pattern. Available: scroll_up, scroll_down, back, forward"
}
```

### Predefined Swipe Patterns

| Pattern | Description | Start | End | Duration |
|---------|-------------|-------|-----|----------|
| scroll_up | Scroll content up | (w/2, h*0.8) | (w/2, h*0.2) | 200ms |
| scroll_down | Scroll content down | (w/2, h*0.2) | (w/2, h*0.8) | 200ms |
| back | Android back gesture | (0, h/2) | (w*0.3, h/2) | 250ms |
| forward | Android forward gesture | (w-1, h/2) | (w*0.7, h/2) | 250ms |

Where w = display_width, h = display_height

### Performance Requirements

- NFR4: Command latency <100ms
- Response time under 100ms for swipe execution
- Use fire-and-forget pattern (spawn async task)

### Project Structure Notes

- Route handler: `src/routes/control.rs` (modify existing `inspector_touch`)
- Tests: `tests/test_server.rs`
- Device info: Already available via `get_device_client`

### References

- [Source: src/routes/control.rs:677-769] - Existing `inspector_touch` implementation with swipe
- [Source: src/routes/control.rs:754-756] - Swipe execution via `client.swipe()`
- [Source: src/device/atx_client.rs] - `swipe()` method
- [Source: src/device/adb.rs] - `input_swipe()` ADB fallback
- [Source: _bmad-output/planning-artifacts/epics-stories.md:637-670] - Story definition
- [Source: _bmad-output/implementation-artifacts/3-1-tap-command-execution.md] - Previous story (coordinate validation patterns)
- [Source: _bmad-output/project-context.md] - Project rules and patterns

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

- Initial implementation completed with predefined swipe patterns and validation
- Fixed forward pattern to use `width - 1` instead of `width` to avoid bounds check failure

### Completion Notes List

- AC1 (Execute swipe gesture): ✅ Already implemented, verified with test
- AC2 (Common swipe patterns - scroll_up): ✅ Implemented via `resolve_swipe_pattern` function
- AC3 (Swipe for navigation - back/forward): ✅ Implemented with left/right edge swipes
- AC4 (Invalid swipe parameters): ✅ Returns HTTP 400 with ERR_INVALID_REQUEST for negative/zero duration
- AC5 (Validate swipe coordinates within bounds): ✅ Validates x2, y2 coordinates against display dimensions
- AC6 (Handle device not found): ✅ Returns HTTP 404 with ERR_DEVICE_NOT_FOUND
- All 11 E2E tests passing
- Fire-and-forget pattern maintained for <100ms response time

### File List

- `src/routes/control.rs` - Added `resolve_swipe_pattern` helper function, modified `inspector_touch` to support pattern parameter and swipe-specific validation (x2, y2 bounds, duration > 0)
- `tests/test_server.rs` - Added 11 E2E tests for swipe functionality (test_swipe_success_mock_device, test_swipe_pattern_scroll_up, test_swipe_pattern_scroll_down, test_swipe_pattern_back, test_swipe_pattern_forward, test_swipe_negative_duration, test_swipe_zero_duration, test_swipe_x2_out_of_bounds, test_swipe_y2_out_of_bounds, test_swipe_nonexistent_device, test_swipe_invalid_pattern)
