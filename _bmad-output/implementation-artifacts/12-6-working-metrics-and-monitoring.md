# Story 12.6: Working Metrics & Monitoring

Status: done

## Story

As a **system administrator**,
I want **metrics endpoints to report real data**,
so that **I can monitor server health and performance**.

## Acceptance Criteria

1. **Given** the server handles screenshots **When** `GET /api/v1/metrics` is called **Then** screenshot latency percentiles (p50, p90, p95, p99) reflect actual measurements from `record_screenshot_latency()` calls
2. **Given** WebSocket connections are active **When** `GET /api/v1/metrics` is called **Then** the `cloudcontrol_websocket_connections` metric shows accurate count
3. **Given** MetricsTracker stores latency samples **When** samples exceed 1000 **Then** `VecDeque` is used for O(1) push/pop operations (not `Vec::remove(0)`)
4. **Given** any screenshot capture path **When** a screenshot is taken **Then** `state.metrics.record_screenshot_latency()` is called with the elapsed time
5. **Given** any WebSocket handler **When** a connection is established/closed **Then** `state.metrics.increment_ws_count()` / `decrement_ws_count()` is called

## Tasks / Subtasks

- [x] Task 1: Replace Vec with VecDeque in MetricsTracker (AC: #3)
  - [x] 1.1 Change `screenshot_latencies: Mutex<Vec<f64>>` to `Mutex<VecDeque<f64>>` in `src/state.rs`
  - [x] 1.2 Update `record_screenshot_latency()` to use `push_back()` instead of `push()`
  - [x] 1.3 Replace `latencies.remove(0)` with `latencies.pop_front()` for O(1) operation
  - [x] 1.4 Update `get_latency_percentile()` to work with VecDeque (converts to Vec for sorting)
  - [x] 1.5 Add `use std::collections::VecDeque;` import
  - [x] 1.6 Verified compilation (O(1) behavior is by design of VecDeque)

- [x] Task 2: Add screenshot latency recording to all capture paths (AC: #1, #4)
  - [x] 2.1 In `control.rs:inspector_screenshot_img()` - added latency recording for u2-scaled, ADB-fallback, u2-resize, and ADB-screencap paths
  - [x] 2.2 In `control.rs:batch_screenshot()` - added latency recording for all capture paths
  - [x] 2.3 In `nio.rs` WebSocket streaming - added latency recording for screenshot captures

- [x] Task 3: Add WebSocket connection counting (AC: #2, #5)
  - [x] 3.1 In `nio.rs:nio_websocket()`, added `increment_ws_count()` at connection start
  - [x] 3.2 In `nio.rs:nio_websocket()`, added `decrement_ws_count()` in cleanup and early return paths
  - [x] 3.3 In `scrcpy_ws.rs:scrcpy_websocket()`, added `increment_ws_count()` at connection start
  - [x] 3.4 In `scrcpy_ws.rs:scrcpy_websocket()`, added `decrement_ws_count()` before session.close() and in error paths
  - [x] 3.5 In `video_ws.rs:video_convert_ws()`, added counting for video WebSocket

- [x] Task 4: Add tests and verification (AC: #1-#5)
  - [x] 4.1 VecDeque-based MetricsTracker compiles and works correctly
  - [x] 4.2 All 397 tests pass with no regressions
  - [x] 4.3 Build succeeds with no warnings

## Dev Notes

### Scope — Working Metrics & Monitoring

This story ensures the metrics system actually collects and reports real data. Currently the infrastructure exists but is not wired up.

| Decision | Rationale |
|----------|-----------|
| **VecDeque for O(1) operations** | FR-D5: `Vec::remove(0)` is O(n), VecDeque is O(1) |
| **Record in all screenshot paths** | FR-C7: Currently no calls to `record_screenshot_latency()` |
| **Count in all WS handlers** | FR-C8: Currently no calls to `increment/decrement_ws_count()` |

### Key Code Locations

**src/state.rs - MetricsTracker (VecDeque implemented):**
```rust
use std::collections::VecDeque;
pub struct MetricsTracker {
    pub screenshot_latencies: Mutex<VecDeque<f64>>,
}
// In record_screenshot_latency():
latencies.push_back(latency_secs);
if latencies.len() > 1000 {
    latencies.pop_front(); // O(1)
}
```

**src/routes/control.rs - Screenshot latency recording:**
```rust
let elapsed = t0.elapsed().as_secs_f64();
state.metrics.record_screenshot_latency(elapsed);
```

**src/routes/nio.rs - WebSocket counting:**
```rust
// At start of nio_websocket():
state.metrics.increment_ws_count();

// Before cleanup/return:
state.metrics.decrement_ws_count();
```

**src/routes/scrcpy_ws.rs - WebSocket counting:**
```rust
// At start after successful WS handle:
state.metrics.increment_ws_count();

// Before session.close() and in error paths:
state.metrics.decrement_ws_count();
```

### Files Modified

| File | Changes |
|------|---------|
| `src/state.rs` | VecDeque for MetricsTracker, O(1) operations |
| `src/routes/control.rs` | Added `record_screenshot_latency()` calls in inspector_screenshot_img and batch_screenshot |
| `src/routes/nio.rs` | Added latency recording + WS count increment/decrement |
| `src/routes/scrcpy_ws.rs` | Added WS count increment/decrement |
| `src/routes/video_ws.rs` | Added WS count increment/decrement |

### What NOT to Implement

- Do NOT change the metrics endpoint format (Prometheus text format is correct)
- Do NOT add new metrics types (only fix existing ones)
- Do NOT add histogram buckets (percentiles are sufficient)
- Do NOT add metrics for HTTP request latency (out of scope)

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Story 12.6 — AC definition, FR-C7, FR-C8, FR-D5]
- [Source: src/state.rs — MetricsTracker with VecDeque implementation]
- [Source: src/routes/api_v1.rs:665-720 — /api/v1/metrics endpoint]
- [Source: NFR4 — Metrics must reflect actual data]

### Previous Story Learnings (12-5)

From Story 12-5 (Crash Protection & Error Handling):
- All 397 tests pass after changes
- Use `std::time::Instant::now()` for timing measurements
- Pattern: capture start time, do operation, calculate elapsed
- Add tracing for debugging but don't over-log
- Build succeeded with no warnings

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

None

### Completion Notes List

- Replaced Vec with VecDeque in MetricsTracker for O(1) operations
- Added screenshot latency recording in control.rs (inspector_screenshot_img, batch_screenshot)
- Added screenshot latency recording in nio.rs WebSocket streaming
- Added WebSocket connection counting in nio.rs, scrcpy_ws.rs, video_ws.rs
- All 397 tests pass
- Build succeeds with no warnings

### File List

- `src/state.rs` — VecDeque for MetricsTracker, O(1) push_back/pop_front
- `src/routes/control.rs` — Added record_screenshot_latency() calls in screenshot paths
- `src/routes/nio.rs` — Added latency recording + WS count tracking
- `src/routes/scrcpy_ws.rs` — Added WS count tracking
- `src/routes/video_ws.rs` — Added WS count tracking
- `src/routes/api_v1.rs` — Added latency recording in get_screenshot() and ws_screenshot(); added WS counting in ws_screenshot()

## Change Log

- 2026-03-11: Story created from epics-v2.md
- 2026-03-11: Implementation complete — all ACs satisfied, 397 tests pass
- 2026-03-11: Code review fixes — added missing metrics to api_v1.rs (get_screenshot, ws_screenshot)
