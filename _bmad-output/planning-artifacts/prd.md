---
stepsCompleted: ['step-01-init', 'step-02-discovery', 'step-02b-vision', 'step-02c-executive-summary', 'step-03-success', 'step-04-journeys', 'step-05-domain', 'step-06-innovation', 'step-07-project-type', 'step-08-scoping', 'step-09-functional', 'step-10-nonfunctional', 'step-11-polish', 'step-e-01-discovery', 'step-e-02-review', 'step-e-03-edit']
inputDocuments:
  - docs/index.md
  - docs/architecture.md
  - docs/data-models.md
  - docs/api-endpoints.md
  - docs/services.md
  - docs/shared-code.md
  - docs/configuration.md
  - docs/tests.md
  - docs/deployment.md
  - _bmad-output/project-context.md
  - _bmad-output/architecture.md
documentCounts:
  briefs: 0
  research: 0
  brainstorming: 0
  projectDocs: 11
workflowType: 'prd'
projectContext: 'brownfield'
classification:
  projectType: 'Web App + API Backend'
  domain: 'Device Automation / IoT Management'
  complexity: 'Medium'
---

# Product Requirements Document - cloudcontrol-rust

**Author:** R2d2
**Date:** 2026-03-05

---

## Executive Summary

**cloudcontrol-rust** is a production-grade Android device management platform enabling remote control and monitoring of phone fleets via WiFi or USB. A Rust rewrite of the Python CloudControl, it delivers low-latency screen streaming, batch operations, and unified device control through a single web interface.

**Target Users:** Mobile QA teams, device farm operators, remote support teams, and automation engineers requiring scalable multi-device management.

**Problem Solved:** Fragmented tooling for Android device control—users previously juggled ADB commands, separate screen mirroring tools, and manual device switching. cloudcontrol-rust unifies ATX Agent protocol, ADB integration, and scrcpy streaming into one cohesive platform with sub-second screenshot latency.

### What Makes This Special

- **Unified Protocol Stack:** ATX Agent (HTTP/WebSocket), ADB (USB/forwarded), and scrcpy (H.264 streaming) integrated in one codebase—not stitched together via scripts
- **Rust Performance:** Zero-cost abstractions, async I/O via tokio, moka-based connection pooling (1200 connections, 600s TTL) delivering production reliability the Python prototype couldn't achieve
- **Real-time Screenshot Pipeline:** Binary WebSocket streaming with JSON-RPC control, WiFi direct connection bypassing ADB forward overhead
- **Dual Connectivity:** Seamless WiFi (port 7912/9008) and USB device support with automatic detection and ADB forwarding

## Project Classification

| Attribute | Value |
|-----------|-------|
| **Project Type** | Web App + API Backend |
| **Domain** | Device Automation / IoT Management |
| **Complexity** | Medium |
| **Project Context** | Brownfield (Rust rewrite of Python prototype) |

---

## Success Criteria

### User Success

- **Device Management Efficiency:** Users can view and control 10+ devices simultaneously without UI lag
- **Screenshot Latency:** Sub-second screen updates during real-time monitoring (<500ms end-to-end)
- **Connection Reliability:** Devices remain connected during extended sessions (hours, not minutes)
- **Batch Operations:** Single action triggers across all selected devices with visual confirmation
- **Remote Inspector:** UI hierarchy loads in <2s, touch/click commands execute in <1s

### Business Success

- **Stability:** 99.5%+ uptime during business hours for device farm operations
- **Python Parity:** Feature-complete with original Python CloudControl within 6 months
- **Performance:** 3x faster screenshot throughput vs Python prototype (measured: frames/second)
- **Adoption:** Existing CloudControl users migrate without workflow changes

### Technical Success

- **Test Coverage:** All 90 tests passing (52 unit, 38 integration, 20 E2E)
- **Connection Pool:** 1200 concurrent connections stable with 600s TTL
- **Memory Efficiency:** <500MB RAM baseline for 50 connected devices
- **WebSocket Stability:** No dropped connections during 1-hour stress test

### Measurable Outcomes

| Metric | Target | Measurement |
|--------|--------|-------------|
| Screenshot latency | <500ms | End-to-end timing logs |
| Concurrent devices | 50+ | Load test with stable connections |
| Test pass rate | 100% | CI/CD pipeline |
| Memory per device | <10MB | Process monitoring |
| Connection pool utilization | <80% | moka cache metrics |

## Product Scope

### MVP - Minimum Viable Product

- WiFi device connection and management
- Real-time screenshot streaming via WebSocket
- Basic remote control (tap, swipe, text input)
- Device list with status indicators
- USB device detection and ADB forwarding
- SQLite persistence for device state

### Growth Features (Post-MVP)

