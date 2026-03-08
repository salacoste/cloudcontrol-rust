# Story 2.5: Screenshot Resizing

Status: done

## Story

As a **Remote Support Technician**, I want screenshots resized for my connection, so that I can work over slower networks.

## Acceptance Criteria

1. **Resize screenshot to half resolution**
   - Given a device has resolution 1080x2400
   - When I request screenshot with scale=0.5
   - Then the returned image is 540x1200
   - And the file size is proportionally smaller
   - And aspect ratio is preserved

2. **Resize screenshot to thumbnail**
   - Given I need a quick preview
   - When I request screenshot with scale=0.25
   - Then the returned image is 270x600 (for 1080x2400 device)
   - And the image loads quickly even on slow connections

3. **Preserve original size by default**
   - Given no scale parameter is provided
   - When I request a screenshot
   - Then the original device resolution is returned
   - And no resizing processing occurs

## Implementation Status

**ALREADY IMPLEMENTED** - Feature exists in codebase as part of Story 2-3 implementation.

### Verified Implementation

- `src/routes/control.rs:345-350` - Scale parameter parsing with defaults
  ```rust
  let scale: f64 = query
      .get("scale")
      .and_then(|v| v.parse::<f64>().ok())
      .unwrap_or(1.0)
      .max(0.25)
      .min(1.0);
  ```

- `src/services/device_service.rs:88-106` - `resize_jpeg()` function using image crate
  ```rust
  fn resize_jpeg(data: &[u8], quality: u8, scale: f64) -> Result<Vec<u8>, String> {
      let img = image::load_from_memory(data)?;
      let img = if scale < 1.0 {
          let new_w = (img.width() as f64 * scale) as u32;
          let new_h = (img.height() as f64 * scale) as u32;
          img.resize(new_w, new_h, image::imageops::FilterType::Nearest)
      } else {
          img
      };
      // ... encode to JPEG
  }
  ```

- `src/device/atx_client.rs` - `screenshot_scaled(scale, quality)` method for device-side compression

### Acceptance Criteria Met

- ✅ Scale parameter: `?scale=0.5` returns half resolution
- ✅ Thumbnail scale: `?scale=0.25` returns quarter resolution
- ✅ Default scale: 1.0 (no resizing)
- ✅ Scale range: 0.25 to 1.0 (clamped)
- ✅ Aspect ratio preserved (proportional resize)
- ✅ File size reduction proportional to scale

### API Usage

```
GET /inspector/{udid}/screenshot?scale=0.5
GET /inspector/{udid}/screenshot/img?scale=0.25
POST /api/screenshot/batch {"devices": [...], "scale": 0.5}
```

## Tasks / Subtasks

- [x] Task 1: Scale parameter support (AC: 1, 2, 3)
  - [x] Parse scale query parameter from request
  - [x] Clamp scale to valid range (0.25-1.0)
  - [x] Default to 1.0 (no resizing) when not specified

- [x] Task 2: Image resizing implementation (AC: 1, 2)
  - [x] Implement resize_jpeg() using image crate
  - [x] Use Nearest filter for fast resizing
  - [x] Preserve aspect ratio in resize calculations

- [x] Task 3: E2E tests
  - [x] Test batch screenshot with quality and scale
  - [x] Test scale clamping (out of range values)

## Dev Notes

### Architecture Constraints

- Uses `image` crate for JPEG decoding/encoding
- Resizing uses `FilterType::Nearest` for performance
- Fire-and-forget pattern not needed (synchronous operation)
- Scale parameter available in all screenshot endpoints

### Implementation Details

The resize operation:
1. Decode JPEG bytes to image buffer
2. Calculate new dimensions: `new_w = width * scale`, `new_h = height * scale`
3. Resize using nearest-neighbor interpolation (fast)
4. Re-encode to JPEG with specified quality

### Performance Considerations

- NFR1: Screenshot capture latency <500ms end-to-end
- Resizing is CPU-intensive but faster than transferring larger images over slow networks
- Device-side compression (ATX Agent) preferred when available

### References

- [Source: src/routes/control.rs:345-350] - Scale parameter parsing
- [Source: src/services/device_service.rs:88-106] - resize_jpeg implementation
- [Source: src/device/atx_client.rs] - screenshot_scaled method
- [Source: tests/test_server.rs:584-645] - Scale tests
- [Source: _bmad-output/implementation-artifacts/2-3-configurable-screenshot-quality.md] - Related story

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

None - feature already implemented.

### Completion Notes List

1. **Feature Already Exists**: Screenshot resizing was implemented as part of Story 2-3 (Configurable Screenshot Quality)
2. **Scale Parameter**: Available in all screenshot endpoints with range 0.25-1.0
3. **Tests Exist**: E2E tests for scale clamping already in test_server.rs

### File List

- `src/routes/control.rs` - Screenshot endpoints with scale parameter
- `src/services/device_service.rs` - resize_jpeg() implementation
- `src/device/atx_client.rs` - screenshot_scaled() method
- `tests/test_server.rs` - E2E tests for scale functionality
