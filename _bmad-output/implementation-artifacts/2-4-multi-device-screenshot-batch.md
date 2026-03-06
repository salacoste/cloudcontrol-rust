# Story 2.4: Multi-Device Screenshot Batch

Status: ready-for-dev

## Story

As a **QA Engineer**, I want to capture screenshots from multiple devices at once, so that I can compare states across my test fleet.

## Acceptance Criteria

1. **Capture screenshots from multiple devices**
   - Given 5 devices are connected and selected
   - When I request POST /screenshot/batch with the device UDIDs
   - Then screenshots from all 5 devices are returned
   - And each screenshot is keyed by device UDID
   - And total response time is under 2 seconds

2. **Handle partial failures in batch**
   - Given 5 devices are selected
   - And device 3 is disconnected
   - When I request batch screenshot
   - Then 4 successful screenshots are returned
   - And device 3 has error "ERR_DEVICE_DISCONNECTED"
   - And HTTP 207 Multi-Status is returned

3. **Progress indicator for batch capture**
   - Given 10 devices are selected for batch screenshot
   - When the batch operation starts
   - Then progress events are emitted via WebSocket
   - And UI shows completion percentage

## Tasks / Subtasks

- [ ] Task 1: Create batch screenshot API endpoint (AC: 1, 2)
  - [ ] Add POST /api/screenshot/batch route
  - [ ] Accept JSON body with array of UDIDs
  - [ ] Execute concurrent screenshot requests using tokio::join!
  - [ ] Return JSON with device-keyed results

- [ ] Task 2: Implement error handling for partial failures (AC: 2)
  - [ ] Track success/failure per device
  - [ ] Return HTTP 207 Multi-Status for partial success
  - [ ] Include error codes per failed device

- [ ] Task 3: Add WebSocket progress events (AC: 3)
  - [ ] Emit progress events during batch operation
  - [ ] Include device count and completion status

- [ ] Task 4: Add E2E tests
  - [ ] Test batch success with 5 devices
  - [ ] Test partial failure with 1 disconnected device
  - [ ] Test all devices disconnected

## Dev Notes

### Architecture Constraints

- Use existing `PhoneService` and `DeviceService` patterns
- Leverage existing `screenshot_scaled()` method from ATX client
- Follow existing error handling patterns with `Result<T, String>`

### Existing Patterns to Follow

- Screenshot endpoint: `src/routes/control.rs:293-356` (inspector_screenshot)
- Error responses: `HttpResponse::InternalServerError().json(json!({...}))`
- Device lookup: `phone_service.query_info_by_udid(&udid).await`

### API Design

```
POST /api/screenshot/batch
Content-Type: application/json

{
  "devices": ["udid1", "udid2", "udid3"],
  "quality": 70,
  "scale": 1.0
}

Response (207 Multi-Status):
{
  "status": "partial",
  "results": {
    "udid1": {"status": "success", "data": "base64...", "type": "jpeg"},
    "udid2": {"status": "success", "data": "base64...", "type": "jpeg"},
    "udid3": {"status": "error", "error": "ERR_DEVICE_DISCONNECTED"}
  }
}
```

### Performance Requirements

- NFR5: Batch operation execution <50ms per device
- Total response time under 2 seconds for 5 devices

### Project Structure Notes

- Route handler in `src/routes/control.rs`
- Service logic can remain in route or add to `DeviceService`
- Tests in `tests/test_server.rs`

### References

- [Source: src/routes/control.rs:293-356] - Single screenshot endpoint pattern
- [Source: src/device/atx_client.rs:86] - screenshot_scaled method
- [Source: src/services/device_service.rs:88] - resize_jpeg for quality
- [Source: _bmad-output/planning-artifacts/epics-stories.md:508-540] - Story definition

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List
