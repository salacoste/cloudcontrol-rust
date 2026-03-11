# Story 7.1: Proxy Remote Page Device Calls

Status: done

## Story

As a **device farm operator**,
I want **the remote control page to communicate with devices through the server**,
so that **I can control devices from any network, not just the local LAN**.

## Acceptance Criteria

1. **Given** the remote control page is open in a browser **When** any operation (shell, screenshot, rotation fix, upload, minicap fix) is triggered **Then** the request goes to the CloudControl server, not directly to the device IP
2. **Given** `remote.js` is loaded **When** inspecting the code **Then** the `deviceUrl` computed property is removed and all `this.deviceUrl + "/..."` calls are replaced with server proxy endpoints
3. **Given** `remote.js` has a `loadWhatsinput()` function **When** the page connects to the whatsinput WebSocket **Then** it connects via `ws://location.host/devices/{udid}/whatsinput` (server proxy) OR is gracefully disabled if proxy not feasible
4. **Given** `remote.js` is loaded **When** inspecting the code **Then** no hardcoded IP addresses or port numbers (7912, 6677) remain
5. **Given** existing proxied functionality (touch, input, keyevent, hierarchy, screenshot via `/inspector/` or NIO) **When** the page is used **Then** all existing proxied features continue working without regression

## Tasks / Subtasks

