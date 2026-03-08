# Story 3.6: Shell Command Execution

Status: done

## Story

As an **Automation Engineer**, I want to execute shell commands on devices, so that I can perform advanced debugging and automation.

## Acceptance Criteria

1. **Execute simple shell command**
   - Given a device is connected
   - When I send POST /shell with command="getprop ro.build.version.release"
   - Then the Android version is returned
   - And the command executes in under 1 second
   - ~~And stdout and stderr are separated in response~~ **LIMITATION**: ATX Agent protocol returns combined output only

2. **Execute command with timeout**
   - Given a potentially slow command
   - When I send POST /shell with command="logcat -d" and timeout=5000
   - Then the command executes with 5-second timeout
   - And if timeout occurs, the process is killed
   - And a timeout error is returned
   - ~~And partial output is included~~ **LIMITATION**: ATX client doesn't support streaming output

3. **Handle dangerous commands safely**
   - Given I attempt to run "reboot" or "rm -rf"
   - When the command is received
   - Then the command is blocked or requires confirmation
   - And a warning is logged
   - And the user is notified of the restriction

4. **Real-time command output** (Optional/Future)
   - Given I run a long-running command
   - When I use WebSocket shell endpoint
   - Then output streams in real-time
   - And I can send input to interactive commands
   - And I can terminate the command early

## Tasks / Subtasks

- [x] Task 1: Improve shell endpoint request/response format (AC: 1)
  - [x] Change UDID from header to path parameter: POST /api/devices/{udid}/shell
  - [x] Accept JSON body with command and optional timeout
  - [x] Return structured JSON with stdout, stderr, exit_code
  - [x] Return proper error codes (400 for bad request, 404 for device not found)

- [x] Task 2: Implement dangerous command blocking (AC: 3)
  - [x] Define list of blocked commands: reboot, rm -rf /, factory-reset, etc.
  - [x] Return HTTP 403 with ERR_DANGEROUS_COMMAND for blocked commands
  - [x] Log warning when blocked command attempted
  - [x] Include list of blocked patterns in error message
  - [x] **Added**: Shell metacharacter detection with logging (not blocked, but audited)
  - [x] **Added**: Extended blocked patterns: init 6, svc power, killall, pm uninstall

- [x] Task 3: Add timeout support (AC: 2)
  - [x] Accept timeout parameter in request body (default: 30000ms, max: 60000ms)
  - [x] Implement timeout handling using tokio::time::timeout
  - [x] Return ERR_COMMAND_TIMEOUT when timeout exceeded
  - [x] ~~Include partial output in timeout response~~ **LIMITATION**: ATX client doesn't support streaming

- [x] Task 4: Add E2E tests
  - [x] Test successful command execution
  - [x] Test dangerous command blocking
  - [x] Test command timeout
  - [x] Test nonexistent device returns 404
  - [x] Test empty command returns 400

## Dev Notes

### Existing Implementation

Shell functionality is **partially implemented** in `src/routes/control.rs`:

**Current Issues:**
1. Uses `Access-Control-Allow-Origin` header to get UDID (incorrect usage)
2. Returns static text instead of command output
3. No timeout support
4. No dangerous command blocking
5. No stdout/stderr separation
6. No proper error handling

```rust
// From src/routes/control.rs:1395-1426
pub async fn shell(
    state: web::Data<AppState>,
    req: HttpRequest,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let udid = req
        .headers()
        .get("Access-Control-Allow-Origin")  // WRONG: misusing CORS header for UDID
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    // ...
    let _ = client.shell_cmd(&command).await;
    HttpResponse::Ok().body(format!("{} sized of 0 successfully stored", udid))  // WRONG: returns garbage
}
```

### ATX Client Shell Method

The `shell_cmd` method in `src/device/atx_client.rs` already exists:

```rust
pub async fn shell_cmd(&self, cmd: &str) -> Result<String, String> {
    let url = format!("{}/shell", self.base_url);
    let resp = self.client
        .get(&url)
        .query(&[("command", cmd)])
        .send()
        .await
        .map_err(|e| format!("Shell command failed: {}", e))?;
    let text = resp.text().await...;
}
```

### Architecture Constraints

- Use existing `shell_cmd` in AtxClient for ATX Agent communication
- Add ADB fallback via `Adb::shell()` for USB devices
- Follow existing error handling patterns with `ERR_*` codes
- Maintain response time under 1 second (NFR16)
- Use fire-and-forget pattern only for non-critical commands

