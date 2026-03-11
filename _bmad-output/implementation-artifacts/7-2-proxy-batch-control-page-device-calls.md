# Story 7.2: Proxy Batch Control Page Device Calls

Status: done

## Story

As a **device farm operator**,
I want **the batch control page to use server-proxied endpoints**,
so that **multi-device operations work from any browser location**.

## Acceptance Criteria

1. **Given** the batch control page (`device_synchronous.html` / `remote_synchronous.js`) is open **When** screen streaming, touch input, shell commands, or screenshots are triggered **Then** all communication goes through server WebSocket (NIO) or HTTP endpoints
2. **Given** `remote_synchronous.js` is loaded **When** inspecting the code **Then** the `deviceUrl` computed property is removed and all `this.deviceUrl + "/..."` calls are replaced with server proxy endpoints
3. **Given** `device_synchronous.html` is loaded **When** inspecting the code **Then** the `deviceUrl` computed property is removed, `deviceIp`/`devicePort` JS vars are removed, and `device: {ip, port}` data is removed
4. **Given** the page uses screen streaming **When** `openScreenStream()` is called **Then** it connects via NIO WebSocket (`/nio/{udid}/ws`) instead of direct `ws://device_ip:port/minicap`
5. **Given** the page uses touch input **When** `enableTouch()` is called **Then** it uses the server-proxied `/inspector/{udid}/touch` HTTP endpoint instead of direct `ws://device_ip:port/minitouch` WebSocket
6. **Given** the batch keyevent function is called **When** inspecting the code **Then** it uses `/inspector/{udid}/keyevent` per-device instead of the legacy `/devices/shell/input keyevent` URL
7. **Given** `remote_synchronous.js` or `device_synchronous.html` is loaded **When** inspecting the code **Then** no hardcoded IP addresses or port numbers (7912, 6677) remain
8. **Given** existing proxied functionality (device info, screenshot thumbnails, input) **When** the page is used **Then** all existing proxied features continue working without regression

## Tasks / Subtasks