- [x] Task 1: Remove `deviceUrl` computed property and replace all direct calls (AC: #1, #2)
  - [x] 1.1 Removed `deviceUrl` computed property
  - [x] 1.2 Replaced shell in `runShell()` → `/inspector/{udid}/shell`
  - [x] 1.3 Replaced shell in `shell()` → `/inspector/{udid}/shell`
  - [x] 1.4 Replaced screenshot in `startLowQualityScreenRecord()` → `/inspector/{udid}/screenshot/img?s=0.4&q=50`
  - [x] 1.5 Replaced screenshot in `saveScreenshot()` → `/inspector/{udid}/screenshot/img`
  - [x] 1.6 Replaced upload in `uploadFile()` → `/inspector/{udid}/upload`
  - [x] 1.7 Replaced rotation in `fixRotation()` → `/inspector/{udid}/rotation`
  - [x] 1.8 Replaced shell in `fixMinicap()` → `/inspector/{udid}/shell`
  - [x] 1.9 Simplified `fixMinicap()` — removed legacy `/minicap` PUT (NIO/scrcpy replaces minicap)
- [x] Task 2: Fix whatsinput WebSocket (AC: #3)
  - [x] 2.1 Gracefully disabled whatsinput — direct device port 6677 not available in proxied mode
- [x] Task 3: Remove hardcoded IP/port references (AC: #4)
  - [x] 3.1 Removed `device: {ip, port}` data properties
  - [x] 3.2 Removed `LOCAL_URL` and `LOCAL_VERSION` constants
  - [x] 3.3 Updated `checkVersion()` — relative URL, removed version mismatch check
  - [x] 3.4 Replaced `LOCAL_URL + 'inspector/'` with `'/inspector/'` (2 occurrences)
  - [x] 3.5 Updated `remote.html` — replaced device IP with UDID, terminal link → server proxy
  - [x] 3.6 Removed `deviceIp`/`devicePort` JS vars from template
- [x] Task 4: Add rotation proxy endpoint (AC: #1)
  - [x] 4.1 Added `POST /inspector/{udid}/rotation` in `control.rs`
  - [x] 4.2 Registered route in `main.rs`
  - [x] 4.3 Forwards POST to device ATX agent `/info/rotation`
- [x] Task 5: Add shell proxy endpoint (AC: #1)
  - [x] 5.1 Added `POST /inspector/{udid}/shell` (form data)
  - [x] 5.2 Added `GET /inspector/{udid}/shell` (query param, legacy support)
  - [x] 5.3 Both use `AtxClient::shell_cmd()` + ADB fallback + dangerous command blocking
  - [x] 5.4 Added `base_url()` and `http_client()` accessors to `AtxClient`
- [x] Task 6: Regression testing (AC: #5)
  - [x] 6.1 Build succeeds — 0 new warnings
  - [x] 6.2 168/177 tests pass — 9 pre-existing failures (coordinate clamping)
  - [x] 6.3 No new regressions introduced

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
| `/nio/{udid}/ws` | WebSocket | Multiplexed screenshot + control | `nio.rs` |
| `/devices/{udid}/shell` | WebSocket | Interactive shell | `control.rs` |
| `/api/devices/{udid}/shell` | POST | Shell command execution | `control.rs` |

### Endpoints to ADD (missing)

| Endpoint | Method | Purpose | Proxies To |
|----------|--------|---------|------------|
| `/inspector/{udid}/rotation` | POST | Fix device rotation | `POST http://{ip}:{port}/info/rotation` on device |
| `/inspector/{udid}/shell` | POST | HTTP shell command (not WS) | `AtxClient::shell_cmd()` + ADB fallback |

### Upload Endpoint Adaptation

The existing `/inspector/{udid}/upload` endpoint handles multipart upload. Check its implementation matches what `remote.js` sends — the frontend uses `FormData` with the file attached. The endpoint must forward to the device's `/upload/sdcard/tmp/` path.

### Whatsinput Decision

Port 6677 `whatsinput` is a minitouch-related service. Options:
1. **Proxy it** — add `WS /devices/{udid}/whatsinput` that opens upstream WS to device port 6677
2. **Gracefully disable** — wrap in try/catch, log warning, continue without it (non-essential feature)

Recommend option 2 for now (graceful disable) — whatsinput is used for detecting physical keyboard input on the device, which is rarely needed in remote farm control. Can be proxied in a future story if needed.

### Screenshot URL Differences

- Device ATX agent: `GET /screenshot` returns JSON `{"data": "base64..."}` and `GET /screenshot/0` returns raw JPEG
- Server proxy: `/inspector/{udid}/screenshot` returns JSON, `/inspector/{udid}/screenshot/img` returns raw JPEG
- For `saveScreenshot()` which downloads the file — use `/inspector/{udid}/screenshot/img`
- For `startLowQualityScreenRecord()` which fetches thumbnails — use `/inspector/{udid}/screenshot/img?thumbnail=800x800` (verify the endpoint supports `thumbnail` query param, or use NIO screenshot with quality setting)

### fixMinicap() Function

This function removes and re-downloads minicap files on the device. Since minicap is a legacy protocol replaced by NIO/scrcpy:
- **Option A**: Remove the function entirely (minicap is deprecated)
- **Option B**: Proxy the shell command part, skip the `/minicap` download

Recommend: Keep the function but route shell commands through proxy. The `/minicap` download endpoint can be skipped or removed — it's a legacy ATX agent feature.

### Code Patterns to Follow

```javascript
// BEFORE (direct device call):
url: this.deviceUrl + "/shell",

// AFTER (server proxy):
url: "/inspector/" + this.deviceUdid + "/shell",

// BEFORE (direct WebSocket):
new WebSocket("ws://" + this.device.ip + ":6677/whatsinput")

// AFTER (graceful disable):
// Whatsinput requires direct device access — disabled in proxied mode
console.log("[whatsinput] Direct device access not available in proxied mode");
```

### Rust Endpoint Pattern

Follow the existing pattern in `control.rs` for new endpoints:

```rust
pub async fn inspector_rotation(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> impl Responder {
    let udid = path.into_inner();
    // 1. Get device from state
    // 2. Get AtxClient from connection pool
    // 3. Forward request to device
    // 4. Return response
}
```

Use `get_device_client()` helper (exists in `control.rs`) to resolve device → AtxClient.

### Project Structure Notes

- Frontend JS: `resources/static/js/remote.js`
- Frontend template: `resources/templates/remote.html`
- Backend routes: `src/routes/control.rs`
- Route registration: `src/main.rs`
- ATX client: `src/device/atx_client.rs`
- Connection pool: `src/pool/connection_pool.rs`

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 7, Story 7.1]
- [Source: docs/project-context.md#Architecture]
- [Source: src/routes/control.rs — existing inspector endpoints]
- [Source: src/routes/nio.rs — NIO WebSocket proxy]
- [Source: resources/static/js/remote.js — all deviceUrl usages]

### Git Context

Recent commits show established patterns:
- `d74fd69` — code review fixes including proxy fix for `/devices/{udid}/info` in `remote_synchronous.js` (same pattern needed here)
- `ead09d9` — recording, API, scrcpy features
- Shell injection prevention pattern: validate key names against allowlist before forwarding

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Build: 0 errors, 5 pre-existing warnings (unused functions)
- Tests: 168 pass, 9 pre-existing failures (coordinate clamping tests)

### Completion Notes List

- Removed all 8 direct device HTTP calls from remote.js, replaced with server proxy endpoints
- Gracefully disabled whatsinput WebSocket (port 6677 not available in proxied mode)
- Simplified fixMinicap — removed legacy minicap download, kept shell cleanup via proxy
- Removed LOCAL_URL, LOCAL_VERSION, deviceIp, devicePort — all hardcoded references eliminated
- Added 2 new backend endpoints: POST /inspector/{udid}/rotation and POST|GET /inspector/{udid}/shell
- Added base_url() and http_client() public accessors to AtxClient
- Updated remote.html template — navbar shows UDID instead of IP, terminal link uses server proxy

### Code Review Fixes (Claude Opus 4.6)

- **[H1] Security fix**: Added `has_dangerous_metacharacters()` check to `inspector_shell` and `inspector_shell_get` endpoints — prevents command injection via shell metacharacters (`;`, `&&`, `$()`, etc.)
- **[M1] Dead code cleanup**: Removed unused `IP` and `Port` template context variables from `remote()` handler in `control.rs`
- **[M2] Dead code removal**: Removed `checkVersion()` function body — referenced non-existent `/api/v1/version` endpoint (planned for Epic 10)
- **[M3] Null guard**: Added null checks to `sendInputText()` and `sendInputKey()` — prevents crash when whatsinput WebSocket is disabled

### File List

- resources/static/js/remote.js
- resources/templates/remote.html
- src/routes/control.rs
- src/main.rs
- src/device/atx_client.rs
