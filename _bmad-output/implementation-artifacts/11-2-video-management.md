# Story 11.2: Video Management

Status: done

## Story

As a **QA tester**,
I want **to list, download, and delete recorded videos with persistent storage and a management UI**,
so that **I can manage test recordings across server restarts**.

## Acceptance Criteria

1. **Given** videos have been recorded **When** I call `GET /api/v1/videos` **Then** all recordings are listed with metadata **And** recordings persist across server restarts (SQLite-backed) **And** results can be filtered by `?udid=X` or `?status=completed`
2. **Given** a completed video recording exists **When** I call `GET /api/v1/videos/{id}/download` **Then** the MP4 file is served with correct Content-Type and Content-Disposition headers
3. **Given** a video recording exists **When** I call `DELETE /api/v1/videos/{id}` **Then** the recording metadata is removed from the database **And** the MP4 file is deleted from disk
4. **Given** the server restarts with existing MP4 files in the recordings directory **When** the server starts **Then** the database is reconciled with files on disk **And** orphaned database entries (missing files) are cleaned up **And** orphaned files (not in database) are registered
5. **Given** a video has been recorded on the remote page **When** recording stops **Then** the remote page shows a link/notification to download the recorded video **And** the video appears in the video management panel

## Tasks / Subtasks

- [x] Task 1: SQLite persistence for video metadata (AC: #1, #3, #4)
  - [x] 1.1 Add `videos` table to SQLite schema in `src/db/sqlite.rs`. Columns: `id TEXT PRIMARY KEY`, `udid TEXT NOT NULL`, `file_path TEXT NOT NULL`, `started_at TEXT NOT NULL`, `stopped_at TEXT`, `frame_count INTEGER DEFAULT 0`, `fps INTEGER DEFAULT 2`, `status TEXT DEFAULT 'recording'`, `duration_ms INTEGER`, `file_size INTEGER`, `device_name TEXT`. Create table in `create_tables()` method alongside existing tables.
  - [x] 1.2 Add video CRUD methods to Database in `src/db/sqlite.rs`:
    - `insert_video(info: &VideoRecordingInfo) -> Result<()>` — insert new record
    - `update_video(info: &VideoRecordingInfo) -> Result<()>` — update existing record (for stop/finalize)
    - `get_video(id: &str) -> Result<Option<VideoRecordingInfo>>` — single lookup
    - `list_videos(udid: Option<&str>, status: Option<&str>) -> Result<Vec<VideoRecordingInfo>>` — list with optional filters
    - `delete_video(id: &str) -> Result<bool>` — delete by ID, return whether row existed
    Follow the existing `insert_device`, `query_info_by_udid` patterns in sqlite.rs.
  - [x] 1.3 Update `VideoService` in `src/services/video_service.rs` to accept `Database` in constructor:
    - Change `VideoService::new()` → `VideoService::new(db: Database)`
    - Store `db: Database` field alongside existing DashMaps
    - In `start_recording()`: insert video metadata to SQLite after DashMap insert
    - In `stop_recording()`: update SQLite record after metadata is finalized
    - In `delete_recording()`: delete from SQLite alongside DashMap removal
    - In `list_recordings()`: read from SQLite instead of DashMap for completed recordings, merge with active DashMap entries
    - In `get_recording()`: check DashMap first (for active), fall back to SQLite
    - Keep DashMap for active recordings (FFmpeg handles can't go in SQLite)
  - [x] 1.4 Update `AppState::new()` in `src/state.rs` — pass `db.clone()` to `VideoService::new(db)`
  - [x] 1.5 Update `main.rs` — VideoService now takes db parameter

- [x] Task 2: Startup recovery — reconcile database with filesystem (AC: #4)
  - [x] 2.1 Add `VideoService::recover_on_startup(&self) -> Result<(), String>` method:
    - Read all videos from SQLite with status != "completed" and != "failed" → mark as "failed" (stale active recordings from previous run)
    - Scan `recordings/video_*.mp4` files in the `recordings/` directory
    - For each file: check if a matching record exists in SQLite (by file_path)
    - If file exists without DB record: create a "recovered" record with metadata extracted from filename (udid, timestamp) and file size from fs::metadata
    - If DB record exists without file: mark status as "failed" and set file_size to None
    - Log summary: "Recovered X videos, cleaned Y orphaned records, registered Z orphaned files"
  - [x] 2.2 Call `video_service.recover_on_startup()` in `main.rs` after FFmpeg check, before starting the HTTP server

- [x] Task 3: Enhanced list API with query filters (AC: #1)
  - [x] 3.1 Update `list_videos()` handler in `src/routes/api_v1.rs`:
    - Parse optional query params: `udid` (filter by device), `status` (filter by status)
    - Pass filters to `video_service.list_recordings(udid, status)`
    - Keep the `success_response(json!(recordings))` pattern
  - [x] 3.2 Update OpenAPI spec in `src/models/openapi.rs` — add query parameter documentation to `GET /api/v1/videos` (two optional query params: `udid` and `status`)

- [x] Task 4: Frontend video management on remote page (AC: #5)
  - [x] 4.1 Add video management panel to `resources/templates/remote.html`:
    - Add a "Videos" section/drawer below the control buttons (collapsible)
    - Show list of video recordings for the current device (fetched from `GET /api/v1/videos?udid={deviceUdid}`)
    - Each entry shows: recording date, duration, file size, status
    - Download button: links to `/api/v1/videos/{id}/download`
    - Delete button: calls `DELETE /api/v1/videos/{id}` with confirmation
    - Auto-refresh list after recording stops
  - [x] 4.2 Add video management methods to `resources/static/js/remote.js`:
    - `loadVideoRecordings()` — fetch `GET /api/v1/videos?udid={this.deviceUdid}`, populate `this.videoRecordings` array
    - `downloadVideo(id)` — open `/api/v1/videos/{id}/download` in new tab
    - `deleteVideo(id)` — confirm + call `DELETE /api/v1/videos/{id}`, refresh list
    - Add `videoRecordings: []` to Vue data properties
    - Call `loadVideoRecordings()` on mount and after `stopScreenRecord()`
  - [x] 4.3 Update `stopScreenRecord()` in `remote.js` — after stopping, show a notification with download link and call `loadVideoRecordings()` to refresh the list. Listen for the `recording_stopped` WebSocket message to get the recording ID for the download link.

- [x] Task 5: Integration tests (AC: #1-#5)
  - [x] 5.1 Add `test_video_list_filter_by_udid` — insert test video records into SQLite, verify `GET /api/v1/videos?udid=test-device` filters correctly
  - [x] 5.2 Add `test_video_list_filter_by_status` — verify `GET /api/v1/videos?status=completed` filters correctly
  - [x] 5.3 Add `test_video_persistence_across_service_restart` — create VideoService, insert video via SQLite, create new VideoService instance with same db, verify video is still retrievable
  - [x] 5.4 Add video DB methods unit tests in `src/db/sqlite.rs`: `test_video_insert_and_get`, `test_video_update`, `test_video_delete`, `test_video_list_with_filters`
  - [x] 5.5 Update OpenAPI completeness test — verify query parameter documentation for GET /api/v1/videos

- [x] Task 6: Regression testing (AC: #1-#5)
  - [x] 6.1 Build succeeds — 0 new warnings
  - [x] 6.2 All existing tests pass (301 existing + new tests)
  - [x] 6.3 No new regressions introduced

## Dev Notes

### Scope — Video Metadata Persistence + Management UI

This story adds **persistence and UI** to the video recording infrastructure created in Story 11.1. The key additions:

| What | Story 11.1 (Done) | Story 11.2 (This Story) |
|------|-------------------|-------------------------|
| **Storage** | In-memory DashMap only | SQLite + DashMap (active only) |
| **Persistence** | Lost on restart | Survives restarts |
| **API** | 5 REST endpoints (basic CRUD) | Enhanced with query filters |
| **UI** | REC button only | Video list panel with download/delete |
| **Recovery** | None | Startup reconciliation |

### Architecture — Dual-Store Pattern

Keep the **DashMap for active recordings** (FFmpeg handles can't go in SQLite) and use **SQLite for completed/failed recordings**. On read operations, merge both sources:

```
list_recordings(filters) → {
    active_recordings = DashMap entries (status: recording/finalizing)
    completed_recordings = SQLite query (status: completed/failed, with filters)
    return merge(active_recordings, completed_recordings)
}

get_recording(id) → {
    if DashMap.contains(id) → return DashMap entry
    else → return SQLite query
}
```

### SQLite Schema — Follow Existing Patterns

The `src/db/sqlite.rs` already has tables for `devices`, `connection_events`, `device_tags`, `products`, `providers`, `recordings`, `recorded_actions`. Add `videos` table following the same pattern:

```sql
CREATE TABLE IF NOT EXISTS videos (
    id TEXT PRIMARY KEY,
    udid TEXT NOT NULL,
    file_path TEXT NOT NULL,
    started_at TEXT NOT NULL,
    stopped_at TEXT,
    frame_count INTEGER DEFAULT 0,
    fps INTEGER DEFAULT 2,
    status TEXT DEFAULT 'recording',
    duration_ms INTEGER,
    file_size INTEGER,
    device_name TEXT
);
```

Use `sqlx::query!` or `sqlx::query_as!` macros following the existing `insert_device` and `query_info_by_udid` patterns. The Database struct already wraps `SqlitePool`.

### VideoService Changes — Minimal Refactor

The VideoService constructor changes from `new()` to `new(db: Database)`. This affects:
- `src/state.rs` line 144: `video_service: VideoService::new()` → `video_service: VideoService::new(db.clone())`
- Unit tests in `video_service.rs` that call `VideoService::new()` — need to pass a test database

For unit tests, use the same temp database pattern from `tests/test_server.rs`:
```rust
let tmp = tempdir().unwrap();
let db = Database::new(tmp.path().to_str().unwrap(), "test.db").await.unwrap();
let service = VideoService::new(db);
```

### Frontend Pattern — Follow index.html Recording UI

The `resources/templates/index.html` already has a recording management pattern:
- `availableRecordings` Vue data array
- Modal to list and select recordings
- Fetch from `/api/recordings` API

For the remote page, implement a simpler collapsible panel (not a modal) since the user is focused on a single device:

```html
<!-- Video Recordings Panel -->
<div class="video-recordings-panel" v-if="videoRecordings.length > 0">
  <h4 @click="showVideoPanel = !showVideoPanel">
    Videos ({{ videoRecordings.length }})
  </h4>
  <div v-if="showVideoPanel">
    <div v-for="video in videoRecordings" :key="video.id" class="video-entry">
      <span>{{ video.started_at }} — {{ formatDuration(video.duration_ms) }}</span>
      <a :href="'/api/v1/videos/' + video.id + '/download'" target="_blank">Download</a>
      <button @click="deleteVideo(video.id)">Delete</button>
    </div>
  </div>
</div>
```

### Startup Recovery Logic

On startup, before the HTTP server starts:

```
1. Query SQLite for videos with status IN ('recording', 'finalizing')
   → Update all to status = 'failed' (they were interrupted by shutdown)

2. Scan recordings/ directory for video_*.mp4 files
   → For each file, check if file_path exists in SQLite
   → If NOT in DB: parse filename for udid/timestamp, create DB record with status='recovered'

3. Query SQLite for all records
   → For each record, check if file_path exists on disk
   → If file missing: update status = 'failed', set file_size = None

4. Log summary
```

### Query Filter Implementation

In `list_videos()` handler, parse query params from `HttpRequest`:

```rust
pub async fn list_videos(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> HttpResponse {
    let query = web::Query::<HashMap<String, String>>::from_query(req.query_string())
        .unwrap_or_else(|_| web::Query(HashMap::new()));
    let udid = query.get("udid").map(|s| s.as_str());
    let status = query.get("status").map(|s| s.as_str());
    let recordings = state.video_service.list_recordings(udid, status);
    success_response(json!(recordings))
}
```

This requires adding `use std::collections::HashMap;` (already imported in api_v1.rs).

### Error Handling — Extend Existing Patterns

No new error codes needed. Existing mappings cover all cases:
- `ERR_RECORDING_NOT_FOUND` → 404
- `ERR_RECORDING_ACTIVE` → 409
- `ERR_FILE_NOT_FOUND` → 404

### What NOT to Implement

- Do NOT add video streaming/playback in browser — just download links
- Do NOT add video transcoding or quality conversion
- Do NOT add authentication — that's Epic 12
- Do NOT add retention policies or automatic cleanup — future enhancement
- Do NOT modify the WebSocket recording endpoint or FFmpeg pipeline — that's Story 11.1 territory
- Do NOT add pagination — video counts are expected to be small (<1000); simple list is sufficient

### videoRecordings vs videoReceiver

These are different Vue data properties:
- `videoReceiver: null` — existing (from 11.1), holds active WebSocket + interval for in-progress recording
- `videoRecordings: []` — NEW (this story), holds list of completed video recordings fetched from API

### Route Changes

No new routes needed. The existing 5 video routes handle all operations. Changes are to the `list_videos` handler signature (add `HttpRequest` parameter for query parsing) and to the VideoService methods.

### Test App Macro

The `setup_test_app!` macro in `tests/test_server.rs` already registers all video routes. No changes needed for route registration. Only add new test functions.

### Previous Story Intelligence (Story 11.1)

Critical learnings to apply:
- **DashMap lifetime**: Clone Arc handles OUT of DashMap before awaiting — never hold DashMap ref across await points
- **Cleanup order**: Stop recording BEFORE aborting producer — prevents data loss
- **Best-effort cleanup**: Don't fail entire delete if one step errors
- **File naming**: Use `recordings/video_{udid}_{timestamp}.mp4` pattern
- **Error response mapping**: Must map error codes in `error_response()` — forgetting causes 500s instead of proper status
- **URL decoding**: Query params must be URL-decoded (fixed in 11.1 code review)
- **FFmpeg stderr**: Now captured for diagnostics (fixed in 11.1 code review)

### Project Structure Notes

- Modified: `src/db/sqlite.rs` — add `videos` table + CRUD methods
- Modified: `src/services/video_service.rs` — accept Database, add persistence + recovery
- Modified: `src/routes/api_v1.rs` — add query filters to list_videos handler
- Modified: `src/models/openapi.rs` — add query param docs to GET /api/v1/videos
- Modified: `src/state.rs` — pass db to VideoService constructor
- Modified: `src/main.rs` — call recovery on startup
- Modified: `resources/static/js/remote.js` — add video list management methods
- Modified: `resources/templates/remote.html` — add video management panel
- Modified: `tests/test_server.rs` — add filter + persistence tests

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Story 11.2 — AC definition]
- [Source: _bmad-output/implementation-artifacts/11-1-screen-to-video-recording.md — Previous story with deferred persistence note]
- [Source: src/services/video_service.rs — VideoService current implementation]
- [Source: src/db/sqlite.rs — Database patterns (insert_device, query_info_by_udid)]
- [Source: src/routes/api_v1.rs:1442-1525 — Existing video REST handlers]
- [Source: src/state.rs:113-115 — VideoService in AppState]
- [Source: resources/templates/index.html:420-856 — Action recording management UI pattern]
- [Source: resources/static/js/remote.js:1411-1471 — Video recording methods from Story 11.1]
- [Source: docs/project-context.md — Project architecture overview]

### Git Context

Recent commits establish these patterns:
- Story 11.1 created the video recording pipeline with in-memory state
- Story 11.1 code review fixed: DashMap guard across await, FFmpeg stderr suppression, URL decoding, dead code, duration cast
- Story 10.4 established upload size limits and path traversal protection
- Database patterns are well-established across Epics 4, 8, 9 (SQLite CRUD)

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Tera template `{% raw %}` needed for Vue.js `{{ }}` expressions in `remote.html`
- `start_recording()` changed from sync to async to support SQLite persistence
- `db` ownership in `state.rs` — cloned before struct literal to avoid move-after-use
- Code review fix: `resp.success` → `resp.status === 'success'` in JS (videos never loaded)
- Code review fix: N+1 query in `recover_on_startup` — `list_videos` moved outside loop into `HashSet`
- Code review fix: `download_video` now allows `status: "recovered"` alongside `"completed"`
- Code review fix: `test_db()` now returns `TempDir` to prevent premature cleanup

### Completion Notes List

- Task 1: SQLite `videos` table + 7 CRUD methods + VideoService constructor accepts Database + persistence in start/stop/delete/list/get
- Task 2: `recover_on_startup()` marks stale recordings as failed, registers orphaned files, marks missing-file records as failed
- Task 3: `list_videos` handler accepts `?udid=` and `?status=` query filters, OpenAPI spec updated with query param docs
- Task 4: Video management panel on remote page with collapsible list, download links, delete buttons, auto-refresh after recording stops
- Task 5: 4 integration tests (udid filter, status filter, persistence across restart, OpenAPI query params) + 4 SQLite unit tests
- Task 6: 309 total tests pass (94 unit + 194 integration + 9 service + 3 bin + 9 services), 0 failures, 0 new warnings

### File List

- `src/db/sqlite.rs` — Added `videos` table + 7 CRUD methods + 4 unit tests
- `src/services/video_service.rs` — Added `Database` field, async persistence, `recover_on_startup()`, updated tests
- `src/state.rs` — Pass `db.clone()` to `VideoService::new()`
- `src/main.rs` — Call `recover_on_startup()` after FFmpeg check
- `src/routes/api_v1.rs` — Added query filter parsing to `list_videos`, `.await` on async methods
- `src/routes/video_ws.rs` — `.await` on `start_recording()`
- `src/models/openapi.rs` — Added `udid` and `status` query parameter docs to GET /api/v1/videos
- `resources/templates/remote.html` — Added video recordings panel with `{% raw %}` Tera escaping
- `resources/static/js/remote.js` — Added `videoRecordings`, `showVideoPanel`, `loadVideoRecordings`, `deleteVideo`, `formatDuration`, `formatFileSize`
- `tests/test_server.rs` — 4 new integration tests for Story 11-2
