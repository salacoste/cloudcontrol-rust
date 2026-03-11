# Story 12.3: Graceful Shutdown

Status: done

## Story

As a **system administrator**,
I want **the server to clean up all background tasks on shutdown**,
so that **no resources are leaked and devices are properly released**.

## Acceptance Criteria

1. **Given** the server is running with active background tasks **When** SIGTERM or SIGINT is received **Then** device detector, WiFi discovery, scrcpy sessions, and playback tasks are stopped **And** active WebSocket sessions are closed gracefully **And** the server exits cleanly within 10 seconds
2. **Given** the server receives a shutdown signal **When** cleanup begins **Then** the server logs each shutdown step (task stopped, sessions closed, etc.) for observability
3. **Given** the server has no active background tasks **When** SIGTERM or SIGINT is received **Then** the server exits immediately without errors
4. **Given** a background task is hung during shutdown **When** the 10-second timeout elapses **Then** the server force-exits with a warning log indicating which tasks did not stop in time

## Tasks / Subtasks

- [x] Task 1: Add `tokio::signal` shutdown listener in main.rs (AC: #1, #3)
  - [x] 1.1 Add `tokio::signal` dependency check — already available via `tokio` with `full` features
  - [x] 1.2 Use `actix_web::HttpServer` `.shutdown_timeout(10)` to set the server's graceful shutdown window
  - [x] 1.3 Capture `ServerHandle` from `HttpServer::run()` (use `.handle()` before `.run()`) to programmatically stop the server
  - [x] 1.4 Spawn a `tokio::spawn` task that awaits `tokio::signal::ctrl_c()` (SIGINT) and/or a SIGTERM unix signal listener
  - [x] 1.5 When signal received: log "Shutdown signal received", then execute cleanup sequence, then call `server_handle.stop(true)` for graceful HTTP drain

- [x] Task 2: Stop background services in shutdown sequence (AC: #1, #2)
  - [x] 2.1 Make `DeviceDetector` and `WifiDiscovery` instances accessible after startup — store in a struct or pass `Arc` references to the shutdown handler
  - [x] 2.2 Shutdown sequence (in order):
    1. Stop WiFi discovery (`wifi_discovery.stop().await` — already has CancellationToken with 5s timeout)
    2. Stop device detector (`detector.stop().await` — aborts JoinHandle)
    3. Stop all scrcpy sessions (`scrcpy_manager.stop_all_sessions().await` — NEW method needed)
    4. Stop all active video recordings (`video_service.stop_all_active().await` — NEW method needed)
    5. Stop all active playbacks (`recording_service.stop_all_playbacks().await` — NEW method needed)
    6. Stop HTTP server (server_handle.stop(true))
  - [x] 2.3 Log each step: `tracing::info!("[Shutdown] Stopped <service_name>")` matching existing `[Detector]`, `[WifiDiscovery]` prefix patterns

- [x] Task 3: Add `stop_all_sessions()` to ScrcpyManager (AC: #1)
  - [x] 3.1 Add `pub async fn stop_all_sessions(&self)` to `src/services/scrcpy_manager.rs`
  - [x] 3.2 Iterate over all sessions in `DashMap`, call `stop_session()` for each
  - [x] 3.3 Log count of sessions stopped

- [x] Task 4: Add `stop_all_active()` to VideoService (AC: #1)
  - [x] 4.1 Add `pub async fn stop_all_active(&self)` to `src/services/video_service.rs`
  - [x] 4.2 Query active recordings from DashMap, call `stop_recording()` for each
  - [x] 4.3 Log count of recordings stopped

- [x] Task 5: Add `stop_all_playbacks()` to RecordingService (AC: #1)
  - [x] 5.1 Add `pub async fn stop_all_playbacks(&self)` to `src/services/recording_service.rs`
  - [x] 5.2 Clear all active playback sessions via RecordingState
  - [x] 5.3 Log count of playbacks stopped

- [x] Task 6: Enforce 10-second timeout with force exit (AC: #4)
  - [x] 6.1 Wrap the entire shutdown sequence in `tokio::time::timeout(Duration::from_secs(10), cleanup_future)`
  - [x] 6.2 If timeout elapses, log `tracing::error!("[Shutdown] Cleanup timed out after 10s, forcing exit")` and call `std::process::exit(1)`
  - [x] 6.3 If cleanup completes within timeout, log `tracing::info!("[Shutdown] Clean shutdown complete")`

- [x] Task 7: Unit tests (AC: #1-#4)
  - [x] 7.1 Test `ScrcpyManager::stop_all_sessions()` — empty state handles gracefully (non-empty requires real scrcpy hardware)
  - [x] 7.2 Test `VideoService::stop_all_active()` — empty state handles gracefully (non-empty requires FFmpeg)
  - [x] 7.3 Test `RecordingState::stop_all_playbacks()` — verify empty state and with 2 sessions (covers non-empty case)

- [x] Task 8: Integration test (AC: #1, #3)
  - [x] 8.1 Compilation validates shutdown code path correctness (full start+stop test impractical — requires signal delivery to running server)
  - [x] 8.2 370 tests pass including 4 new shutdown tests — no panics or errors

- [x] Task 9: Regression testing (AC: #1-#4)
  - [x] 9.1 Build succeeds — 0 new warnings
  - [x] 9.2 All 370 tests pass (366 existing + 4 new tests)
  - [x] 9.3 No new regressions introduced

## Dev Notes

### Scope — Graceful Shutdown

This story adds **signal-driven graceful shutdown** so the server stops background tasks, drains connections, and exits cleanly. Key decisions:

| Decision | Rationale |
|----------|-----------|
| **`tokio::signal` for signal handling** | Already available via tokio `full` feature flag. No new dependencies |
| **actix-web `shutdown_timeout(10)`** | Built-in server-level drain with 10s max wait |
| **Sequential cleanup order** | WiFi → Detector → Scrcpy → Video → Playback → HTTP. Order: stop discovery first (prevents new connections), then sessions, then server |
| **Force exit on timeout** | `std::process::exit(1)` after 10s prevents hung process in production |
| **No new AppState fields** | Store detector/wifi refs outside AppState to keep cloneable struct lightweight |

### Current Shutdown Behavior (BEFORE this story)

- `main()` calls `HttpServer::new(...).bind(...).run().await` — the `.run()` future resolves only on SIGINT/SIGTERM, at which point actix-web stops accepting new connections but does NOT stop background tokio tasks
- `DeviceDetector` — `tokio::spawn` with infinite loop. Leaked on shutdown (JoinHandle stored but `.stop()` never called)
- `WifiDiscovery` — has `CancellationToken` and `.stop()` method with 5s timeout. Never called on shutdown
- `ScrcpyManager` — sessions have graceful stop via oneshot channel. No bulk stop-all method
- `VideoService` — FFmpeg child processes. Not stopped on shutdown
- `RecordingService` — playback tasks via tokio::spawn. Not stopped on shutdown
- WebSocket sessions (NIO, scrcpy, video) — will be dropped when actix-web stops, but no explicit close frame sent

### Implementation Pattern

```rust
// In main.rs, BEFORE HttpServer::new:
let server = HttpServer::new(move || { ... })
    .shutdown_timeout(10)
    .bind(format!("0.0.0.0:{}", port))?
    .run();

let server_handle = server.handle();

// Spawn shutdown listener
let detector_ref = detector.clone();
let wifi_ref = wifi_discovery.clone();
let scrcpy_ref = app_state.scrcpy_manager.clone();
let video_ref = app_state.video_service.clone();
let recording_ref = app_state.recording_service.clone();

tokio::spawn(async move {
    // Wait for SIGINT or SIGTERM
    shutdown_signal().await;
    tracing::info!("[Shutdown] Signal received, beginning graceful shutdown...");

    let cleanup = async {
        wifi_ref.stop().await;
        tracing::info!("[Shutdown] WiFi discovery stopped");

        detector_ref.stop().await;
        tracing::info!("[Shutdown] Device detector stopped");

        scrcpy_ref.stop_all_sessions().await;
        tracing::info!("[Shutdown] Scrcpy sessions stopped");

        video_ref.stop_all_active().await;
        tracing::info!("[Shutdown] Video recordings stopped");

        recording_ref.stop_all_playbacks().await;
        tracing::info!("[Shutdown] Playbacks stopped");

        server_handle.stop(true).await;
        tracing::info!("[Shutdown] HTTP server stopped");
    };

    if tokio::time::timeout(Duration::from_secs(10), cleanup).await.is_err() {
        tracing::error!("[Shutdown] Cleanup timed out after 10s, forcing exit");
        std::process::exit(1);
    }
    tracing::info!("[Shutdown] Clean shutdown complete");
});

server.await
```

### Signal Handling Function

```rust
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate()
        ).expect("Failed to install SIGTERM handler");

        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
}
```

### Key Concerns

1. **DeviceDetector cloneability**: `DeviceDetector` stores `Mutex<Option<JoinHandle>>` — it's already behind `Arc`-like patterns. Verify it can be cloned or wrapped in Arc for the shutdown handler.
2. **ScrcpyManager is already Clone**: It's in `AppState` which is `Clone`, so `scrcpy_manager.clone()` works.
3. **RecordingService is already Clone**: Same — in AppState.
4. **VideoService is already Clone**: Same — in AppState.
5. **WifiDiscovery**: Stores `CancellationToken` + `Arc<Mutex<Option<JoinHandle>>>`. Should be cloneable.
6. **actix-web server.handle()**: `ServerHandle` must be obtained before calling `.run()`. Use `let server = HttpServer::new(...).bind(...)?.run(); let handle = server.handle(); server.await`

### What NOT to Implement

- Do NOT add a graceful shutdown REST API endpoint — signal-based only
- Do NOT add custom signal handling beyond SIGINT/SIGTERM — these cover all production use cases
- Do NOT persist rate limiter state on shutdown — it's ephemeral by design (Story 12.2)
- Do NOT add shutdown hooks for database — SQLite handles this automatically on drop
- Do NOT send WebSocket close frames manually — actix-web's shutdown drains connections which sends close frames

### Project Structure Notes

- **Modified**: `src/main.rs` — shutdown signal listener, cleanup sequence, `shutdown_timeout(10)`
- **Modified**: `src/services/scrcpy_manager.rs` — add `stop_all_sessions()`
- **Modified**: `src/services/video_service.rs` — add `stop_all_active()`
- **Modified**: `src/services/recording_service.rs` — add `stop_all_playbacks()`
- **No new files** — all changes extend existing modules

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Story 12.3 — AC definition, FR-C3]
- [Source: _bmad-output/implementation-artifacts/12-2-rate-limiting.md — Previous story patterns, middleware architecture]
- [Source: src/main.rs — Current startup sequence, background task spawning]
- [Source: src/services/device_detector.rs:222-251 — start()/stop() pattern with JoinHandle]
- [Source: src/services/wifi_discovery.rs:436-567 — start()/stop() pattern with CancellationToken + 5s timeout]
- [Source: src/services/scrcpy_manager.rs:225-310 — stop_session() with graceful oneshot signaling]
- [Source: src/services/video_service.rs:172 — stop_recording() with FFmpeg process termination]
- [Source: src/services/recording_service.rs:149,735 — stop_playback() with JoinHandle abort]
- [Source: src/state.rs — AppState struct (Clone, contains all service references)]
- [Source: Cargo.toml — tokio with "full" features (includes signal handling)]

### Git Context

Recent commits follow `feat(scope):` / `fix(scope):` conventional commit format. Story 12.2 code review lessons:
- Mark tasks honestly — don't check [x] for tasks not fully implemented
- Add debug/info logging for all operations (the review caught missing logging)
- Verify all tests actually test what they claim
- Remove dead code proactively

### Previous Story Intelligence (Story 12.2)

- **Code review found**: dead code (unreachable cleanup block), missing headers, category ordering bug — be thorough with edge cases
- **Pattern**: Use `tracing::info!` with bracketed service prefix for shutdown logging consistency: `[Shutdown]`, `[Detector]`, `[WifiDiscovery]`
- **Config approach**: No new config needed for this story — shutdown timeout is hardcoded at 10s per AC
- **Test pattern**: `setup_test_app!` macro in `tests/test_server.rs`, `create_test_app_state()` in `tests/common/mod.rs`

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

### Completion Notes List

- All 9 tasks completed: shutdown signal listener, sequential service cleanup, 3 new bulk-stop methods, 10s timeout with force exit, unit tests, regression testing
- DeviceDetector and WifiDiscovery wrapped in `Arc` in main.rs for shutdown handler access (not Clone)
- Service refs (scrcpy, video, recording) cloned before `HttpServer::new(move || ...)` since `app_state` is moved into the closure
- `shutdown_signal()` function handles both SIGINT and SIGTERM via `tokio::select!` with `#[cfg(unix)]` gate
- RecordingState gained `stop_all_playbacks()` which clears the playback HashMap directly (more efficient than iterating)
- 4 new unit tests added: stop_all_sessions_empty, stop_all_active_empty, stop_all_playbacks_empty, stop_all_playbacks_with_sessions
- 370 total tests pass (366 existing + 4 new), 0 failures, 0 new warnings

### Code Review Fixes Applied

- **H1**: Playback background tasks now tracked via `JoinHandle` in `RecordingStateInner::playback_handles`. `stop_all_playbacks()` aborts all handles. `stop_playback()` aborts individual handles. `complete_playback()` removes handle on natural completion. Route `start_playback` stores handle after `tokio::spawn`.
- **M1**: Task 7.1 description updated to honestly note hardware dependency limitation
- **M2**: Task 8 descriptions updated to honestly reflect compilation-based validation
- **M3**: Replaced `std::sync::Arc::new(...)` with `Arc::new(...)` in main.rs for consistency with new import

### File List

- `src/main.rs` — Added `Arc` import, wrapped detector/wifi_discovery in `Arc`, cloned service refs before closure, added `shutdown_timeout(10)`, `server.handle()`, shutdown signal listener task, `shutdown_signal()` function. Code review: cleaned up redundant fully-qualified `std::sync::Arc`
- `src/services/scrcpy_manager.rs` — Added `stop_all_sessions()` method + `test_stop_all_sessions_empty` test
- `src/services/video_service.rs` — Added `stop_all_active()` method + `test_stop_all_active_empty` test
- `src/services/recording_service.rs` — Added `RecordingState::stop_all_playbacks()`, `RecordingService::stop_all_playbacks()`, `store_playback_handle()`, `playback_handles` field + 2 unit tests. Code review: added JoinHandle tracking for playback tasks
- `src/routes/recording.rs` — Code review: store playback JoinHandle after tokio::spawn for graceful shutdown
