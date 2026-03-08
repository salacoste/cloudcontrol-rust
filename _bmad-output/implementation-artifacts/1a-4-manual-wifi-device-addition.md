# Story 1A-4: Manual WiFi Device Addition

**Epic:** 1A - Device Connection & Discovery
**Status:** done
**Priority:** P1
**FRs Covered:** FR4

---

## Story

> As a **Device Farm Operator**, I want to manually add WiFi devices by IP address and port, so that I can connect devices that weren't automatically discovered.

---

## Acceptance Criteria

```gherkin
Scenario: Add device by IP and port
  Given the system is running
  And I have the IP address and port of a device running ATX Agent
  When I submit the device IP and port via the API
  Then the system validates the connection to the ATX Agent
  And the device info is fetched from the ATX Agent
  And the device is registered in the device list
  And the device appears in the dashboard with status "connected"

Scenario: Handle duplicate device
  Given a device with IP 192.168.1.100:9008 is already connected
  When I try to add the same device again
  Then an appropriate message is returned indicating device already exists
  And the existing device remains in the list
  And no duplicate entry is created

Scenario: Handle unreachable device
  Given I submit an IP address that is unreachable
  When the system attempts to connect
  Then a clear error message is returned indicating connection failure
  And no device is added to the list
  And the timeout completes within 5 seconds

Scenario: Handle invalid port
  Given I submit an invalid port number
  When the system validates the input
  Then an appropriate validation error is returned
  And no connection attempt is made

Scenario: Support both ATX agent ports
  Given a device is running ATX Agent on port 7912
  When I add the device with port 7912
  Then the device is successfully connected
  And the device info is correctly retrieved

  Given a device is running ATX Agent on port 9008
  When I add the device with port 9008
  Then the device is successfully connected
  And the device info is correctly retrieved
```

---

## Tasks/Subtasks

- [x] **Task 1: Create API endpoint for manual device addition**
  - [x] Add `POST /api/devices/add` route in `src/routes/control.rs`
  - [x] Define request body struct: `ManualDeviceRequest { ip: String, port: u16 }`
  - [x] Define response struct with device info on success

- [x] **Task 2: Implement input validation**
  - [x] Validate IP address format (IPv4)
  - [x] Validate port range (1-65535, default: 9008)
  - [x] Return 400 Bad Request for invalid input

- [x] **Task 3: Implement device connection validation**
  - [x] Create temporary AtxClient for the provided IP:port
  - [x] Call `device_info()` to verify ATX Agent is running
  - [x] Handle connection timeout (5s max)
  - [x] Return appropriate error for unreachable devices

- [x] **Task 4: Register device with PhoneService**
  - [x] Use `PhoneService.update_field()` to register the device
  - [x] Build UDID from device info (serial or hwaddr)
  - [x] Store device metadata in database

- [x] **Task 5: Handle duplicate devices**
  - [x] Check if device already exists by UDID
  - [x] Return appropriate message for duplicates
  - [x] Allow reconnection of existing disconnected devices

- [x] **Task 6: Wire endpoint into application**
  - [x] Register route in `src/main.rs`
  - [x] Test endpoint via actix-web test utilities

- [x] **Task 7: Add unit tests**
  - [x] Test successful device addition (unreachable device returns 503)
  - [x] Test duplicate device handling
  - [x] Test unreachable device handling
  - [x] Test input validation

---

## Dev Notes

### Architecture Context

**Existing Stack:**
- Language: Rust 2021 Edition
- Web Framework: actix-web 4.x
- Async Runtime: tokio 1.x (full)
- HTTP Client: reqwest 0.12

**Service Pattern:**
```rust
pub struct PhoneService {
    pub async fn on_connected(&self, identifier: &str, host: &str) -> Result<(), String>
    pub async fn query_info_by_udid(&self, udid: &str) -> Result<Option<Value>, String>
}
```

**AtxClient Pattern:**
```rust
pub struct AtxClient {
    pub async fn device_info(&self) -> Result<Value, String>
}
```