- Scrcpy high-fidelity screen mirroring
- Batch operation templates and scheduling
- Device grouping and tagging
- Performance analytics dashboard
- Multi-user access control
- API rate limiting and quotas

### Vision (Future)

- AI-powered automated testing scripts
- Cross-platform support (iOS via WebDriver)
- Cloud deployment with horizontal scaling
- Plugin architecture for custom integrations
- Enterprise SSO and audit logging

---

## User Journeys

### Journey 1: QA Engineer - Mobile App Testing

**Persona:** Alex is a QA engineer at a mobile game studio. Every release cycle, they need to test the app across 15 different Android devices to catch device-specific bugs.

**Opening Scene:** It's Tuesday morning. Alex has a new build to test before Thursday's release. Previously, this meant walking between three desks with different phones, constantly plugging/unplugging USB cables, and manually running the same test script on each device.

**Rising Action:** Alex opens cloudcontrol-rust in their browser. The dashboard shows all 15 devices connected—some via WiFi in the test rack, others plugged into USB hubs. They select all devices and click "Start Recording."

**Climax:** Alex taps through the game's tutorial on one device. Simultaneously, all 15 devices mirror the same actions. Real-time screenshots stream in from each device, revealing that one budget phone is rendering textures incorrectly.

**Resolution:** The bug is caught before release. Alex exports the batch test report showing the specific device model with issues. The 4-hour manual testing process now takes 45 minutes.

---

### Journey 2: Device Farm Operator - Infrastructure Management

**Persona:** Sam manages a device farm with 50 phones for a testing services company. Their job is keeping all devices online, charged, and ready for client testing sessions.

**Opening Scene:** Monday 8 AM. Sam arrives to find three devices showing "disconnected" status from last week. Previously, this meant physically checking each device, running ADB commands, and restarting services manually.

**Rising Action:** Sam opens cloudcontrol-rust admin panel. The device list shows battery levels, connection status, and last-seen timestamps for all 50 devices. Three phones are flagged red—two have low battery, one has a crashed ATX agent.

**Climax:** With one click, Sam restarts the ATX agent on the crashed device remotely. For the low-battery phones, Sam tags them for maintenance and reroutes incoming test sessions to other devices.

**Resolution:** The device farm is back to 100% availability within 10 minutes—no walking the racks, no manual ADB commands. Sam gets an alert when battery levels recover after charging.

---

### Journey 3: Remote Support Technician

**Persona:** Jordan works at a company that remotely manages kiosk devices deployed in retail stores across the country. When a store reports an issue, Jordan needs to see what's happening on the device.

**Opening Scene:** A retail store calls—their payment kiosk is showing a blank screen after the latest update. The store is 2,000 miles away. Previously, Jordan could only guess the issue or schedule an on-site technician.

**Rising Action:** Jordan opens cloudcontrol-rust, searches for the device by store ID, and initiates a remote inspection session. The device's screen appears in real-time, showing the app frozen on a loading screen.

**Climax:** Jordan uses the UI hierarchy inspector to see that a "network timeout" dialog is hidden behind the frozen loading screen. They remotely tap the invisible "retry" button through cloudcontrol-rust.

**Resolution:** The kiosk recovers instantly. Jordan documents the issue and schedules a permanent fix for the next app update. No on-site visit needed—the 4-hour problem is solved in 10 minutes.

---

### Journey 4: Automation Engineer - CI/CD Integration

**Persona:** Morgan is building automated test pipelines for their company's mobile app. They need screenshots and UI state from devices as part of the regression test suite.

**Opening Scene:** The CI/CD pipeline needs to capture screenshots from 5 devices after each automated test run. Morgan previously wrote custom ADB scripts that were brittle and slow.

**Rising Action:** Morgan integrates with cloudcontrol-rust's REST API. The test runner calls `/api/wifi-connect` to ensure devices are ready, then `/inspector/{udid}/screenshot` for high-speed captures.

**Climax:** The pipeline runs 200 test cases across 5 devices in parallel. Screenshots are captured in <200ms each via the binary WebSocket channel, not the 2-3 seconds ADB required.

**Resolution:** Test suite runtime drops from 45 minutes to 12 minutes. Morgan adds screenshot comparison to catch visual regressions automatically, all powered by cloudcontrol-rust's fast capture API.

---

### Journey Requirements Summary

| Journey | Key Capabilities Revealed |
|---------|---------------------------|
| QA Engineer | Batch operations, real-time streaming, multi-device sync, test reporting |
| Device Farm Operator | Device monitoring, remote agent management, tagging/routing, alerts |
| Remote Support | Device search, UI hierarchy inspection, remote control, session logging |
| Automation Engineer | REST API, WebSocket screenshots, connection management, parallel capture |

---

## Web App + API Backend Requirements

### Architecture Overview

