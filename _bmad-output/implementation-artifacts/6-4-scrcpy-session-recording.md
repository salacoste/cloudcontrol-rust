# Story 6.4: Scrcpy Session Recording

Status: done

## Story

As a **QA Engineer**,
I want to record scrcpy sessions for later review,
so that I can analyze issues that occurred during testing.

## Acceptance Criteria

### AC1: Start Recording Scrcpy Session
```gherkin
Scenario: Start recording scrcpy session
  Given a scrcpy session is active
  When I enable recording via POST /scrcpy/{udid}/recording/start
  Then the H.264 stream is saved to a file
  And the file is named with UDID and timestamp
  And recording continues until stopped
  And a 200 response returns recording metadata (id, file_path, started_at)
```

### AC2: Stop Recording and Access File
```gherkin
Scenario: Stop recording and access file
  Given recording is in progress
  When I stop recording via POST /scrcpy/{udid}/recording/stop
  Then the file is finalized
  And the file is accessible via download endpoint
  And file metadata (duration, size, frame_count) is returned
```

### AC3: List Recorded Sessions
```gherkin
Scenario: List recorded sessions
  Given multiple sessions were recorded
  When I request GET /scrcpy/recordings
  Then a list of recordings is returned
  And each shows: id, udid, started_at, duration, file_size, status
```

### AC4: Delete Recorded Session
```gherkin
Scenario: Delete recorded session
  Given a recording exists
  When I request DELETE /scrcpy/recordings/{id}
  Then the file is deleted from disk
  And it no longer appears in the list
```

## Requirements Coverage

- **FR38**: Users can record scrcpy sessions for later review
- **NFR17**: scrcpy video stream latency <100ms (recording must NOT add latency to live viewers)

## Tasks / Subtasks