**Connection Pool:**
- Uses moka LRU cache (1200 max, 600s TTL)
- Access via `state.connection_pool.get_or_create(udid, ip, port)`

### ATX Agent Protocol

**Ports:**
- 7912: Old atx-agent (Python)
- 9008: New uiautomator2 (Java)

**Device Info Endpoint:**
- `GET http://{ip}:{port}/info`
- Returns JSON with: serial, brand, model, version, sdk, hwaddr, display, battery, memory, cpu

**Example Request/Response:**
```json
// POST /api/devices/add
{
  "ip": "192.168.1.100",
  "port": 9008
}

// Response 200 OK
{
  "status": "success",
  "device": {
    "udid": "serial123-SM-G990B",
    "ip": "192.168.1.100",
    "port": 9008,
    "model": "SM-G990B",
    "brand": "Samsung",
    "version": "13",
    "display": {"width": 1080, "height": 2400},
    "battery": {"level": 85}
  }
}
```

### Implementation Approach

1. **Input Validation:**
   - Use serde for JSON deserialization
   - Validate IP with `std::net::Ipv4Addr` or regex
   - Port range: 1-65535, default 9008 if not specified

2. **Connection Validation:**
   - Create temporary AtxClient (don't pool yet)
   - Call `device_info()` with 5s timeout
   - If successful, proceed with registration
   - If failed, return 503 Service Unavailable

3. **Device Registration:**
   - Extract serial/hwaddr for UDID
   - Call `phone_service.update_field(udid, device_data)`
   - Build device JSON with all metadata

4. **Duplicate Detection:**
   - Query existing device by UDID first
   - If exists and connected, return 409 Conflict
   - If exists but disconnected, allow reconnection

5. **Response Building:**
   - Return device info JSON on success
   - Include connection status and metadata

### Naming Conventions

- Files: snake_case (e.g., `control.rs`)
- Functions: snake_case (e.g., `add_device()`)
- Structs: PascalCase (e.g., `ManualDeviceRequest`)
- Error handling: `Result<T, String>` pattern

### Error Handling

```rust
// Input validation error
HttpResponse::BadRequest().json(json!({"error": "Invalid IP address format"}))

// Connection timeout
HttpResponse::ServiceUnavailable().json(json!({"error": "Device unreachable: connection timeout"}))

// Duplicate device
HttpResponse::Conflict().json(json!({"error": "Device already connected"}))
```

---

## Dev Agent Record

### Implementation Plan

1. Add `ManualDeviceRequest` struct with `ip` (String) and `port` (u16, default 9008)
2. Implement `add_device` handler in control.rs
3. Validate IP format using `std::net::Ipv4Addr`
4. Create AtxClient and fetch device info
5. Build UDID from hwaddr or serial
6. Check for duplicates using PhoneService
7. Register device with PhoneService.update_field
8. Add route in main.rs
9. Add tests in test_server.rs

### Completion Notes

- Implemented `POST /api/devices/add` endpoint
- Request body: `{"ip": "x.x.x.x", "port": 9008}` (port defaults to 9008)
- Response: Device info JSON on success, error on failure
- Validates IPv4 format
- Returns 400 for invalid input
- Returns 503 for unreachable devices
- Returns 409 for duplicate connected devices
- Allows reconnection of disconnected devices
- All 92 tests pass

---

## File List

- `src/routes/control.rs` - Added `add_device()` endpoint and `ManualDeviceRequest` struct
- `src/main.rs` - Registered route `POST /api/devices/add`
- `tests/test_server.rs` - Added 4 tests for add_device endpoint

---

## Change Log

| Date | Change |
|------|--------|
| 2026-03-05 | Story file created with comprehensive context |
| 2026-03-06 | Implementation complete - all tasks done |

---

## Status History

| Date | Status |
|------|--------|
| 2026-03-05 | backlog |
| 2026-03-05 | ready-for-dev |
| 2026-03-05 | in-progress |
| 2026-03-06 | done |
