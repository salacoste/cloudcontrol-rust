# Story 4.4: Recording Session Management

Status: in-progress

## Story

As a **QA Engineer**, I want to control recording sessions with start/stop/pause, so that I can manage the recording process.

## Acceptance Criteria

1. **Pause and resume recording**
   - Given recording is active with 3 actions recorded
   - When I click "Pause Recording"
   - Then no new actions are recorded
   - And the indicator changes to "Paused"
   - When I click "Resume Recording"
   - Then new actions are recorded again
   - And actions after pause are appended to the same recording

2. **Delete recorded action**
   - Given a recording has 5 actions
   - When I delete action 3
   - Then the recording has 4 actions
   - And remaining actions are renumbered

3. **Edit recorded action**
   - Given a recorded tap at (100, 200)
   - When I edit it to (150, 250)
   - Then the action is updated
   - And the recording reflects the change

4. **Cancel recording without saving**
   - Given recording is active with 5 actions
   - When I click "Cancel Recording"
   - Then a confirmation dialog appears
   - And when confirmed, the recording is discarded
   - And no recording is saved

## Tasks / Subtasks

- [ ] Task 1: Add pause/resume functionality (AC: 1)
  - [ ] Add `paused_recordings` state to RecordingState
  - [ ] Add `pause_recording` method to RecordingService
  - [ ] Add `resume_recording` method to RecordingService
  - [ ] Add `is_paused` method to RecordingService
  - [ ] Modify `record_action` to check paused state

- [ ] Task 2: Add action management API endpoints (AC: 2, 3)
  - [ ] PUT /api/recordings/{id}/actions/{action_id} - Edit action
  - [ ] DELETE /api/recordings/{id}/actions/{action_id} - Delete action
  - [ ] Add `update_action` method to RecordingService
  - [ ] Add `delete_action` method to RecordingService with renumbering

- [ ] Task 3: Add cancel recording functionality (AC: 4)
  - [ ] POST /api/recordings/{id}/cancel - Cancel without saving
  - [ ] Add `cancel_recording` method to RecordingService
  - [ ] Delete recording and all actions from database

- [ ] Task 4: Add API endpoints for pause/resume (AC: 1)
  - [ ] POST /api/recordings/{id}/pause - Pause recording
  - [ ] POST /api/recordings/{id}/resume - Resume recording
  - [ ] GET /api/recordings/{id}/status - Get recording status

- [ ] Task 5: Add UI controls (AC: 1, 4)
  - [ ] Add Pause/Resume button with state toggle
  - [ ] Add Cancel Recording button with confirmation
  - [ ] Add paused indicator styling

- [ ] Task 6: Add E2E tests
  - [ ] Test pause/resume recording
  - [ ] Test delete action with renumbering
  - [ ] Test edit action
  - [ ] Test cancel recording without saving

## Dev Notes

### Architecture Context

This story extends Story 4-3 (Action Recording System) with session management features. The recording system needs to:

1. **Track paused state** per device alongside active recording state
2. **Support action editing** with coordinate/parameter modification
3. **Support action deletion** with automatic renumbering
4. **Support cancel** to discard recording without saving

### Existing Patterns

From Story 4-3:
- `RecordingState` uses `Arc<RwLock<HashMap>>` for thread-safe state
- `RecordingService` provides CRUD operations
- Control endpoints check recording state before capturing actions

### Data Model Extensions

```rust
// RecordingStateInner - add paused tracking
pub struct RecordingStateInner {
    active_recordings: HashMap<String, i64>,  // device_udid -> recording_id
    paused_recordings: HashSet<String>,        // device_udid for paused recordings
}
```

### API Design

```http
POST /api/recordings/{id}/pause
Response (200 OK):
{
  "status": "success",
  "message": "Recording paused"
}

POST /api/recordings/{id}/resume
Response (200 OK):
{
  "status": "success",
  "message": "Recording resumed"
}

GET /api/recordings/{id}/status
Response (200 OK):
{
  "status": "success",
  "recording_status": "active|paused|stopped",
  "action_count": 5
}

PUT /api/recordings/{id}/actions/{action_id}
Content-Type: application/json
{
  "x": 150,
  "y": 250
}
Response (200 OK):
{
  "status": "success",
  "action": { ... }
}

DELETE /api/recordings/{id}/actions/{action_id}
Response (200 OK):
{
  "status": "success",
  "message": "Action deleted and remaining actions renumbered"
}

POST /api/recordings/{id}/cancel
Response (200 OK):
{
  "status": "success",
  "message": "Recording cancelled and discarded"
}
```

### Error Codes

| Code | Description |
|------|-------------|
| `ERR_RECORDING_NOT_ACTIVE` | Recording exists but is not active for this device |
| `ERR_RECORDING_ALREADY_PAUSED` | Recording is already paused |
| `ERR_RECORDING_NOT_PAUSED` | Recording is not paused |
| `ERR_ACTION_NOT_FOUND` | Action ID doesn't exist in this recording |
| `ERR_CANNOT_MODIFY_SAVED` | Cannot modify actions in a saved/stopped recording |

### References

- [Source: src/services/recording_service.rs] - Service to extend
- [Source: src/routes/recording.rs] - Routes to extend
- [Source: src/models/recording.rs] - Models to extend
- [Source: resources/templates/index.html] - UI to extend
- [Source: _bmad-output/implementation-artifacts/4-3-action-recording-system.md] - Previous story

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### File List

- `src/services/recording_service.rs` - Add pause/resume/cancel methods, action CRUD
- `src/routes/recording.rs` - Add new API endpoints
- `src/models/recording.rs` - Add request/response types
- `src/main.rs` - Register new routes
- `resources/templates/index.html` - Add UI controls
- `resources/static/css/terminal-theme.css` - Add paused indicator styles
- `tests/test_server.rs` - Add E2E tests