- [x] Task 1: Recording state infrastructure in ScrcpyManager (AC: #1, #2)
  - [x] 1.1 Define `ScrcpyRecordingInfo` struct (id, udid, file_path, started_at, stopped_at, duration_ms, file_size, frame_count, status)
  - [x] 1.2 Add `recordings: Arc<DashMap<String, ScrcpyRecordingInfo>>` to `ScrcpyManager`
  - [x] 1.3 Add `recording_task: Option<JoinHandle<()>>` and `recording_id: Option<String>` to `ScrcpySessionEntry`
  - [x] 1.4 Create `recordings/` directory on startup if not exists
  - [x] 1.5 Implement `start_recording(udid) -> Result<ScrcpyRecordingInfo, String>`
  - [x] 1.6 Implement `stop_recording(udid) -> Result<ScrcpyRecordingInfo, String>`
  - [x] 1.7 Implement `list_recordings() -> Vec<ScrcpyRecordingInfo>`
  - [x] 1.8 Implement `get_recording(id) -> Option<ScrcpyRecordingInfo>`
  - [x] 1.9 Implement `delete_recording(id) -> Result<(), String>`
  - [x] 1.10 On `stop_session()`: auto-stop any active recording before cleanup

- [x] Task 2: H.264 file writer (recording consumer task) (AC: #1, #2)
  - [x] 2.1 Subscribe to broadcast channel via `subscribe_video(udid)`
  - [x] 2.2 Create output file: `recordings/{udid}_{timestamp}.h264`
  - [x] 2.3 Write H.264 Annex B format: for each BroadcastFrame, extract NAL data (bytes 5+), prepend start code `[0x00, 0x00, 0x00, 0x01]`, write to file
  - [x] 2.4 Ensure config frames (SPS/PPS, flag bit0=1) are written first and on every keyframe
  - [x] 2.5 Handle `RecvError::Lagged(n)` — log, skip, continue (same as WS viewer)
  - [x] 2.6 Handle `RecvError::Closed` — finalize file, update recording metadata
  - [x] 2.7 Use `tokio::io::BufWriter<tokio::fs::File>` for efficient disk I/O
  - [x] 2.8 Track frame_count and file position for metadata updates on stop

- [x] Task 3: REST endpoints for recording control (AC: #1, #2, #3, #4)
  - [x] 3.1 `POST /scrcpy/{udid}/recording/start` — validate session exists, reject if already recording, start recording
  - [x] 3.2 `POST /scrcpy/{udid}/recording/stop` — validate recording active, stop recording, return metadata
  - [x] 3.3 `GET /scrcpy/recordings` — list all recordings (completed and in-progress)
  - [x] 3.4 `GET /scrcpy/recordings/{id}` — get single recording metadata
  - [x] 3.5 `GET /scrcpy/recordings/{id}/download` — serve file with `Content-Disposition: attachment`
  - [x] 3.6 `DELETE /scrcpy/recordings/{id}` — delete file and remove from tracking

- [x] Task 4: Route registration and module integration (AC: #1)
  - [x] 4.1 Add recording endpoint functions to `src/routes/scrcpy.rs` (same file as other scrcpy REST routes)
  - [x] 4.2 Register routes in `src/main.rs` under the scrcpy scope
  - [x] 4.3 Verify no route conflicts with existing endpoints

- [x] Task 5: Tests (AC: #1, #2, #3, #4)
  - [x] 5.1 Unit test: `ScrcpyRecordingInfo` serialization
  - [x] 5.2 Unit test: `start_recording` returns error for nonexistent session
  - [x] 5.3 Unit test: `stop_recording` returns error for nonexistent session
  - [x] 5.4 Unit test: `delete_recording` returns error for nonexistent recording
  - [x] 5.5 Unit test: `list_recordings` returns empty when no recordings
  - [x] 5.6 Unit test: H.264 Annex B start code prepending logic
  - [x] 5.7 Integration test: POST /scrcpy/{udid}/recording/start → 404 when no session
  - [x] 5.8 Integration test: POST /scrcpy/{udid}/recording/stop → 404 when no session
  - [x] 5.9 Integration test: GET /scrcpy/recordings → empty list
  - [x] 5.10 Integration test: DELETE /scrcpy/recordings/{id} → 404 for nonexistent
  - [x] 5.11 Verify all existing tests still pass (no regressions)

## Dev Notes

### Architecture: Recording as Another Broadcast Consumer

Recording uses the SAME broadcast channel from Story 6-3. The recording task is another consumer alongside WS viewers:

```
Device → ScrcpySession.read_frame() → Producer Task → broadcast::Sender
                                                          ↓
                                           broadcast::Receiver → Viewer 1 WS
                                           broadcast::Receiver → Viewer 2 WS
                                           broadcast::Receiver → Recording Task → File I/O
```

**Key design decisions:**
1. **Recording does NOT create its own frame reader** — it subscribes to the existing broadcast channel via `subscribe_video(udid)`, exactly like WS viewers do.
2. **One recording per session max** — prevents duplicate file writes. Use `recording_id` field in `ScrcpySessionEntry` to track.
3. **Recording is independent of viewers** — can record with zero WS viewers connected.
4. **Session stop auto-stops recording** — when `stop_session()` is called, any active recording is finalized first (broadcast sender drop → `RecvError::Closed` in recording task).
5. **Recording must NOT add latency** — broadcast send is O(1) regardless of consumer count. File I/O is async and buffered.

### H.264 File Format: Annex B Bitstream

Save raw H.264 NAL units in **Annex B format** (the standard raw bitstream format):

```
[Start Code: 0x00 0x00 0x00 0x01] [NAL Unit 1 (SPS)]
[Start Code: 0x00 0x00 0x00 0x01] [NAL Unit 2 (PPS)]
[Start Code: 0x00 0x00 0x00 0x01] [NAL Unit 3 (IDR frame)]
[Start Code: 0x00 0x00 0x00 0x01] [NAL Unit 4 (P-frame)]
...
```

**Why raw H.264 (not MP4):**
- No additional crate dependencies needed
- Playable by VLC, mpv, ffplay without conversion
- Simpler and more reliable than MP4 muxing
- Can be converted to MP4 post-hoc with `ffmpeg -i file.h264 -c copy file.mp4` (zero re-encode)
- Avoids needing PTS tracking for MP4 sample tables

**Extracting NAL data from BroadcastFrame:**
```rust
// BroadcastFrame.data format: flags(1) + size(4 BE) + NAL_data(N)
let nal_data = &frame.data[5..];  // Skip 5-byte header
// Write: [0x00, 0x00, 0x00, 0x01] + nal_data
```

### Recording State Design

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ScrcpyRecordingInfo {
    pub id: String,              // UUID
    pub udid: String,
    pub file_path: String,       // Relative path: "recordings/{udid}_{timestamp}.h264"
    pub started_at: String,      // RFC3339
    pub stopped_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub file_size: Option<u64>,
    pub frame_count: u64,
    pub status: String,          // "recording" | "completed" | "error"
}
```

Track recordings in `ScrcpyManager` with `Arc<DashMap<String, ScrcpyRecordingInfo>>`. This is in-memory tracking (not persisted to DB) — acceptable for session recordings that are transient.

### Recording Consumer Task Pattern

```rust
// Pseudo-code for recording task
async fn recording_task(
    mut video_rx: broadcast::Receiver<BroadcastFrame>,
    file_path: PathBuf,
    recording_id: String,
    recordings: Arc<DashMap<String, ScrcpyRecordingInfo>>,
) {
    let file = tokio::fs::File::create(&file_path).await?;
    let mut writer = tokio::io::BufWriter::new(file);
    let start_code: &[u8] = &[0x00, 0x00, 0x00, 0x01];
    let mut frame_count: u64 = 0;

    loop {
        match video_rx.recv().await {
            Ok(frame) => {
                let nal_data = &frame.data[5..]; // Skip flags(1) + size(4)
                writer.write_all(start_code).await?;
                writer.write_all(nal_data).await?;
                frame_count += 1;
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("Recording lagged, skipped {} frames", n);
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                break; // Session stopped
            }
        }
    }

    writer.flush().await?;
    // Update recording metadata with final stats
}
```

### REST Endpoint Patterns

Follow the exact same patterns as existing scrcpy routes in `src/routes/scrcpy.rs`:

**Start recording:**
```json
POST /scrcpy/{udid}/recording/start
Response 200: {"status":"success","recording":{"id":"...","udid":"...","file_path":"...","started_at":"...","status":"recording"}}
Response 404: {"status":"error","error":"ERR_SESSION_NOT_FOUND","message":"..."}
Response 409: {"status":"error","error":"ERR_RECORDING_ALREADY_ACTIVE","message":"..."}
```

**Stop recording:**
```json
POST /scrcpy/{udid}/recording/stop
Response 200: {"status":"success","recording":{"id":"...","duration_ms":12345,"file_size":67890,"frame_count":1234,"status":"completed"}}
Response 404: {"status":"error","error":"ERR_NO_ACTIVE_RECORDING","message":"..."}
```

**List recordings:**
```json
GET /scrcpy/recordings
Response 200: {"recordings":[...]}
```

**Download:**
```
GET /scrcpy/recordings/{id}/download
Response 200: binary file with Content-Disposition: attachment; filename="device_2026-03-09T12-00-00.h264"
Response 404: {"status":"error","error":"ERR_RECORDING_NOT_FOUND","message":"..."}
```

**Delete:**
```json
DELETE /scrcpy/recordings/{id}
Response 200: {"status":"success","message":"Recording deleted"}
Response 404: {"status":"error","error":"ERR_RECORDING_NOT_FOUND","message":"..."}
```

### Error Handling Patterns

| Scenario | Error Code | HTTP | Action |
|----------|-----------|------|--------|
| No active session | ERR_SESSION_NOT_FOUND | 404 | Return error JSON |
| Already recording | ERR_RECORDING_ALREADY_ACTIVE | 409 | Return error JSON |
| No active recording on stop | ERR_NO_ACTIVE_RECORDING | 404 | Return error JSON |
| Recording not found | ERR_RECORDING_NOT_FOUND | 404 | Return error JSON |
| File I/O error during recording | — | — | Log error, update status to "error", break consumer loop |
| Session stops while recording | — | — | RecvError::Closed → finalize file normally |
| File not found on download | ERR_RECORDING_NOT_FOUND | 404 | Return error JSON |
| File not found on delete | ERR_RECORDING_NOT_FOUND | 404 | Return error JSON |

### File Serving for Downloads

Use `actix_files::NamedFile` for efficient file serving:
```rust
use actix_files::NamedFile;
// In download handler:
let file = NamedFile::open(&recording.file_path)?
    .set_content_disposition(actix_web::http::header::ContentDisposition {
        disposition: actix_web::http::header::DispositionType::Attachment,
        parameters: vec![actix_web::http::header::DispositionParam::Filename(filename)],
    });
```

Check if `actix-files` is already a dependency. If not, add it to Cargo.toml.

### Session Stop + Recording Cleanup

In `stop_session()`, recording must be stopped BEFORE aborting the producer task:

```rust
pub async fn stop_session(&self, udid: &str) -> Result<(), String> {
    // 1. Remove session from map
    // 2. If recording is active: abort recording task, finalize metadata
    // 3. Abort producer task (drops sender → closes any remaining receivers)
    // 4. Best-effort stream/process cleanup (existing pattern)
}
```

Order matters: if producer is aborted first, the recording task gets `RecvError::Closed` and exits — but we want controlled finalization with proper metadata update.

### Anti-Patterns to Avoid

1. **DO NOT create a new frame reader for recording** — use the existing broadcast channel
2. **DO NOT hold DashMap ref across await** — clone Arc/info out first (Story 6-2 learning)
3. **DO NOT use synchronous file I/O** — use `tokio::fs` and `BufWriter` for async I/O
4. **DO NOT block the broadcast channel** — if file I/O is slow, frames will be skipped via `Lagged` (acceptable)
5. **DO NOT persist recordings to SQLite** — in-memory DashMap is sufficient for this transient data
6. **DO NOT modify BroadcastFrame** — the existing format works; extract NAL data by skipping 5-byte header
7. **DO NOT start/stop scrcpy sessions from recording endpoints** — recording is a sub-feature of an existing session

### Project Structure Notes

Files to modify/create:
```
src/services/scrcpy_manager.rs  — Add ScrcpyRecordingInfo, recording DashMap, start/stop/list/get/delete recording methods, recording consumer task
src/routes/scrcpy.rs            — Add REST endpoints for recording start/stop/list/get/download/delete
src/main.rs                     — Register new recording routes
tests/test_server.rs            — Add integration tests for recording endpoints
```

No new files needed — extend existing scrcpy modules.

### Dependencies Check

**Likely already available:**
- `tokio::fs` — included in `tokio = { features = ["full"] }`
- `tokio::io::BufWriter` — included in tokio
- `uuid` — already in Cargo.toml
- `chrono` — already in Cargo.toml
- `serde_json` — already in Cargo.toml
- `DashMap` — already in Cargo.toml

**May need to add:**
- `actix-files` — for `NamedFile` file serving. Check Cargo.toml; if missing, add `actix-files = "0.6"`.

### References

- [Source: src/services/scrcpy_manager.rs] — ScrcpyManager, BroadcastFrame, subscribe_video(), ScrcpySessionEntry
- [Source: src/routes/scrcpy.rs] — Existing REST endpoints pattern, resolve_device_serial(), error handling
- [Source: src/routes/scrcpy_ws.rs] — Broadcast consumer pattern (video_rx.recv() loop)
- [Source: src/device/scrcpy.rs] — ScrcpyFrame, read_frame(), binary frame format
- [Source: _bmad-output/implementation-artifacts/6-3-h264-websocket-relay.md] — Previous story: broadcast architecture, code review fixes
- [Source: _bmad-output/implementation-artifacts/6-2-scrcpy-device-control.md] — DashMap ref lifetime, TOCTOU prevention
- [Source: _bmad-output/implementation-artifacts/6-1-scrcpy-session-management.md] — Session lifecycle, race prevention, cleanup patterns
- [Source: _bmad-output/planning-artifacts/epics-stories.md] — Epic 6, Story 6-4 BDD scenarios
- [Source: _bmad-output/planning-artifacts/architecture.md] — NFR17, scrcpy architecture, file structure

### Previous Story Intelligence (Stories 6-1, 6-2, 6-3)

**Key learnings to apply:**
- Async closure lifetime issue: Don't use `FnOnce(&mut T) -> Future`. Return `Arc<Mutex<>>` and let caller lock. (6-2)
- TOCTOU prevention: Use `get_session_with_info()` for atomic info+handle retrieval. (6-2)
- Safe u16 conversion: Use `u16::try_from().unwrap_or(u16::MAX)` not `as u16`. (6-2)
- DashMap ref lifetime: Clone Arc handles out of DashMap ref before awaiting. (6-2)
- Zero-copy frames: Use `frame.data.clone()` (O(1) Bytes ref-count), NOT `.to_vec()`. (6-3)
- Always close WS on broadcast close: Send error text + `session.close(None)`. (6-3)
- Producer mutex contention: Known limitation — inherent to shared ScrcpySession design. (6-3)
- Race condition prevention: Use `starting` sentinel between check and insert. (6-1)
- Best-effort cleanup: Per-operation error logging, don't fail entire cleanup. (6-1)
- Integration tests: Test deterministic error paths (404 no session) without real devices. (6-1, 6-2)

**Test count after 6-3:** 240 passing, 0 failures

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

None — clean implementation with no blocking issues.

### Completion Notes List

- Implemented `ScrcpyRecordingInfo` struct with full metadata tracking (id, udid, file_path, timestamps, duration, file_size, frame_count, status)
- Added `recordings: Arc<DashMap<String, ScrcpyRecordingInfo>>` to `ScrcpyManager` for in-memory recording tracking
- Added `recording_task` and `recording_id` fields to `ScrcpySessionEntry` for per-session recording state
- `recordings/` directory created in `ScrcpyManager::new()` via `std::fs::create_dir_all`
- `start_recording()` subscribes to existing broadcast channel (same as WS viewers), spawns recording consumer task
- Recording consumer writes H.264 Annex B format: `[0x00, 0x00, 0x00, 0x01]` start code + NAL data extracted from BroadcastFrame (skip 5-byte header)
- `stop_recording()` aborts recording task, updates metadata (status, stopped_at, duration_ms, file_size)
- `stop_session()` auto-stops any active recording before aborting producer task
- `delete_recording()` removes tracking entry and best-effort deletes file from disk
- `get_recording_file_path()` added for download endpoint support
- 6 REST endpoints: start/stop recording, list/get/download/delete recordings
- Download uses `actix_files::NamedFile` with `Content-Disposition: attachment` (no new deps — `actix-files` already in Cargo.toml)
- 8 new unit tests + 6 new integration tests, all 276 tests pass with 0 regressions
- No new crate dependencies required

### File List

- `src/services/scrcpy_manager.rs` — Added ScrcpyRecordingInfo, recording DashMap, recording_task/recording_id in ScrcpySessionEntry, start/stop/list/get/delete recording methods, recording_consumer_task, get_recording_file_path, auto-stop in stop_session, 8 new unit tests
- `src/routes/scrcpy.rs` — Added 6 REST endpoint handlers: start_scrcpy_recording, stop_scrcpy_recording, list_scrcpy_recordings, get_scrcpy_recording, download_scrcpy_recording, delete_scrcpy_recording
- `src/main.rs` — Registered 6 new recording routes under scrcpy scope
- `tests/test_server.rs` — Added 6 recording routes to setup_test_app! macro, 6 new integration tests