cloudcontrol-rust is a dual-nature application:
- **Web Application:** Browser-based UI for device management and control
- **API Backend:** REST + WebSocket endpoints for programmatic access

### API Specification

**HTTP Endpoints (25+ routes):**

| Category | Endpoints | Purpose |
|----------|-----------|---------|
| Device Management | `/list`, `/device/{udid}`, `/wifi-connect` | Device discovery and connection |
| Screenshot | `/inspector/{udid}/screenshot`, `/screenshot/batch` | Real-time screen capture |
| Remote Control | `/tap`, `/swipe`, `/input`, `/key` | Device input commands |
| Scrcpy | `/scrcpy/{udid}/start`, `/scrcpy/{udid}/stop` | Screen mirroring control |
| System | `/`, `/async`, `/nio/stats` | Health and monitoring |

**WebSocket Endpoints (3 routes):**

| Endpoint | Protocol | Purpose |
|----------|----------|---------|
| `/ws/screenshot/{udid}` | Binary WebSocket | Real-time screenshot stream |
| `/ws/nio` | JSON-RPC | NIO protocol commands |
| `/ws/scrcpy/{udid}` | H.264 stream | Scrcpy video relay |

### Data Formats

- **Request:** JSON for HTTP, JSON-RPC 2.0 for WebSocket control
- **Response:** JSON for HTTP, binary frames for screenshot streaming
- **Screenshot:** JPEG (default) or PNG, base64-encoded for HTTP, raw binary for WebSocket

### Real-time Architecture

- **WebSocket-based streaming:** Binary frames for low-latency screenshot delivery
- **JSON-RPC control:** Structured command protocol over WebSocket
- **Heartbeat mechanism:** DashMap-tracked sessions with periodic ping/pong
- **Connection pooling:** moka LRU cache (1200 max, 600s TTL) for AtxClient instances

### Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Screenshot latency (HTTP) | <500ms | End-to-end including encode |
| Screenshot latency (WebSocket) | <200ms | Binary streaming mode |
| Concurrent devices | 50+ | Tested with stable connections |
| API response time | <100ms | Non-streaming endpoints |
| WebSocket connection capacity | 100+ | Per server instance |

### Browser Support

- **Target:** Modern browsers (Chrome 90+, Firefox 88+, Safari 14+, Edge 90+)
- **Requirements:** WebSocket support, ES6+, Fetch API
- **Not Supported:** IE11, legacy mobile browsers

### Implementation Notes

- **Async runtime:** tokio 1.x with full features
- **Web framework:** actix-web 4.x with actix-ws 0.3
- **Database:** SQLite via sqlx 0.8 (compile-time verification)
- **Template engine:** tera 1.x for HTML rendering
- **Deployment:** Single binary, no external runtime dependencies

---

## Project Scoping & Phased Development

### MVP Strategy & Philosophy

**MVP Approach:** Platform Parity MVP - Achieve feature completeness with Python prototype while delivering performance improvements
**Resource Requirements:** 1-2 Rust developers, Android device testing infrastructure

### MVP Feature Set (Phase 1)

**Core User Journeys Supported:**
- Journey 1: QA Engineer batch testing
- Journey 4: Automation Engineer CI/CD integration

**Must-Have Capabilities:**
- WiFi device connection and management
- Real-time screenshot streaming via WebSocket
- Basic remote control (tap, swipe, text input)
- Device list with status indicators
- USB device detection and ADB forwarding
- SQLite persistence for device state

### Post-MVP Features

**Phase 2 (Growth):**
- Scrcpy high-fidelity screen mirroring
- Batch operation templates and scheduling
- Device grouping and tagging
- Performance analytics dashboard
- Multi-user access control
- API rate limiting and quotas

**Phase 3 (Expansion):**
- AI-powered automated testing scripts
- Cross-platform support (iOS via WebDriver)
- Cloud deployment with horizontal scaling
- Plugin architecture for custom integrations
- Enterprise SSO and audit logging

### Risk Mitigation Strategy

| Risk Type | Mitigation Approach |
|-----------|---------------------|
| **Technical** | 90 existing tests validate core functionality; async Rust patterns proven in similar projects |
| **Market** | Existing Python user base provides adoption path; feature parity reduces migration friction |
| **Resource** | Single-binary deployment simplifies operations; SQLite eliminates external database dependency |

---

## Functional Requirements

### Device Connection & Discovery

- **FR1:** System can discover Android devices connected via WiFi on port 7912/9008
- **FR2:** System can discover Android devices connected via USB with automatic ADB forwarding
- **FR3:** System can connect to devices running ATX Agent protocol
- **FR4:** Users can manually add WiFi devices by IP address and port
- **FR5:** System can detect when devices disconnect and update status accordingly
- **FR6:** System can reconnect to devices automatically after network interruptions

### Device Management & Monitoring

