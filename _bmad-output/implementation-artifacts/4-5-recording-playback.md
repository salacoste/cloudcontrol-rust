# Story 4.5: Recording Playback

Epic: 4 (Multi-Device Batch Operations)
Status: done
Story: 4-4-recording-session-management (completed)
Priority: P2

## Story

As a **QA Engineer**, I want to playback recorded actions on a target device so that I can verify the recordings work correctly before using them in real scenarios.

## Acceptance Criteria

```gherkin
Feature: Play recorded actions sequentially
  Given a recording has 5 actions
  When I select the recording for playback
  Then all actions are played in sequence order
  And the device executes them one by one
  And the playback progress updates in real-time

Feature: Configurable playback speed
  Given a recording has 5 actions
  When I start playback at 1x speed
  Then actions are played at 1x speed with a 0.5s delay between each
  And the playback progress updates accordingly

Feature: Playback controls
  Given a recording is playing
  When I press the "Stop" button
  Then playback stops
  And the progress indicator shows 100% complete

Feature: Playback progress indicator
  Given a recording is playing
  When I view the recording
  Then the progress indicator shows "3/5 actions (60%)"
  And the current action is highlighted

Feature: Playback from non-existent recording
  Given a non-existent recording ID
  When I request playback
  Then a 404 error is returned
  And no actions are played

Feature: Playback on cancelled recording
  Given a recording was cancelled
  When I request playback
  Then a 400 error is returned
  And a clear message indicates the recording was cancelled
```

## Tasks/Subtasks

- [x] Task 1: Add playback execution engine
  - [x] Create PlaybackService with execute_playback method
  - [x] Create playback state tracking (current_recording, current_action_index)
  - [x] Add pause/resume/stop methods

- [x] Task 2: Add playback API endpoint
  - [x] POST /api/recordings/{id}/play - Start playback
  - [x] GET /api/recordings/{id}/playback/status - Get playback status

- [x] Task 3: Add playback UI controls
  - [x] Add "Play Recording" button with recording selection modal
  - [x] Add playback speed selector (0.25x - 4x)
  - [x] Add playback progress indicator (current/total + percentage)
  - [x] Add "Stop Playback" button

- [x] Task 4: Write E2E tests for playback
  - [x] Test playback start and status (test_playback_start_and_status)
  - [x] Test playback speed control (test_playback_speed_control)
  - [x] Test playback stop functionality (test_playback_stop)
  - [x] Test playback progress updates (test_playback_start_and_status)
  - [x] Test error cases: non-existent recording, empty recording, duplicate start, missing params

## Dev Notes

### Architecture Context
This story extends the recording functionality from Story 4-3 with playback capabilities. The Recording model from Story 4-3 is now extended with a `playback_status` field that tracks the state of a recording during playback.

### Implementation Pattern
- Create a new `PlaybackService` similar to `RecordingService`
- Use the existing `RecordingState` to track playback state (current recording, current_action_index)
- Add a `playback` method that:
  1. Retrieves all actions from the recording
  2. Executes each action in sequence order
  3. Updates playback state
  4. Reports progress via WebSocket

- Use the existing `execute_control` method to execute each action
- For pause/resume, reuse the existing `pause_recording`/`resume_recording` logic

### References
- [Source: src/services/recording_service.rs](./services/recording_service.rs) - Service to extend
- [Source: src/routes/recording.rs](./routes/recording.rs) - Routes to extend
- [Source: src/models/recording.rs](./models/recording.rs) - Models to extend
- [Source: resources/templates/index.html] - UI to extend
- [Source: _bmad-output/implementation-artifacts/4-3-action-recording-system.md] - Previous story

- [Source: _bmad-output/implementation-artifacts/4-4-recording-session-management.md] - Previous story (for pause/resume/cancel patterns)

- [Source: _bmad-output/implementation-artifacts/sprint-status.yaml] - Updated with story 4-5 status