### API Design

```
POST /api/devices/{udid}/shell
Request Body:
{
  "command": "getprop ro.build.version.release",
  "timeout": 5000  // optional, default 30000, max 60000
}

Response (200 OK):
{
  "status": "success",
  "stdout": "14",
  "stderr": "",
  "exit_code": 0,
  "duration_ms": 123
}

Response (403 Forbidden for dangerous command):
{
  "status": "error",
  "error": "ERR_DANGEROUS_COMMAND",
  "message": "Command 'reboot' is blocked for safety. Blocked patterns: reboot, rm -rf /, factory-reset, ..."
}

Response (408 Request Timeout):
{
  "status": "error",
  "error": "ERR_COMMAND_TIMEOUT",
  "message": "Command exceeded 5000ms timeout",
  "partial_stdout": "...partial output...",
  "partial_stderr": ""
}
```

### Dangerous Commands List

Block commands that could:
1. Disrupt device connectivity: `reboot`, `shutdown`
2. Cause data loss: `rm -rf /`, `format`, `factory-reset`
3. Modify system: `mount`, `umount`, `dd`
4. Install/remove packages: `pm install`, `pm uninstall` (require confirmation)

Pattern matching approach:
```rust
const BLOCKED_PATTERNS: &[&str] = &[
    "reboot", "shutdown", "restart",
    "rm -rf /", "rm -rf /*", 
    "factory-reset", "recovery",
    "dd if=", "dd of=",
    "mount ", "umount ",
];

fn is_dangerous_command(cmd: &str) -> bool {
    let cmd_lower = cmd.to_lowercase();
    BLOCKED_PATTERNS.iter().any(|p| cmd_lower.contains(p))
}
```

### Project Structure Notes

- Route handlers: `src/routes/control.rs` - modify `shell` function
- ATxClient: `src/device/atx_client.rs` - existing `shell_cmd()` method
- ADB: `src/device/adb.rs` - add `shell_with_timeout()` if needed
- Tests: `tests/test_server.rs` - add E2E tests for shell endpoint
- Main.rs: Add new route `/api/devices/{udid}/shell`

### Performance Requirements

- NFR3: API response time <100ms (command execution can be longer)
- NFR16: ADB command execution <1s for standard commands
- Shell command timeout: default 30s, max 60s

### Previous Story Learnings (3-4 Physical Key Events)

1. **Input validation**: Validate all input parameters before processing
2. **Error handling**: Return proper HTTP status codes (400 for bad request, 404 for device not found)
3. **Mock device handling**: Check `is_mock` flag and return success immediately for mock devices
4. **Fire-and-forget pattern**: Use `tokio::spawn` for async operations to maintain response time
5. **Case-insensitive matching**: Use `to_lowercase()` for command pattern matching

### References

- [Source: src/routes/control.rs:1395-1426] - Existing shell implementation
- [Source: src/device/atx_client.rs:256-267] - AtxClient shell_cmd method
- [Source: src/device/adb.rs] - ADB shell commands
- [Source: _bmad-output/planning-artifacts/epics-stories.md:792-828] - Story definition
- [Source: _bmad-output/implementation-artifacts/3-4-physical-key-events.md] - Previous story patterns
- [Source: docs/architecture.md] - Architecture constraints

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

None - implementation was straightforward.

### Completion Notes List

1. **New Endpoint Created**: `POST /api/devices/{udid}/shell` with JSON body
2. **Dangerous Command Blocking**: Implemented `BLOCKED_COMMAND_PATTERNS` with case-insensitive matching
3. **Timeout Support**: Uses `tokio::time::timeout` with configurable timeout (1s-60s)
4. **Structured Response**: Returns JSON with stdout, stderr, exit_code, duration_ms
5. **Error Handling**: Proper HTTP status codes (400, 403, 404, 408, 500)
6. **Mock Device Support**: Returns mock output for mock devices

**Code Review Fixes Applied:**
7. **Extended Blocked Patterns**: Added init 6, svc power reboot, killall, pm uninstall, chmod 777 /
8. **Shell Metacharacter Detection**: Added `has_dangerous_metacharacters()` for audit logging
9. **Documented Limitations**: ATX Agent doesn't support stderr separation or streaming output

### File List

- `src/routes/control.rs` - Added `execute_shell` function with dangerous command blocking and timeout support
- `src/main.rs` - Added route `/api/devices/{udid}/shell`
- `tests/test_server.rs` - Added 9 E2E tests for shell endpoint
