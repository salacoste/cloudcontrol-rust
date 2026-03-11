# Story 7.3: Server-Side Proxy Endpoints

Status: done

## Story

As a **backend developer**,
I want **all missing server-side proxy endpoints implemented and the last remaining page (test.html) updated to use them**,
so that **the frontend can route all device communication through the server with zero hardcoded IPs**.

## Acceptance Criteria

1. **Given** the server is running **When** the frontend calls `POST /inspector/{udid}/rotation` **Then** the rotation fix is forwarded to the device ATX agent and the response returned
2. **Given** existing endpoints (`/inspector/{udid}/shell`, `/inspector/{udid}/screenshot`, `/inspector/{udid}/upload`) **When** called from the frontend **Then** they work correctly for proxied calls with proper error handling
3. **Given** `test.html` is loaded in a browser **When** a device UDID is specified **Then** screen streaming connects via `/nio/{udid}/ws` instead of hardcoded `ws://172.17.2.23:7912/minicap`
4. **Given** `test.html` is loaded **When** touch input occurs **Then** it goes through `/inspector/{udid}/touch` instead of hardcoded `http://172.17.2.228:8000/devices/touch/`
5. **Given** `test.html` is loaded **When** inspecting the code **Then** no hardcoded IP addresses exist in the page
6. **Given** all frontend files in `resources/` **When** searching for hardcoded IPs or ports (7912, 6677, direct device URLs) **Then** no functional hardcoded references remain (comments documenting removal are acceptable)

## Tasks / Subtasks

