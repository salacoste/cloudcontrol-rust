# Story 3.4: Physical Key Events

Status: done

## Story

As a **QA Engineer**, I want to send physical key events like home and back, so that I can navigate the device without touch gestures.

## Acceptance Criteria

1. **Send HOME key**
   - Given a device is on any screen
   - When I send POST /inspector/{udid}/keyevent with key="home"
   - Then the device returns to home screen
   - And response time is under 100ms

2. **Send BACK key**
   - Given an app is open
   - When I send POST /inspector/{udid}/keyevent with key="back"
   - Then the device navigates back
   - And the previous screen appears

3. **Send VOLUME keys**
   - Given a device is connected
   - When I send POST /inspector/{udid}/keyevent with key="volume_up"
   - Then the volume increases by one step
   - And when I send key="volume_down"
   - Then the volume decreases by one step

4. **Send POWER key**
   - Given a device screen is on
   - When I send POST /inspector/{udid}/keyevent with key="power"
   - Then the screen turns off
   - And when I send power again
   - Then the screen turns on

5. **Invalid key action**
   - Given I send POST /inspector/{udid}/keyevent with key="invalid_key"
   - Then HTTP 400 is returned
   - And supported keys are listed in the error message

## Tasks / Subtasks

- [x] Task 1: Add validation for key actions (AC: 5)
  - [x] Define list of supported keys: home, back, menu, power, wakeup, volume_up, volume_down
  - [x] Return HTTP 400 with ERR_INVALID_REQUEST for invalid keys
  - [x] Include list of supported keys in error message

- [x] Task 2: Ensure all key mappings are correct (AC: 1, 2, 3, 4)
  - [x] Verify HOME key maps to Android KEYCODE_HOME
  - [x] Verify BACK key maps to Android KEYCODE_BACK
  - [x] Verify volume_up/volume_down map correctly
  - [x] Verify power key toggles screen state

- [x] Task 3: Add E2E tests
  - [x] Test HOME key execution
  - [x] Test BACK key execution
  - [x] Test volume keys execution
  - [x] Test power key execution
  - [x] Test invalid key returns 400 with error message
  - [x] Test keyevent on nonexistent device returns 404

## Dev Notes

### Existing Implementation

Physical key event functionality is **already implemented** in `src/routes/control.rs` in the `inspector_keyevent` function (lines 958-1020). The existing implementation:

- Accepts POST `/inspector/{udid}/keyevent` with JSON body `{ "key": "<key_name>" }`
- Has key mapping for common keys (Enter, Backspace, Delete, Home, Back, Tab, Escape, Arrow keys, Menu, Power, WakeUp)
- Uses fire-and-forget pattern via `tokio::spawn`
- Returns HTTP 200 immediately with success response
- Has ADB fallback via `Adb::input_keyevent()`

**What's Missing**:
1. **Validation for invalid key actions** - Currently accepts any key string
2. **Error response for invalid keys** - Should return HTTP 400 with list of supported keys
3. **E2E tests** - No tests for keyevent endpoint

### Architecture Constraints

- Use existing `inspector_keyevent` handler - extend, don't create new endpoint
- Follow existing error handling patterns with `ERR_INVALID_REQUEST`, `ERR_DEVICE_NOT_FOUND`
- Maintain fire-and-forget pattern for <100ms response time
- Reuse `get_device_client` for device lookup

### Key Mapping Reference

The existing code maps keys to Android keyevent names:

```rust
// From src/routes/control.rs:988-1005
let android_key = match key.as_str() {
    "Enter" => "enter",
    "Backspace" | "DEL" => "del",
    "Delete" => "forward_del",
    "Home" | "HOME" | "home" => "home",
    "Back" | "BACK" | "back" => "back",
    "Tab" => "tab",
    "Escape" => "back",
    "ArrowUp" => "dpad_up",
    "ArrowDown" => "dpad_down",
    "ArrowLeft" => "dpad_left",
    "ArrowRight" => "dpad_right",
    "Menu" | "MENU" | "menu" => "menu",
    "Power" | "POWER" | "power" => "power",
    "WAKEUP" | "wakeup" => "wakeup",
    other => other,  // Pass through unknown keys
}
```

**Note**: volume_up and volume_down are NOT currently mapped - they need to be added.

### Required Key Additions

Add to the key mapping:
- `"volume_up" | "VOLUME_UP" => "volume_up"`
- `"volume_down" | "VOLUME_DOWN" => "volume_down"`

### API Design

```
POST /inspector/{udid}/keyevent
Request Body:
{
  "key": "home"  // or "back", "menu", "power", "volume_up", "volume_down"
}

Response (200 OK):
{
  "status": "ok"
}

Response (400 Bad Request for invalid key):
{
  "status": "error",
  "error": "ERR_INVALID_REQUEST",
  "message": "Invalid key action: invalid_key. Supported keys: home, back, menu, power, wakeup, volume_up, volume_down, enter, tab, del, forward_del, dpad_up, dpad_down, dpad_left, dpad_right"
}
```

### Project Structure Notes

- Route handlers: `src/routes/control.rs` - modify `inspector_keyevent`
- ATxClient: `src/device/atx_client.rs` - `press_key()` method
- ADB: `src/device/adb.rs` - `input_keyevent()` for fallback
- Tests: `tests/test_server.rs` - add E2E tests for keyevent

### Performance Requirements

- NFR3: API response time <100ms
- Key events should be near-instantaneous

### Previous Story Learnings (3-3 Text Input)

1. **Fire-and-forget pattern**: Use `tokio::spawn` for async operations to maintain <100ms response
2. **ADB fallback**: Always implement ADB fallback for when ATX fails
3. **Error handling**: Return proper HTTP status codes (400 for bad request, 404 for device not found)
4. **Mock device handling**: Check `is_mock` flag and return success immediately for mock devices
5. **Input validation**: Validate all input parameters before processing

### References

- [Source: src/routes/control.rs:958-1020] - Existing keyevent implementation
- [Source: src/device/atx_client.rs] - AtxClient press_key method
- [Source: src/device/adb.rs] - ADB input_keyevent fallback
- [Source: _bmad-output/planning-artifacts/epics-stories.md:713-752] - Story definition
- [Source: _bmad-output/implementation-artifacts/3-3-text-input-to-device.md] - Previous story patterns
- [Source: docs/architecture.md] - Architecture constraints

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

None - implementation was straightforward.

### Completion Notes List

1. **Key Validation Added**: Implemented validation for supported keys with HTTP 400 response for invalid keys
2. **Volume Keys Added**: Added `volume_up` and `volume_down` to the key mapping
3. **Case-Insensitive Matching**: Changed key matching to be case-insensitive using `to_lowercase()`
4. **E2E Tests Added**: Added 9 tests covering all acceptance criteria

### File List

- `src/routes/control.rs` - Modified `inspector_keyevent` function:
  - Added `SUPPORTED_KEYS` constant with all valid keys
  - Added volume_up and volume_down to key mapping
  - Added case-insensitive key matching
  - Added validation that returns HTTP 400 for invalid keys

- `tests/test_server.rs` - Added 9 new E2E tests:
  - `test_keyevent_home_key`
  - `test_keyevent_back_key`
  - `test_keyevent_volume_keys`
  - `test_keyevent_power_key`
  - `test_keyevent_menu_key`
  - `test_keyevent_wakeup_key`
  - `test_keyevent_case_insensitive`
  - `test_keyevent_invalid_key_returns_400`
  - `test_keyevent_nonexistent_device_returns_404`
