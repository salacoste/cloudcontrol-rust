# Story 11.1: Screen-to-Video Recording

Status: done

## Story

As a **QA tester**,
I want **to record the device screen stream into a video file**,
so that **I can review and share test sessions**.

## Acceptance Criteria

1. **Given** a device has an active screen stream (NIO or scrcpy) **When** I start recording via the frontend **Then** a WebSocket endpoint `ws://.../video/convert` accepts JPEG frames **And** frames are composited into a video file (MP4) using FFmpeg **And** recording can be stopped, producing a downloadable video file **And** recording metadata (device, duration, file path) is stored
2. **Given** a video recording is in progress **When** I stop the recording **Then** FFmpeg finishes encoding and produces a valid MP4 file **And** the WebSocket connection is closed gracefully **And** metadata is finalized with actual duration and file size
3. **Given** no FFmpeg binary is available on the system **When** a recording is attempted **Then** the server returns a clear error indicating FFmpeg is required **And** no partial files are left behind
4. **Given** multiple devices are being monitored **When** I start recordings on different devices **Then** each recording operates independently **And** each produces its own MP4 file with device-specific metadata

## Tasks / Subtasks

- [x] Task 1: FFmpeg integration and video service (AC: #1, #3)
  - [x] 1.1 Verify FFmpeg is available: Add startup check function `check_ffmpeg_available()` that runs `ffmpeg -version` via `tokio::process::Command`. Log result at startup. Store availability in `AppState` as `ffmpeg_available: bool`. DO NOT make it a hard requirement — server starts without FFmpeg, but video recording endpoints return 503 if unavailable.
  - [x] 1.2 Create `src/services/video_service.rs` — VideoService struct with:
    - `active_recordings: Arc<DashMap<String, VideoRecordingState>>` keyed by recording ID (UUID)
    - `VideoRecordingState`: `id`, `udid`, `file_path`, `started_at`, `frame_count`, `status` (recording/finalizing/completed/failed), `ffmpeg_handle` (Child process)
    - `start_recording(udid: &str) -> Result<String, String>` — spawns FFmpeg child process with stdin pipe, returns recording ID
    - `feed_frame(id: &str, jpeg_data: &[u8]) -> Result<(), String>` — writes JPEG frame to FFmpeg stdin
    - `stop_recording(id: &str) -> Result<VideoRecordingInfo, String>` — closes FFmpeg stdin (triggers finalization), waits for process exit, returns metadata
    - `get_recording(id: &str) -> Option<VideoRecordingInfo>` — get metadata
    - `list_recordings() -> Vec<VideoRecordingInfo>` — list all completed recordings
  - [x] 1.3 FFmpeg command construction: `ffmpeg -f image2pipe -framerate {fps} -i pipe:0 -c:v libx264 -pix_fmt yuv420p -preset fast -crf 23 -movflags +faststart {output_path}`. Input via stdin pipe (`Stdio::piped()`). Output to `recordings/video_{udid}_{timestamp}.mp4`. The `-movflags +faststart` flag is CRITICAL — it puts the moov atom at the start of the MP4 for web playback.
  - [x] 1.4 Add `video_service: VideoService` to `AppState` in `src/state.rs`. Initialize in `main.rs` with `VideoService::new()`.
  - [x] 1.5 Add `src/services/video_service.rs` to `src/services/mod.rs`

- [x] Task 2: WebSocket endpoint for JPEG frame ingestion (AC: #1, #4)
  - [x] 2.1 Create `src/routes/video_ws.rs` — WebSocket handler for `ws://.../video/convert`. Parse query params: `fps` (default: 2), `udid` (required), `name` (optional device name). On connection open: call `video_service.start_recording(udid)` with fps. On binary frame: call `video_service.feed_frame(id, data)`. On connection close: call `video_service.stop_recording(id)`.
  - [x] 2.2 Register WebSocket route in `src/main.rs`: `.route("/video/convert", web::get().to(routes::video_ws::video_convert_ws))` — place in the WebSocket routes section near scrcpy_ws routes. This matches the original frontend URL pattern.
  - [x] 2.3 Handle connection cleanup: If the client disconnects without sending stop, auto-stop the recording. If FFmpeg is not available, reject the WebSocket upgrade with a close frame containing the error message.
  - [x] 2.4 Add `src/routes/video_ws.rs` to `src/routes/mod.rs`

- [x] Task 3: REST API endpoints for video management (AC: #1, #2)
  - [x] 3.1 Add video REST endpoints in `src/routes/api_v1.rs` (NOT a new file — follow existing api_v1 pattern):
    - `GET /api/v1/videos` — list all completed video recordings with metadata
    - `GET /api/v1/videos/{id}` — get single recording metadata
    - `GET /api/v1/videos/{id}/download` — serve MP4 file via `actix_files::NamedFile`
    - `DELETE /api/v1/videos/{id}` — delete recording + file
    - `POST /api/v1/videos/{id}/stop` — force-stop an in-progress recording
  - [x] 3.2 Register routes in `src/main.rs` in the api_v1 block
  - [x] 3.3 Add OpenAPI spec entries for all video endpoints in `src/models/openapi.rs`

- [x] Task 4: Frontend recording controls (AC: #1, #2)
  - [x] 4.1 Re-implement `connectImage2VideoWebSocket` in `resources/static/js/remote.js` — this method was removed as dead code in Story 10.4 because the server endpoint didn't exist. Now the endpoint exists. Restore the method (reference git history: `git show d74fd69:resources/static/js/remote.js`). The original implementation connects to `ws://.../video/convert?fps=N&udid=UDID&name=MODEL`.
  - [x] 4.2 Re-implement `startLowQualityScreenRecord` — takes screenshots at configurable FPS and sends JPEG blobs to the WebSocket. Uses `setInterval` with periodic screenshot fetch + `ws.send(blob)`. Store the interval key and WebSocket reference in `this.videoReceiver`.
  - [x] 4.3 Re-implement `stopVideoRecord` — clears the interval, closes the WebSocket, nulls `videoReceiver`. The `videoReceiver` data property already exists at line 92 (was kept as Vue initialization).
  - [x] 4.4 Verify HTML templates have the recording UI buttons. Check `remote.html` for existing record/stop buttons that called these methods. If buttons exist, they should work now. If not, add minimal controls.

- [x] Task 5: Integration tests (AC: #1-#4)
  - [x] 5.1 Add `test_video_list_empty` — GET `/api/v1/videos` returns 200 with empty array
  - [x] 5.2 Add `test_video_get_not_found` — GET `/api/v1/videos/nonexistent` returns 404
  - [x] 5.3 Add `test_video_delete_not_found` — DELETE `/api/v1/videos/nonexistent` returns 404
  - [x] 5.4 Add `test_video_stop_not_found` — POST `/api/v1/videos/nonexistent/stop` returns 404
  - [x] 5.5 Register all video routes in `setup_test_app!` macro in `tests/test_server.rs`
  - [x] 5.6 Add OpenAPI completeness assertions for new video endpoints

- [x] Task 6: Regression testing (AC: #1-#4)
  - [x] 6.1 Build succeeds — 0 new warnings
  - [x] 6.2 All existing tests pass (293 existing + new tests)
  - [x] 6.3 No new regressions introduced

## Dev Notes

### Scope — JPEG-to-MP4 Video Recording Pipeline

This story creates a **new video recording pipeline** that captures JPEG screenshot frames from the browser and composes them into MP4 video files using FFmpeg. This is DISTINCT from the existing recording systems:

| System | What it records | Format | Source |
|--------|----------------|--------|--------|
| **Action Recording** (Epic 4) | User interactions (tap, swipe, etc.) | SQLite rows | Server-side event capture |
| **Scrcpy Recording** (Epic 6.4) | H.264 stream from device | Raw H.264 Annex B | Device scrcpy protocol |
| **Video Recording** (THIS STORY) | JPEG screenshots from browser | MP4 (H.264 via FFmpeg) | Browser WebSocket frames |

### Original Frontend Design (Removed in Story 10.4)

The legacy frontend had this exact feature implemented client-side but the server endpoint (`/video/convert`) was never created. The original code (from git history `d74fd69`):

```javascript
connectImage2VideoWebSocket: function (fps) {
    var protocol = location.protocol == "http:" ? "ws:" : "wss:";
    var wsURL = protocol + location.host + "/video/convert"
    var wsQueries = encodeURI("fps=" + fps) + "&" + encodeURI("udid=" + this.deviceUdid) + "&" + encodeURI("name=" + this.deviceInfo.model)
    var ws = new WebSocket(wsURL + "?" + wsQueries)
    // ...
}
```

The `startLowQualityScreenRecord` method fetches screenshots at 2 FPS and sends JPEG blobs to this WebSocket. The `videoReceiver` data property (still at line 92 in `remote.js`) stores the WebSocket + interval references.

### FFmpeg Command Design

```bash
ffmpeg -f image2pipe -framerate {fps} -i pipe:0 \
       -c:v libx264 -pix_fmt yuv420p -preset fast -crf 23 \
       -movflags +faststart \
       recordings/video_{udid}_{timestamp}.mp4
```

**Key flags:**
- `-f image2pipe`: Read images from stdin pipe
- `-framerate {fps}`: Match the client-side capture rate (default 2 FPS)
- `-c:v libx264`: H.264 codec (universally playable)
- `-pix_fmt yuv420p`: Compatibility format for all players
- `-preset fast`: Balance speed vs compression
- `-crf 23`: Reasonable quality (18=high, 28=low)
- `-movflags +faststart`: **CRITICAL** — moves moov atom to file start for web streaming

### VideoService Design — Follow Existing Patterns

Follow the `ScrcpyManager` pattern from Epic 6:
- **State tracking**: `Arc<DashMap<String, VideoRecordingState>>` (keyed by recording UUID, NOT udid — multiple recordings per device allowed)
- **Child process management**: `tokio::process::Command` for FFmpeg (same pattern as scrcpy server launch)
- **Frame delivery**: Write JPEG bytes to FFmpeg's stdin pipe
- **Lifecycle**: start → feed frames → stop (close stdin → FFmpeg auto-finishes) → metadata finalized
- **Cleanup**: On WebSocket disconnect, auto-stop recording
- **File storage**: `recordings/` directory (already exists, used by scrcpy recordings)

### WebSocket Endpoint Design

The `ws://.../video/convert` endpoint:
1. Parses query: `fps` (int, default 2), `udid` (string, required), `name` (string, optional)
2. Checks `state.ffmpeg_available` — rejects if false
3. Calls `video_service.start_recording(udid, fps)` → spawns FFmpeg child
4. On each binary WebSocket frame → `video_service.feed_frame(id, &data)`
5. On close/error → `video_service.stop_recording(id)`

Follow the `scrcpy_ws.rs` pattern for WebSocket setup using `actix_ws`.

### REST API Pattern — Follow api_v1.rs Conventions

Video REST endpoints go in `api_v1.rs` following existing patterns:
- Use `success_response()` and `error_response()` helpers
- Use `ApiResponse` struct wrapping
- Add OpenAPI spec entries
- Register in `main.rs` api_v1 block

### Error Handling

- **FFmpeg not available**: Return 503 with `ERR_SERVICE_UNAVAILABLE` and message "FFmpeg is required for video recording"
- **Recording not found**: Return 404 with `ERR_RECORDING_NOT_FOUND`
- **Recording already active for same ID**: This shouldn't happen (UUIDs are unique)
- **FFmpeg process crash during recording**: Log error, mark recording as `failed`, clean up partial file
- **WebSocket disconnect during recording**: Auto-stop recording (same as manual stop)

### What NOT to Implement

- Do NOT add database persistence for video metadata — use in-memory DashMap like scrcpy recordings (Story 11.2 may add DB if needed)
- Do NOT implement `startVideoRecord` (high quality) — the original frontend method used a different approach. Only implement `startLowQualityScreenRecord` pattern (periodic screenshot + send)
- Do NOT modify existing recording systems (Epic 4 action recording, Epic 6 scrcpy recording)
- Do NOT add authentication or rate limiting — that's Epic 12
- Do NOT implement video streaming/playback in browser — just download. Browser playback is a future enhancement
- Do NOT implement `fixMinicap` — was dead code, minicap is replaced by NIO/scrcpy

### videoReceiver Data Property

The `videoReceiver: null` property was intentionally kept at `remote.js:92` during Story 10.4 cleanup (it's a Vue data property initialization, not a dead method). It will be used by the restored recording methods to store `{ws: WebSocket, key: intervalId}`.

**UPDATE**: `videoReceiver: null` was removed in the Story 10.4 code review. It needs to be RE-ADDED when restoring the recording methods. Add it back at the same location.

### Route Registration Pattern

In `src/main.rs`, the WebSocket route goes near other WS routes:
```rust
.route("/video/convert", web::get().to(routes::video_ws::video_convert_ws))
```

REST routes go in the api_v1 block:
```rust
.route("/api/v1/videos", web::get().to(routes::api_v1::list_videos))
.route("/api/v1/videos/{id}", web::get().to(routes::api_v1::get_video))
.route("/api/v1/videos/{id}/download", web::get().to(routes::api_v1::download_video))
.route("/api/v1/videos/{id}", web::delete().to(routes::api_v1::delete_video))
.route("/api/v1/videos/{id}/stop", web::post().to(routes::api_v1::stop_video))
```

### Test App Macro Pattern

In `tests/test_server.rs`, the `setup_test_app!` macro registers routes. Add video routes after existing api_v1 routes:
```rust
.route("/video/convert", web::get().to(routes::video_ws::video_convert_ws))
.route("/api/v1/videos", web::get().to(routes::api_v1::list_videos))
.route("/api/v1/videos/{id}", web::get().to(routes::api_v1::get_video))
.route("/api/v1/videos/{id}/download", web::get().to(routes::api_v1::download_video))
.route("/api/v1/videos/{id}", web::delete().to(routes::api_v1::delete_video))
.route("/api/v1/videos/{id}/stop", web::post().to(routes::api_v1::stop_video))
```

### Project Structure Notes

- **New**: `src/services/video_service.rs` — video recording lifecycle + FFmpeg process management
- **New**: `src/routes/video_ws.rs` — WebSocket handler for `/video/convert`
- Modified: `src/routes/api_v1.rs` — add video REST endpoint handlers
- Modified: `src/main.rs` — register WS route + REST routes, init VideoService, FFmpeg check
- Modified: `src/state.rs` — add `video_service: VideoService` and `ffmpeg_available: bool` to AppState
- Modified: `src/services/mod.rs` — add `video_service` module
- Modified: `src/routes/mod.rs` — add `video_ws` module
- Modified: `src/models/openapi.rs` — add OpenAPI entries for video endpoints
- Modified: `resources/static/js/remote.js` — restore recording methods
- Modified: `tests/test_server.rs` — add tests + register routes in macro

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 11, Story 11.1]
- [Source: src/services/scrcpy_manager.rs — ScrcpyManager pattern (DashMap state, recording lifecycle)]
- [Source: src/routes/scrcpy_ws.rs — WebSocket relay pattern (actix_ws, binary frames)]
- [Source: src/routes/scrcpy.rs:323-517 — Recording REST endpoints (list, get, download, delete)]
- [Source: src/device/scrcpy.rs:75-88 — Scrcpy config and child process spawning pattern]
- [Source: src/routes/api_v1.rs:29-74 — get_device_client() helper for API v1 handlers]
- [Source: src/state.rs — AppState struct (add video_service field)]
- [Source: git show d74fd69:resources/static/js/remote.js — Original connectImage2VideoWebSocket implementation]
- [Source: _bmad-output/implementation-artifacts/6-4-scrcpy-session-recording.md — H.264 recording patterns]
- [Source: _bmad-output/implementation-artifacts/10-4-legacy-endpoint-cleanup.md — Dead code removal context]
- [Source: docs/project-context.md — Project architecture overview]

### Git Context

Recent commits establish these patterns:
- Story 10.4 removed dead frontend video recording methods (now being restored with server support)
- Story 10.4 code review added upload size limits and path traversal protection
- Epic 6 established scrcpy session recording with DashMap state and broadcast channels
- Epic 4 established action recording with SQLite persistence and playback state machines
- Code reviews consistently catch: missing tests, missing OpenAPI entries, silent error handling

### Previous Story Intelligence (Epic 6, Story 6.4)

Critical lessons to apply:
- **DashMap lifetime**: Clone Arc handles OUT of DashMap before awaiting — never hold DashMap ref across await points
- **Cleanup order**: Stop recording BEFORE aborting producer — prevents data loss
- **Best-effort cleanup**: Don't fail entire cleanup if one step errors
- **File naming**: Use `recordings/{type}_{udid}_{timestamp}.{ext}` pattern
- **Broadcast backpressure**: Use Lagged error handling for slow consumers (not directly applicable here since we use stdin pipe, but relevant pattern)
- **Testing without devices**: Use deterministic 404 paths for integration tests

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Build: 0 errors, 0 new warnings
- Tests: 301 total (90 unit + 190 integration + 12 service + 9 persistence), 0 failures — 8 new tests added (4 integration + 4 unit)

### Completion Notes List

- Task 1: Created `src/services/video_service.rs` with `VideoService` struct — manages JPEG-to-MP4 recording via FFmpeg child processes. Uses `Arc<DashMap>` for recording state (following ScrcpyManager pattern). `start_recording()` spawns FFmpeg with image2pipe stdin, `feed_frame()` writes JPEG bytes to stdin, `stop_recording()` closes stdin and waits for FFmpeg with 30s timeout. Added `check_ffmpeg_available()` startup probe. Added `video_service: VideoService` and `ffmpeg_available: bool` to `AppState`. Registered module in `services/mod.rs`.
- Task 2: Created `src/routes/video_ws.rs` with WebSocket handler at `/video/convert`. Parses query params `fps` (1-30, default 2), `udid` (required), `name` (optional). Rejects with 503 if FFmpeg unavailable. On connect: starts recording and sends JSON `recording_started` message. On binary frame: feeds JPEG to FFmpeg stdin. On disconnect: auto-stops recording. Registered route in `main.rs` and `routes/mod.rs`.
- Task 3: Added 5 REST endpoints in `api_v1.rs` — `list_videos`, `get_video`, `download_video` (with NamedFile + Content-Disposition), `delete_video`, `stop_video`. Extended `error_response()` to map `ERR_RECORDING_NOT_FOUND` → 404, `ERR_RECORDING_ACTIVE`/`ERR_RECORDING_NOT_READY` → 409. Registered all routes in `main.rs`. Added OpenAPI spec entries for all 5 endpoints (4 paths) with video_id parameter.
- Task 4: Added `videoReceiver: null` data property back to `remote.js`. Implemented `connectImage2VideoWebSocket(fps)`, `startScreenRecord()`, `stopScreenRecord()`, `toggleVideoRecord()` methods. Added REC toggle button to `remote.html` control bar with recording indicator SVG.
- Task 5: Added 4 integration tests (list_empty, get_not_found, delete_not_found, stop_not_found). Updated OpenAPI completeness test with 4 new required paths and 5 method assertions. Registered 6 video routes in `setup_test_app!` macro.
- Task 6: Build clean, 301 tests pass, 0 regressions.

### File List

- src/services/video_service.rs (NEW — VideoService with FFmpeg child process management, 4 unit tests)
- src/routes/video_ws.rs (NEW — WebSocket handler for /video/convert JPEG frame ingestion)
- src/routes/api_v1.rs (added 5 video REST handlers + extended error_response mapping)
- src/state.rs (added video_service: VideoService and ffmpeg_available: bool to AppState)
- src/main.rs (registered WS + REST routes, FFmpeg availability check at startup)
- src/services/mod.rs (added video_service module)
- src/routes/mod.rs (added video_ws module)
- src/models/openapi.rs (added OpenAPI spec entries for 4 video endpoint paths)
- resources/static/js/remote.js (added videoReceiver data property + 4 recording methods)
- resources/templates/remote.html (added REC toggle button in control bar)
- tests/test_server.rs (added 4 integration tests, updated OpenAPI test, registered 6 video routes)
