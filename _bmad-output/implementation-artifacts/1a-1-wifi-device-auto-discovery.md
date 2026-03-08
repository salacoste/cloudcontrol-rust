# Story 1A-1: WiFi Device Auto-Discovery

**Epic:** 1A - Device Connection & Discovery
**Status:** in-progress
**Priority:** P0
**FRs Covered:** FR1

---

## Story

> As a **Device Farm Operator**, I want devices on my network to be automatically discovered, so that I don't have to manually configure each device.

---

## Acceptance Criteria

```gherkin
Scenario: Discover devices on standard ATX port
  Given the system is running
  And there are Android devices running ATX Agent on port 7912 reachable via WiFi
  When the discovery scan executes
  Then all reachable devices appear in the device list
  And each device shows connection status "connected"

Scenario: Discover devices on alternate port
  Given there are devices running ATX Agent on port 9008
  When the discovery scan executes
  Then devices on port 9008 appear in the device list

Scenario: Handle network timeout gracefully
  Given a device is unreachable
  When the discovery scan executes
  Then the system logs the timeout
  And the device does not appear in the list
  And no error is thrown to the user
```

---

## Tasks/Subtasks

- [ ] **Task 1: Create WiFi discovery service module**
  - [ ] Create `src/services/wifi_discovery.rs`
  - [ ] Define `WifiDiscovery` struct with configuration (scan interval, timeout, ports)
  - [ ] Implement `new()` constructor with default config
  - [ ] Add module to `src/services/mod.rs`

- [ ] **Task 2: Implement network scanning logic**
  - [ ] Create `scan_subnet()` method to iterate through local subnet
  - [ ] Implement `probe_device(ip, port)` to check ATX agent availability
  - [ ] Probe both ports 7912 and 9008 for each IP
  - [ ] Use async concurrent scanning with `tokio::join!` or `futures::join_all`
  - [ ] Implement configurable timeout (default: 500ms per probe)

- [ ] **Task 3: Detect local subnet automatically**
  - [ ] Create `get_local_subnet()` utility function
  - [ ] Parse network interfaces to find WiFi/ethernet subnet
  - [ ] Return list of IPs to scan (e.g., 192.168.1.1-254)
  - [ ] Handle multiple network interfaces

- [ ] **Task 4: Integrate with PhoneService for device registration**
  - [ ] Call `phone_service.on_connected()` for discovered devices
  - [ ] Fetch device info via ATX `/info` endpoint
  - [ ] Track known devices to avoid duplicate registrations
  - [ ] Mark devices offline when no longer discovered

- [ ] **Task 5: Implement background polling loop**
  - [ ] Create `start()` method with tokio::spawn background task
  - [ ] Implement configurable scan interval (default: 30 seconds)
  - [ ] Add `stop()` method for graceful shutdown
  - [ ] Log discovery events with tracing

- [ ] **Task 6: Wire into application startup**
  - [ ] Add `WifiDiscovery` to `AppState` or initialize in `main.rs`
  - [ ] Start discovery service on application startup
  - [ ] Stop discovery service on shutdown

---

## Dev Notes

### Architecture Context

**Existing Code Pattern (from `device_detector.rs`):**
```rust
pub struct DeviceDetector {
    phone_service: PhoneService,
    known_devices: Arc<Mutex<HashMap<String, String>>>,
    poll_handle: Mutex<Option<JoinHandle<()>>>,
}
```

**Service Pattern:**
- Services are created per-request or as singletons with Arc
- Use `tokio::spawn` for background tasks
- Store known devices in `Arc<Mutex<HashMap>>` for thread safety
- Implement `start()` and `stop()` methods

**Naming Conventions:**
- Files: `snake_case` (e.g., `wifi_discovery.rs`)
- Structs: `PascalCase` (e.g., `WifiDiscovery`)
- Functions: `snake_case` (e.g., `scan_subnet()`)
- Variables: `snake_case`

### ATX Agent Protocol

**Endpoints:**
- `GET http://{ip}:{port}/info` - Device info JSON
- `POST http://{ip}:{port}/jsonrpc/0` - JSON-RPC commands
- Port 7912: Old atx-agent (Python)
- Port 9008: New uiautomator2 (Java)

**Device Info Response:**
```json
{
  "serial": "device_serial",
  "brand": "Samsung",
  "model": "Galaxy S21",
  "version": "12",
  "sdk": 31,
  "hwaddr": "aa:bb:cc:dd:ee:ff",
  "agentVersion": "2.0.0",
  "display": {"width": 1080, "height": 2400},
  "battery": {"level": 85},
  "memory": {"total": 8589934592},
  "cpu": {"cores": 8}
}
```

### Existing Code to Reference

**PhoneService (`src/services/phone_service.rs`):**
- `on_connected(identifier, host)` - Register new device
- `offline_connected(identifier)` - Mark device offline
- `update_field(identifier, item)` - Generic update

**AtxClient (`src/device/atx_client.rs`):**
- `device_info()` - Fetches `/info` endpoint
- HTTP client with timeout configuration

**DeviceDetector (`src/services/device_detector.rs`):**
- Background polling pattern
- Known device tracking
- Integration with PhoneService

### Implementation Approach

1. **Subnet Detection:**
   - Use `pnet` crate or parse `ifconfig`/`ip addr` output
   - Alternative: Allow configuration of subnet range
   - Simple approach: Scan common /24 subnets (192.168.1.x, 192.168.0.x, 10.0.0.x)

2. **Concurrent Scanning:**
   - Use `futures::stream` with buffer for rate limiting
   - Max 50 concurrent probes to avoid network saturation
   - Each probe: HTTP GET with 500ms timeout

3. **Device Identification:**
   - Use `hwaddr` (MAC) or `serial` as unique identifier
   - Build UDID format: `{serial}-{model}` (matching existing pattern)

4. **Discovery vs Detection Coexistence:**
   - `WifiDiscovery` discovers devices via network scan
   - `DeviceDetector` manages USB devices via ADB
   - Both use `PhoneService` for registration
   - Track discovered devices separately from detected devices

### Performance Targets

| Metric | Target |
|--------|--------|
| Scan duration (254 IPs) | <30 seconds |
| Per-probe timeout | 500ms |
| Scan interval | 30 seconds |
| Memory overhead | <10MB |

### Error Handling

- Network errors: Log and continue (don't fail scan)
- Timeout: Skip device, log at debug level
- Invalid response: Skip device, log warning
- Use `Result<T, String>` pattern in services

---

## Dev Agent Record

### Implementation Plan
(To be filled during implementation)

### Debug Log
(To be filled during implementation)

### Completion Notes

Implemented all 7 code review findings:
- HIGH #1: Wired WifiDiscovery into main.rs application startup
- HIGH #2: Added automatic subnet detection via `get_local_subnets()` in host_ip.rs
- MEDIUM #3: Added error logging in background task loop
- MEDIUM #4: Implemented graceful shutdown via CancellationToken
- MEDIUM #5: Optimized cloning with Arc<Client> shared reference
- LOW #6: Made ports configurable via WifiDiscoveryConfig
- LOW #7: Added retry logic with `offline_retry_count` before marking devices offline

All 61 tests pass. Story complete!

---

## File List
(To be updated during implementation)

---

## Change Log

| Date | Change |
|------|--------|
| 2026-03-05 | Story file created with full context |

---

## Status History

| Date | Status |
|------|--------|
| 2026-03-05 | backlog |
| 2026-03-05 | ready-for-dev |