- [x] Task 1: Remove `deviceUrl` and direct device references from `remote_synchronous.js` (AC: #2, #7)
  - [x] 1.1 Remove `deviceUrl` computed property (line 69-71)
  - [x] 1.2 Remove `device: { ip: deviceIp, port: 7912 }` data property (line 8-11)
  - [x] 1.3 Remove `deviceIp` data property (line 5)
  - [x] 1.4 Replace `saveScreenshot()` — `this.deviceUrl + "/screenshot"` → `/inspector/{udid}/screenshot/img` (line 136)
  - [x] 1.5 Replace `fixRotation()` — `this.deviceUrl + "/info/rotation"` → `/inspector/{udid}/rotation` (line 153)
  - [x] 1.6 Replace `shell()` — `this.deviceUrl + "/shell"` → `/inspector/{udid}/shell` (line 184)
- [x] Task 2: Remove `deviceUrl` and direct device references from `device_synchronous.html` (AC: #3, #7)
  - [x] 2.1 Remove `deviceUrl` computed property (line 1448-1450)
  - [x] 2.2 Remove `var deviceIp` and `var devicePort` template vars (line 1368-1369)
  - [x] 2.3 Remove `deviceIp` from Vue data and `device: { ip: deviceIp, port: 7912 }` (line 1379, 1383)
- [x] Task 3: Replace minicap WebSocket with NIO in `remote_synchronous.js` (AC: #1, #4)
  - [x] 3.1 Rewrite `openScreenStream()` to use NIO WebSocket at `/nio/{udid}/ws` instead of `ws://device_ip:port/minicap` (line 383)
  - [x] 3.2 Send NIO `subscribe` message to start screenshot streaming
  - [x] 3.3 Handle NIO screenshot messages (binary JPEG frames via ArrayBuffer) and render to canvas
  - [x] 3.4 Remove old minicap binary protocol handling
- [x] Task 4: Replace minitouch WebSocket with HTTP touch proxy (AC: #1, #5)
  - [x] 4.1 Remove `enableTouch()` minitouch WebSocket loop (line 724-736)
  - [x] 4.2 Replace with mouse event handlers that POST to `/inspector/{udid}/touch` for each device
  - [x] 4.3 Rewrite `mouseDownListener`, `mouseMoveListener`, `mouseUpListener` to use HTTP touch API
  - [x] 4.4 Handle batch touch — send touch to all target devices via `/inspector/{udid}/touch`
  - [x] 4.5 Remove `MiniTouch.createNew(ws)` references and `control_list` WebSocket array
- [x] Task 5: Fix keyevent to use proper proxy (AC: #6)
  - [x] 5.1 Replace `keyevent()` legacy URL `/devices/shell/input keyevent ...` with per-device `/inspector/{udid}/keyevent` calls
  - [x] 5.2 Send keyevent to all target devices (batch behavior)
  - [x] 5.3 Remove the `this.shell("input keyevent " + meta)` fallback call
- [x] Task 6: Fix `window.refersh()` typo and dead code (AC: #1)
  - [x] 6.1 Remove `window.refersh()` call in `mouseUpListener` (line 862) — it's a typo and undefined function
  - [x] 6.2 Remove `inputText` watcher that uses `this.inputWS.send()` (line 111-114) — `inputWS` is never initialized
- [x] Task 7: Remove IP/Port from `control.rs` template context for batch page (AC: #7)
  - [x] 7.1 Check `async_list_get()` and `async_list_page()` — remove `IP`/`Port` from template context if unused by template
- [x] Task 8: Regression testing (AC: #8)
  - [x] 8.1 Build succeeds — 0 new warnings
  - [x] 8.2 All existing tests pass (168/177, 9 pre-existing failures)
  - [x] 8.3 No new regressions introduced

## Dev Notes

### Critical Architecture Constraint

**ALL browser-to-device calls MUST go through the CloudControl server.** The browser may be on a completely different network than the devices. Direct calls to device IPs will fail in production.

### Existing Proxy Infrastructure (DO NOT DUPLICATE)

These endpoints already exist and work — **reuse them**:

| Endpoint | Method | Purpose | File |
|----------|--------|---------|------|
| `/inspector/{udid}/screenshot` | GET | Screenshot as base64 JSON | `control.rs` |
| `/inspector/{udid}/screenshot/img` | GET | Screenshot as JPEG binary | `control.rs` |
| `/inspector/{udid}/touch` | POST | Touch/tap/swipe | `control.rs` |
| `/inspector/{udid}/input` | POST | Text input | `control.rs` |
| `/inspector/{udid}/keyevent` | POST | Key press | `control.rs` |
| `/inspector/{udid}/hierarchy` | GET | UI hierarchy dump | `control.rs` |
| `/inspector/{udid}/upload` | POST | File upload to device | `control.rs` |
| `/inspector/{udid}/rotation` | POST | Fix device rotation | `control.rs` |
| `/inspector/{udid}/shell` | POST/GET | Shell command execution | `control.rs` |
| `/nio/{udid}/ws` | WebSocket | Multiplexed screenshot + control | `nio.rs` |
| `/devices/{udid}/shell` | WebSocket | Interactive shell | `control.rs` |
| `/devices/{udid}/info` | GET | Device info JSON | `control.rs` |

### NIO WebSocket Protocol (for replacing minicap)

The NIO WebSocket at `/nio/{udid}/ws` supports:

**Subscribe to screenshots:**
```json
{"type": "subscribe", "data": {"channel": "screenshot", "interval": 200, "quality": 60, "scale": 0.6}}
```

**Receive screenshots:**
```json
{"type": "screenshot", "data": {"image": "base64_jpeg_data...", "width": 1080, "height": 1920, "timestamp": 1234567890}}
```

**Unsubscribe:**
```json
{"type": "unsubscribe", "data": {"channel": "screenshot"}}
```

**Touch via NIO (alternative to HTTP):**
```json
{"type": "touch", "data": {"x": 500, "y": 1000, "action": "tap"}}
```

### Touch Proxy Pattern (for replacing minitouch)

The old code uses minitouch WebSocket protocol (`touchDown`, `touchMove`, `touchUp`, `touchCommit`) which is a low-level binary protocol. Replace with the HTTP touch endpoint:

```javascript
// BEFORE (minitouch WebSocket - direct to device):
var ws = new WebSocket("ws://" + device.ip + ":" + device.port + "/minitouch");
control = MiniTouch.createNew(ws);
control.touchDown(0, xP, yP, pressure);
control.touchCommit();

// AFTER (HTTP proxy - through server):
// For tap (mouseDown + mouseUp at same position):
$.ajax({
  url: "/inspector/" + udid + "/touch",
  method: "POST",
  contentType: "application/json",
  data: JSON.stringify({ x: pixelX, y: pixelY })
});

// For swipe (mouseDown → mouseMove → mouseUp):
$.ajax({
  url: "/inspector/" + udid + "/touch",
  method: "POST",
  contentType: "application/json",
  data: JSON.stringify({ x: startX, y: startY, x2: endX, y2: endY, duration: 200 })
});
```

**Important coordinate conversion:** The old minitouch uses percentage-based coords (0.0-1.0), but the `/inspector/{udid}/touch` endpoint uses **pixel coordinates**. You need to multiply by the device's actual resolution (width/height stored in `deviceList[i].width` and `deviceList[i].height`).

### Keyevent Fix

The current `keyevent()` function does TWO things (both wrong for proxied mode):

```javascript
// Line 176-180 — current code:
keyevent: function (meta) {
    // 1. Sends to legacy batch URL (unclear what this does)
    $.ajax({ url: "/devices/shell/input keyevent " + meta + "?list=" + JSON.stringify(deviceList) });
    // 2. Also calls shell() which goes direct to device
    return this.shell("input keyevent " + meta.toUpperCase());
}
```

Replace with per-device keyevent calls through the proxy:

```javascript
keyevent: function (meta) {
    var targets = this.getTargetDevices();
    for (var i = 0; i < targets.length; i++) {
        $.ajax({
            url: "/inspector/" + targets[i].udid + "/keyevent",
            method: "POST",
            contentType: "application/json",
            data: JSON.stringify({ key: meta.toUpperCase() })
        });
    }
}
```

### Batch Touch Architecture Decision

The old minitouch approach opened N WebSocket connections (one per device) and sent touch events in parallel. For the proxied version, choose ONE of:

1. **HTTP per-device** — Send `/inspector/{udid}/touch` to each target device. Simpler, but sequential latency.
2. **NIO WebSocket per-device** — Open `/nio/{udid}/ws` for each device, send touch events via WebSocket. Lower latency for multi-device.
3. **Batch API** — Use `/api/batch/tap` and `/api/batch/swipe` endpoints for simultaneous multi-device touch.

**Recommendation:** Use option 3 (`/api/batch/tap`, `/api/batch/swipe`) for batch operations. These endpoints already handle concurrent execution server-side. For the main canvas interaction (mouseDown/Move/Up on the primary screen), use option 1 (HTTP per-device) since it maps cleanly to the existing event handlers.

### `window.refersh()` — Dead Code

Line 862: `window.refersh()` is a typo (should be `refresh`). But even `window.refresh()` doesn't exist as a browser API. The original intent was probably to refresh the screenshot display after a touch event, but since NIO streaming provides continuous updates, this call is unnecessary. **Remove it entirely.**

### `inputWS` — Dead Code

Line 111-114: The `inputText` watcher calls `this.inputWS.send()`, but `inputWS` is initialized to `null` and never assigned. This will crash if `inputText` changes. **Remove the watcher or guard with null check.**

### Code Patterns to Follow (established in Story 7.1)

```javascript
// Server proxy URL pattern:
url: "/inspector/" + this.deviceUdid + "/shell"

// NIO WebSocket connection:
var protocol = location.protocol === "https:" ? "wss:" : "ws:";
var ws = new WebSocket(protocol + "//" + location.host + "/nio/" + udid + "/ws");
```

### File List of Changes Expected

- `resources/static/js/remote_synchronous.js` — Main changes: remove deviceUrl, replace all direct calls
- `resources/templates/device_synchronous.html` — Remove deviceUrl, deviceIp, devicePort
- `src/routes/control.rs` — Minor: remove unused IP/Port template vars from batch page handlers

### `device_synchronous.html` Already-Proxied Calls (DO NOT CHANGE)

These were already fixed in a previous session or are already server-relative:
- Line 1500: `/devices/{udid}/info` — already proxied
- Line 1473: `/inspector/{udid}/screenshot/img` — already proxied
- Line 1856: `/inspector/{udid}/input` — already proxied
- Line 2026: `/inspector/{udid}/screenshot/img` — already proxied

### Story 7.1 Code Review Learnings

From the Story 7.1 code review, these issues were found and fixed:
- **Security**: Shell proxy endpoints needed `has_dangerous_metacharacters()` check in addition to `is_dangerous_command()`
- **Dead code**: Unused template context variables (`IP`, `Port`) should be removed from Rust handlers
- **Null guards**: Functions that reference disabled WebSockets need null checks
- **Dead functions**: Remove rather than leave commented-out code calling non-existent endpoints

Apply these same patterns to Story 7.2.

### Project Structure Notes

- Frontend JS: `resources/static/js/remote_synchronous.js`
- Frontend template: `resources/templates/device_synchronous.html`
- Backend routes: `src/routes/control.rs`
- NIO WebSocket: `src/routes/nio.rs`
- ATX client: `src/device/atx_client.rs`

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 7, Story 7.2]
- [Source: docs/project-context.md#Architecture]
- [Source: src/routes/control.rs — existing inspector endpoints]
- [Source: src/routes/nio.rs — NIO WebSocket proxy]
- [Source: resources/static/js/remote_synchronous.js — all deviceUrl usages]
- [Source: _bmad-output/implementation-artifacts/7-1-proxy-remote-page-device-calls.md — Story 7.1 learnings]

### Git Context

Recent commits show established patterns:
- `d74fd69` — code review fixes including proxy fix for `/devices/{udid}/info` in `remote_synchronous.js`
- Story 7.1 established the `/inspector/{udid}/...` proxy pattern for all device calls
- Shell injection prevention: `is_dangerous_command()` + `has_dangerous_metacharacters()` checks

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

### Completion Notes List

1. **Task 1 & 2 — deviceUrl elimination**: Removed `deviceUrl` computed property, `deviceIp` data, `device: {ip, port}` from both `remote_synchronous.js` and `device_synchronous.html`. Replaced 3 HTTP calls (`saveScreenshot`, `fixRotation`, `shell`) with server-proxied `/inspector/{udid}/...` endpoints.

2. **Task 3 — NIO WebSocket migration**: Rewrote `openScreenStream()` to connect via `/nio/{udid}/ws` instead of direct `ws://device_ip:port/minicap`. Key discovery: NIO sends **binary JPEG frames** (not base64 JSON as documented in dev notes). Client uses `ws.binaryType = 'arraybuffer'` and creates Blob from ArrayBuffer for canvas rendering.

3. **Task 4 — Touch proxy migration**: Complete rewrite of `enableTouch()`. Replaced minitouch WebSocket protocol (touchDown/touchMove/touchUp/touchCommit with percentage coords) with HTTP POST to `/inspector/{udid}/touch` using pixel coordinates. Added `sendTouchToDevices()` helper and `getTargetDevices()` method. Implements tap vs swipe detection (distance < 10px = tap, else swipe with start/end coords).

4. **Task 5 — Keyevent fix**: Replaced legacy `/devices/shell/input keyevent` batch URL with per-device `/inspector/{udid}/keyevent` POST calls. Removed `this.shell()` fallback.

5. **Task 6 — Dead code cleanup**: Removed `window.refersh()` typo call and dead `inputText` watcher (inputWS was never initialized).

6. **Task 7 — Backend cleanup**: Removed unused `IP`/`Port` template context variables from `async_list_get()` and `async_list_page()` in `control.rs`.

7. **Task 8 — Regression verification**: Build passes with 0 new warnings. 168/177 tests pass (9 pre-existing failures). `grep -r` confirms no hardcoded IPs (7912, 6677) or `deviceUrl` references remain.

### Code Review Fixes (2026-03-10)

**Reviewer:** Claude Opus 4.6 (adversarial code review)

- **H1 (CRITICAL)**: `targets[i].udid` was `undefined` — Vue `deviceList` items missing `udid` property. All proxy calls would 404 at `/inspector/undefined/...`. Fixed by adding `udid`, `width`, `height` to deviceList push in mounted().
- **H2 (HIGH)**: `toggleScreen()` broken — `screenWS` changed from array to single WebSocket but loop logic not updated. Fixed by rewriting to match `device_synchronous.html` pattern.
- **H3 (HIGH)**: `hold()` crashed — referenced `this.control` (null after minitouch removal). Fixed by rewriting to use HTTP touch proxy via `sendTouchToDevices()`.
- **M1 (MEDIUM)**: 356 lines of commented-out dead code (old minicap/minitouch functions). Removed entirely (852 → 496 lines).
- **M2 (MEDIUM)**: `masterIndex` missing from Vue data — `toDeviceCoords()` always used default 1080x1920. Fixed by adding `masterIndex: 0` to data.
- **M3 (MEDIUM)**: Dead data properties (`control`, `control_list`, `inputWS`) never cleaned up after minitouch removal. Removed all three.
- **L1 (LOW)**: Device resolution not stored in deviceList items. Fixed as part of H1 (added `width`/`height`).

### File List

- resources/static/js/remote_synchronous.js
- resources/templates/device_synchronous.html
- src/routes/control.rs