- [x] Task 1: Verify existing server-side proxy endpoints work (AC: #1, #2)
  - [x] 1.1 Verify `POST /inspector/{udid}/rotation` is registered in `main.rs` and implemented in `control.rs`
  - [x] 1.2 Verify `POST /inspector/{udid}/shell` and `GET /inspector/{udid}/shell` have `is_dangerous_command()` + `has_dangerous_metacharacters()` security checks
  - [x] 1.3 Verify all existing inspector endpoints (`screenshot`, `screenshot/img`, `touch`, `input`, `keyevent`, `hierarchy`, `upload`) are registered and functional
  - [x] 1.4 Confirm no missing proxy endpoints needed for frontend operations
- [x] Task 2: Rewrite `test.html` screen streaming to use NIO WebSocket proxy (AC: #3, #5)
  - [x] 2.1 Remove hardcoded `ws://172.17.2.23:7912/minicap` WebSocket connection
  - [x] 2.2 Add device UDID parameter (from URL query param or template variable)
  - [x] 2.3 Connect via NIO WebSocket: `protocol + "//" + location.host + "/nio/" + udid + "/ws"`
  - [x] 2.4 Send NIO subscribe message: `{"type": "subscribe", "target": "screenshot", "interval": 100}`
  - [x] 2.5 Handle NIO binary JPEG frames (ArrayBuffer) — same pattern as `remote_synchronous.js`
  - [x] 2.6 Remove old minicap binary blob handling and `ws.send('1920x1080/0')` init message
- [x] Task 3: Rewrite `test.html` touch input to use server proxy (AC: #4, #5)
  - [x] 3.1 Remove hardcoded `http://172.17.2.228:8000/devices/touch/` URL
  - [x] 3.2 Replace `request().get()` tap call with `POST /inspector/{udid}/touch` using `{x, y}` JSON body
  - [x] 3.3 Replace `request().get()` swipe call with `POST /inspector/{udid}/touch` using `{x, y, x2, y2, duration}` JSON body
  - [x] 3.4 Remove custom `request()` XMLHttpRequest wrapper — use `fetch()`
  - [x] 3.5 Coordinate calculation uses canvas pixel scaling (canvas.width/offsetWidth ratio)
- [x] Task 4: Add test.html route handler (AC: #3)
  - [x] 4.1 Added `GET /test` route in `main.rs` → `control::test_page`
  - [x] 4.2 UDID read from `?udid=` query parameter via `URLSearchParams` in JavaScript
- [x] Task 5: Final hardcoded IP sweep (AC: #6)
  - [x] 5.1 Searched all files in `resources/` for patterns: `7912`, `6677`, `deviceUrl`, `device.ip`, `device.port`, `ws://[0-9]`, `http://[0-9]`
  - [x] 5.2 Only comments remain (no functional code referencing direct device IPs)
  - [x] 5.3 Confirmed `index.html` references to `192.168.1.100:5555` are placeholder/documentation text only
- [x] Task 6: Regression testing (AC: #1, #2)
  - [x] 6.1 Build succeeds — 0 new warnings (5 pre-existing)
  - [x] 6.2 All existing tests pass (168/177, 9 pre-existing failures)
  - [x] 6.3 No new regressions introduced

## Dev Notes

### Critical Architecture Constraint

**ALL browser-to-device calls MUST go through the CloudControl server.** The browser may be on a completely different network than the devices. Direct calls to device IPs will fail in production.

### What's Already Done (DO NOT RE-IMPLEMENT)

Stories 7.1 and 7.2 already completed the heavy lifting. These proxy endpoints are **already implemented and registered**:

| Endpoint | Method | Status | File:Line |
|----------|--------|--------|-----------|
| `/inspector/{udid}/screenshot` | GET | ✅ Done | `control.rs` |
| `/inspector/{udid}/screenshot/img` | GET | ✅ Done | `control.rs` |
| `/inspector/{udid}/touch` | POST | ✅ Done | `control.rs` |
| `/inspector/{udid}/input` | POST | ✅ Done | `control.rs` |
| `/inspector/{udid}/keyevent` | POST | ✅ Done | `control.rs` |
| `/inspector/{udid}/hierarchy` | GET | ✅ Done | `control.rs` |
| `/inspector/{udid}/upload` | POST | ✅ Done | `control.rs` |
| `/inspector/{udid}/rotation` | POST | ✅ Done | `control.rs:1611-1640`, `main.rs:171-174` |
| `/inspector/{udid}/shell` | POST | ✅ Done | `control.rs:1645-1686`, `main.rs:176-179` |
| `/inspector/{udid}/shell` | GET | ✅ Done | `control.rs:1689-1728`, `main.rs:180-183` |
| `/nio/{udid}/ws` | WebSocket | ✅ Done | `nio.rs` |
| `/devices/{udid}/shell` | WebSocket | ✅ Done | `control.rs` |
| `/devices/{udid}/info` | GET | ✅ Done | `control.rs` |

### The Remaining Work: `test.html` Migration

`test.html` is the **only remaining file** with hardcoded device IPs. It has 3 problems:

1. **Line 21**: `ws://172.17.2.23:7912/minicap` — direct minicap WebSocket to device
2. **Line 104**: `http://172.17.2.228:8000/devices/touch/` — direct tap URL
3. **Line 119**: `http://172.17.2.228:8000/devices/touch/` — direct swipe URL

The file also uses a legacy approach:
- Custom `request()` XMLHttpRequest wrapper instead of `fetch()` or jQuery
- `ws.send('1920x1080/0')` minicap init message (NIO doesn't need this)
- Old-style blob handling (`ws.binaryType = 'blob'`) — NIO sends ArrayBuffer

### NIO WebSocket Protocol (established in Story 7.2)

```javascript
// Connect to NIO
var protocol = location.protocol === "https:" ? "wss:" : "ws:";
var ws = new WebSocket(protocol + "//" + location.host + "/nio/" + udid + "/ws");
ws.binaryType = 'arraybuffer';

// Subscribe to screenshots
ws.onopen = function() {
    ws.send(JSON.stringify({
        type: "subscribe",
        target: "screenshot",
        interval: 100
    }));
};

// Receive binary JPEG frames
ws.onmessage = function(message) {
    if (message.data instanceof ArrayBuffer) {
        var blob = new Blob([message.data], { type: 'image/jpeg' });
        var url = URL.createObjectURL(blob);
        var img = new Image();
        img.onload = function() {
            canvas.width = img.width;
            canvas.height = img.height;
            ctx.drawImage(img, 0, 0);
            img.onload = img.onerror = null;
            img.src = BLANK_IMG;
            URL.revokeObjectURL(url);
        };
        img.src = url;
    }
};
```

### Touch Proxy Pattern (established in Story 7.2)

```javascript
// Tap
fetch("/inspector/" + udid + "/touch", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ x: pixelX, y: pixelY })
});

// Swipe
fetch("/inspector/" + udid + "/touch", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ x: startX, y: startY, x2: endX, y2: endY, duration: 200 })
});
```

### Device UDID for test.html

`test.html` currently has no concept of device UDID — it uses hardcoded IPs. Options:
1. **URL query parameter**: `/test?udid=XXXXX` — simplest, read from `URLSearchParams`
2. **Template variable**: Like other pages, pass `{{ Udid }}` from Rust handler

**Recommendation:** Use URL query parameter since `test.html` is a simple diagnostic tool. If no UDID is provided, show a device picker or error message. Check if a route already exists for test.html — if not, it can be served as a static file with JS reading the query param.

### Route Check for test.html

Search `main.rs` for any existing `/test` route. If none exists, the simplest approach is to read the UDID from URL query parameter `?udid=XXX` in JavaScript, requiring no backend changes.

### index.html IP References (NOT a problem)

`index.html` has references to `192.168.1.100:5555` — these are **placeholder text** in the manual device connection form, not functional code. They appear as:
- Input placeholder text showing expected format
- Help text example

These are documentation, not hardcoded device connections. **Do not change.**

### Story 7.1 and 7.2 Code Review Learnings

Apply these patterns established in prior stories:
- **Security**: Shell proxy endpoints need both `is_dangerous_command()` AND `has_dangerous_metacharacters()` checks
- **Dead code**: Remove commented-out code entirely, don't leave it in the file
- **Null guards**: Functions that reference WebSockets need null checks
- **Comments**: Leave a brief `// removed — proxied through server` comment, not the old code

### Project Structure Notes

- Test page: `resources/templates/test.html`
- Backend routes: `src/routes/control.rs`
- Route registration: `src/main.rs`
- NIO WebSocket: `src/routes/nio.rs`
- Established patterns: `resources/static/js/remote_synchronous.js` (Story 7.2)

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 7, Story 7.3]
- [Source: docs/project-context.md#Architecture]
- [Source: src/routes/control.rs — inspector_rotation:1611-1640, inspector_shell:1645-1728]
- [Source: src/main.rs — route registrations:171-183]
- [Source: resources/templates/test.html — hardcoded IPs at lines 21, 104, 119]
- [Source: _bmad-output/implementation-artifacts/7-2-proxy-batch-control-page-device-calls.md — Story 7.2 patterns]

### Git Context

Recent commits show established patterns:
- `d74fd69` — code review fixes including proxy fix for `/devices/{udid}/info`
- Story 7.1 established `/inspector/{udid}/...` proxy pattern for all device calls
- Story 7.2 established NIO WebSocket pattern for screen streaming
- Shell injection prevention: `is_dangerous_command()` + `has_dangerous_metacharacters()` checks

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

N/A — no debug issues encountered.

### Completion Notes List

1. All 10 server-side proxy endpoints verified as registered and functional (Task 1)
2. `test.html` completely rewritten — removed all 3 hardcoded IPs (minicap WS, 2x touch URLs)
3. Screen streaming now uses NIO WebSocket proxy (`/nio/{udid}/ws`) with ArrayBuffer handling
4. Touch input now uses `fetch()` + `POST /inspector/{udid}/touch` with JSON body for both tap and swipe
5. Custom `request()` XMLHttpRequest wrapper removed, replaced with modern `fetch()` API
6. Device UDID read from URL query parameter (`?udid=XXX`) with error message if missing
7. Added `GET /test` route in `main.rs` and `test_page` handler in `control.rs`
8. Final IP sweep: zero functional hardcoded IPs remain in `resources/` — only comments and placeholder text
9. Build: 0 new warnings, 168/177 tests pass (9 pre-existing failures unchanged)

### Code Review Fixes

- **M1 fix**: mouseup listener moved from canvas to document (dynamically added/removed on mousedown/mouseup) — prevents stale `mouseDown` state when drag ends outside canvas. Follows `remote_synchronous.js` pattern.
- **M2 fix**: Added `.catch()` error handlers to `sendTap()` and `sendSwipe()` fetch calls — logs errors to console and shows status feedback on the page. Critical for a diagnostic tool.

### File List

- resources/templates/test.html (rewritten — removed hardcoded IPs, added NIO WS + proxy touch)
- src/main.rs (added `/test` route registration)
- src/routes/control.rs (added `test_page` handler)