- **FR7:** Users can view a list of all connected devices with status indicators
- **FR8:** Users can view device metadata (model, Android version, battery level, screen resolution)
- **FR9:** System can persist device state across server restarts
- **FR10:** Users can tag and label devices for organization
- **FR11:** System can display device connection history and uptime statistics
- **FR12:** Users can disconnect individual devices from the management interface

### Screenshot & Screen Streaming

- **FR13:** Users can request a screenshot from any connected device
- **FR14:** Users can stream real-time screenshots from devices via WebSocket
- **FR15:** System can capture screenshots at configurable quality levels
- **FR16:** Users can request screenshots from multiple devices simultaneously
- **FR17:** System can resize screenshots for bandwidth optimization
- **FR18:** Users can download screenshots as JPEG or PNG files

### Remote Control Operations

- **FR19:** Users can send tap commands to specific screen coordinates on devices
- **FR20:** Users can send swipe gestures with configurable direction and duration
- **FR21:** Users can input text into focused text fields on devices
- **FR22:** Users can send physical key events (home, back, volume, power)
- **FR23:** Users can view UI hierarchy inspector for accessibility debugging
- **FR24:** Users can execute shell commands on connected devices

### Batch Operations

- **FR25:** Users can select multiple devices for synchronized operations
- **FR26:** Users can execute the same tap/swipe/input across all selected devices
- **FR27:** System can record user actions for batch replay
- **FR28:** Users can start and stop recording sessions across multiple devices
- **FR29:** System can export batch test reports with per-device results

### API & Integration

- **FR30:** External applications can connect to devices via REST API
- **FR31:** External applications can stream screenshots via WebSocket API
- **FR32:** System can provide device status and health via API endpoints
- **FR33:** CI/CD pipelines can integrate for automated screenshot capture
- **FR34:** System can accept JSON-RPC commands over WebSocket

### Scrcpy Integration (Post-MVP)

- **FR35:** Users can start high-fidelity screen mirroring via scrcpy
- **FR36:** Users can control devices through scrcpy video stream
- **FR37:** System can relay scrcpy H.264 video via WebSocket
- **FR38:** Users can record scrcpy sessions for later review

---

## Non-Functional Requirements

### Performance

| ID | Requirement | Target |
|----|-------------|--------|
| NFR1 | Screenshot capture latency (HTTP) | <500ms end-to-end |
| NFR2 | Screenshot streaming latency (WebSocket) | <200ms per frame |
| NFR3 | API response time (non-streaming) | <100ms |
| NFR4 | Device connection establishment | <3s |
| NFR5 | Batch operation execution | <50ms per device |
| NFR6 | UI hierarchy inspection | <2s to load |

### Reliability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR7 | System uptime during business hours | 99.5%+ |
| NFR8 | Device reconnection after network recovery | Automatic within 30s |
| NFR9 | WebSocket connection stability | No drops in 1-hour session |
| NFR10 | Memory stability | <500MB for 50 devices |

### Scalability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR11 | Concurrent connected devices | 50+ per server |
| NFR12 | WebSocket concurrent streams | 100+ per server |
| NFR13 | Connection pool capacity | 1200 entries |
| NFR14 | Screenshot cache efficiency | >80% hit rate |

### Integration

| ID | Requirement | Target |
|----|-------------|--------|
| NFR15 | ATX Agent protocol compatibility | Python uiautomator2 compatible |
| NFR16 | ADB command execution | <1s for standard commands |
| NFR17 | scrcpy video stream latency | <100ms |
| NFR18 | REST API compatibility | OpenAPI 3.0 spec compliant |

**NFR19: Accessibility Compliance**

| Requirement | Target |
|------------|--------|
| WCAG 2.1 Level A | Desktop web application meets WCAG 2.1 A requirements |
| Keyboard navigation | All actions accessible via keyboard |
| Screen reader compatibility | ARIA labels and semantic HTML |
| Color contrast | 4.5:1 ratio minimum |
| Non-text alternatives | Alt text for icons and images |
| Resizable layouts | 200% zoom without horizontal scrolling |

**NFR20: API Error Response Standardization**

All API endpoints return standardized error responses:

| Error Code | HTTP Status | Description |
|------------|-------------|-------------|
| `ERR_DEVICE_NOT_FOUND` | 404 | Device UDID not in system |
| `ERR_DEVICE_DISCONNECTED` | 503 | Device exists but connection lost |
| `ERR_INVALID_REQUEST` | 400 | Malformed request body/parameters |
| `ERR_OPERATION_FAILED` | 500 | Device operation failed |

**NFR21: API Versioning Strategy**

- URL-based versioning: `/api/v1/...` endpoints
- Backward compatibility via version in URL
- Breaking changes require major version bump
- Current version: v1 (MVP)
- Future versions: v2 (Growth), v3 (Enterprise)

