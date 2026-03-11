# Story 10.3: Diagnostic Test Page

Status: done

## Story

As a **developer**,
I want **test.html to use modern NIO/scrcpy endpoints**,
so that **I can diagnose screen streaming and touch issues without hardcoded IPs**.

## Acceptance Criteria

1. **Given** test.html is loaded in a browser **When** a device UDID is specified **Then** screen streaming connects via `/nio/{udid}/ws` or `/scrcpy/{udid}/ws`
2. **Given** test.html is loaded **When** a user interacts with the canvas **Then** touch input goes through `/inspector/{udid}/touch`
3. **Given** test.html source code **When** inspected **Then** no hardcoded IP addresses exist in the page

## Tasks / Subtasks

- [x] Task 1: Verify existing test.html compliance (AC: #1, #2, #3)
  - [x] 1.1 Verify screen streaming uses `/nio/{udid}/ws` via server-relative URL — confirm at `test.html:44` that `ws` connects to `protocol + "//" + location.host + "/nio/" + udid + "/ws"` with no hardcoded IPs
  - [x] 1.2 Verify touch input uses `/inspector/{udid}/touch` — confirm `sendTap()` at line 118 and `sendSwipe()` at line 129 use `"/inspector/" + udid + "/touch"` with no hardcoded IPs
  - [x] 1.3 Grep entire test.html for any hardcoded IP patterns (regex: `\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}`) — confirm zero matches
  - [x] 1.4 Verify UDID is extracted from query parameter `?udid=` (line 28) — no hardcoded device identifiers
- [x] Task 2: Add diagnostic enhancements to test.html (AC: #1)
  - [x] 2.1 Add frame rate counter — display FPS in status bar to help diagnose streaming performance
  - [x] 2.2 Add touch coordinate display — show last tap/swipe coordinates in status bar for debugging input mapping
  - [x] 2.3 Add connection info display — show WebSocket URL and connection state for debugging connectivity
- [x] Task 3: Regression testing (AC: #1-#3)
  - [x] 3.1 Build succeeds — 0 new warnings
  - [x] 3.2 All existing tests pass (181/181)
  - [x] 3.3 No new regressions introduced

## Dev Notes

### Critical Discovery — Test Page Already Compliant

**All 3 acceptance criteria are ALREADY MET by the current test.html.** The page was modernized during Epic 7 (Server Proxy Migration) and already uses:

| AC | Requirement | Status | Evidence |
|----|-------------|--------|----------|
| #1 | NIO/scrcpy streaming | ✅ Done | `test.html:44` — `new WebSocket(protocol + "//" + location.host + "/nio/" + udid + "/ws")` |
| #2 | Touch via inspector | ✅ Done | `test.html:118,129` — `fetch("/inspector/" + udid + "/touch", ...)` |
| #3 | No hardcoded IPs | ✅ Done | Uses `location.host` and `location.protocol` throughout |

**The primary work is verification (Task 1) and adding diagnostic enhancements (Task 2) to make the page a better diagnostic tool.**

### Current test.html Implementation (182 lines)

The page is a self-contained diagnostic tool with inline JavaScript (no external JS dependencies):

**Screen Streaming (lines 42-93):**
- Connects to `/nio/{udid}/ws` with dynamic protocol detection (`wss:` for HTTPS, `ws:` for HTTP)
- Sends subscribe message: `{type: "subscribe", target: "screenshot", interval: 100}`
- Handles binary frames (ArrayBuffer → Blob → canvas) and text frames (JSON logging)
- WebSocket close/error handlers update status display

**Touch Input (lines 95-176):**
- Three-state mouse FSM: mouseDown → mouseMove → mouseUp
- Coordinate scaling: `canvas.width / canvas.offsetWidth` for proper device coordinate mapping
- Tap: `{x, y}` payload to `/inspector/{udid}/touch`
- Swipe: `{x, y, x2, y2, duration: 200}` payload to same endpoint

**UI (minimal):**
- Dark theme (`#1a1a1a` background), crosshair cursor on canvas
- Status bar shows connection state, error bar shows failures
- UDID from `?udid=` query parameter, usage message if missing

### Route & Handler — No Changes Needed

| Component | Location | Status |
|-----------|----------|--------|
| Route registration | `src/main.rs:115` — `/test` → `routes::control::test_page` | Exists |
| Handler | `src/routes/control.rs:258-263` — renders `test.html` template | Exists |
| Template | `resources/templates/test.html` (182 lines) | Exists |

The handler is a simple template render with no AppState dependencies — no backend changes needed.

### Diagnostic Enhancement Design (Task 2)

Add lightweight diagnostic info to the status bar. Keep changes minimal — inline JS only, no new dependencies:

**2.1 Frame Rate Counter:**
```javascript
var frameCount = 0;
var lastFpsUpdate = Date.now();
var currentFps = 0;

// Inside ws.onmessage, after drawing frame:
frameCount++;
var now = Date.now();
if (now - lastFpsUpdate >= 1000) {
    currentFps = frameCount;
    frameCount = 0;
    lastFpsUpdate = now;
    updateStatus();
}
```

**2.2 Touch Coordinate Display:**
```javascript
var lastAction = '';

// Inside sendTap():
lastAction = 'tap(' + x + ', ' + y + ')';
updateStatus();

// Inside sendSwipe():
lastAction = 'swipe(' + x1 + ',' + y1 + ' → ' + x2 + ',' + y2 + ')';
updateStatus();
```

**2.3 Connection Info & Combined Status:**
```javascript
function updateStatus() {
    var parts = ['Connected — ' + udid];
    if (currentFps > 0) parts.push(currentFps + ' fps');
    if (lastAction) parts.push('Last: ' + lastAction);
    statusEl.innerText = parts.join(' | ');
}
```

### What NOT to Implement

- Do NOT add scrcpy fallback — the AC says "via `/nio/{udid}/ws` **or** `/scrcpy/{udid}/ws`" and NIO is already working. Adding scrcpy toggle is out of scope
- Do NOT add external JS dependencies (jQuery, Vue) — keep the page self-contained
- Do NOT add backend routes or handlers — template rendering is sufficient
- Do NOT add keyboard input or complex diagnostic panels — keep enhancements minimal
- Do NOT modify remote.js or other pages — scope is test.html only

### Project Structure Notes

- Modified: `resources/templates/test.html` — add diagnostic enhancements (FPS, coordinates, connection info)
- NO new files needed
- NO backend changes needed (route and handler already exist)
- NO new routes needed
- NO database changes needed
- NO JS file changes needed (all JS is inline in template)

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 10, Story 10.3]
- [Source: resources/templates/test.html:44 — NIO WebSocket connection (already compliant)]
- [Source: resources/templates/test.html:118,129 — touch endpoint (already compliant)]
- [Source: src/main.rs:115 — /test route registration]
- [Source: src/routes/control.rs:258-263 — test_page handler]
- [Source: _bmad-output/implementation-artifacts/10-2-device-reservation-system.md — Story 10.2 patterns]

### Git Context

Recent commits establish these patterns:
- Story 10.2 established DashMap entry API for atomic operations, DB error logging
- Story 10.1 established version endpoint with OpenAPI spec + test coverage requirements
- Code reviews consistently catch missing tests — always verify test coverage

### Previous Story Intelligence (Story 10.2)

Critical lessons to apply:
- **Test coverage**: Add integration tests where possible — code review will catch gaps
- **Build verification**: Ensure 0 new warnings, all 181 tests pass
- **Minimal changes**: Story 10.2 only modified 3 files — keep scope tight
- **Verify before implementing**: Task 1 verifies existing compliance before adding enhancements

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Build: 0 errors, 0 new warnings
- Tests: 181/181 passed (0 failures)

### Completion Notes List

- Task 1: Verified all 3 ACs already met — test.html:44 uses `/nio/{udid}/ws` via `location.host` (AC#1), test.html:118,129 use `/inspector/{udid}/touch` (AC#2), zero hardcoded IP matches via regex grep (AC#3). UDID extracted from `?udid=` query parameter with no hardcoded device identifiers.
- Task 2: Added lightweight diagnostic enhancements — FPS counter (frameCount + 1s interval update), touch coordinate display (lastAction updated in sendTap/sendSwipe), connection info display (wsUrl shown on connect, combined updateStatus() function). All inline JS, no new dependencies.
- Task 3: Build succeeds with 0 new warnings, all 181 tests pass with 0 regressions. No new tests needed — changes are frontend-only in a Tera template.
- Only 1 file modified — minimal, focused implementation adding diagnostic value to an already-compliant page.

### Code Review Fixes (2026-03-10)

- **M1 FIXED**: updateStatus() overwrote disconnect messages — added `wsConnected` guard so updateStatus() returns early when WebSocket is closed, preventing misleading "Connected" text after disconnect
- **M2 FIXED**: FPS counter froze at last value when streaming stopped — added 2-second setInterval that resets currentFps to 0 when no frames arrive; also reset currentFps in ws.onclose/onerror
- **M3 FIXED**: No connection state tracking — added `var wsConnected = false;` flag, set true in ws.onopen and false in ws.onclose/ws.onerror; updateStatus() now checks state before updating
- **L1 FIXED**: Status bar color inconsistency — updateStatus() now resets statusEl.style.color to '#aaa' when connected, preventing "Connected" text showing in red after a prior disconnect

### File List

- resources/templates/test.html (added diagnostic enhancements: FPS counter, touch coordinates, connection info display)
