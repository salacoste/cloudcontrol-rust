---
stepsCompleted: ['step-01-validate-prerequisites', 'step-02-design-epics', 'step-03-create-stories', 'step-04-final-validation']
inputDocuments:
  - _bmad-output/planning-artifacts/prd.md
  - _bmad-output/architecture.md
  - _bmad-output/planning-artifacts/ux-design-specification.md
workflowType: 'create-epics-and-stories'
lastUpdated: '2026-03-05'
epicCount: 7
mvpEpicCount: 5
totalStories: 39
storiesFile: '_bmad-output/planning-artifacts/epics-stories.md'
validationStatus: 'COMPLETE'
---

# cloudcontrol-rust - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for cloudcontrol-rust, decomposing the requirements from the PRD, UX Design, and Architecture requirements into implementable stories.

## Requirements Inventory

### Functional Requirements (38 Total)

**Device Connection & Discovery (FR1-FR6):**
- FR1: System can discover Android devices connected via WiFi on port 7912/9008
- FR2: System can discover Android devices connected via USB with automatic ADB forwarding
- FR3: System can connect to devices running ATX Agent protocol
- FR4: Users can manually add WiFi devices by IP address and port
- FR5: System can detect when devices disconnect and update status accordingly
- FR6: System can reconnect to devices automatically after network interruptions

**Device Management & Monitoring (FR7-FR12):**
- FR7: Users can view a list of all connected devices with status indicators
- FR8: Users can view device metadata (model, Android version, battery level, screen resolution)
- FR9: System can persist device state across server restarts
- FR10: Users can tag and label devices for organization
- FR11: System can display device connection history and uptime statistics
- FR12: Users can disconnect individual devices from the management interface

**Screenshot & Screen Streaming (FR13-FR18):**
- FR13: Users can request a screenshot from any connected device
- FR14: Users can stream real-time screenshots from devices via WebSocket
- FR15: System can capture screenshots at configurable quality levels
- FR16: Users can request screenshots from multiple devices simultaneously
- FR17: System can resize screenshots for bandwidth optimization
- FR18: Users can download screenshots as JPEG or PNG files

**Remote Control Operations (FR19-FR24):**
- FR19: Users can send tap commands to specific screen coordinates on devices
- FR20: Users can send swipe gestures with configurable direction and duration
- FR21: Users can input text into focused text fields on devices
- FR22: Users can send physical key events (home, back, volume, power)
- FR23: Users can view UI hierarchy inspector for accessibility debugging
- FR24: Users can execute shell commands on connected devices

**Batch Operations (FR25-FR29):**
- FR25: Users can select multiple devices for synchronized operations
- FR26: Users can execute the same tap/swipe/input across all selected devices
- FR27: System can record user actions for batch replay
- FR28: Users can start and stop recording sessions across multiple devices
- FR29: System can export batch test reports with per-device results

**API & Integration (FR30-FR34):**
- FR30: External applications can connect to devices via REST API
- FR31: External applications can stream screenshots via WebSocket API
- FR32: System can provide device status and health via API endpoints
- FR33: CI/CD pipelines can integrate for automated screenshot capture
- FR34: System can accept JSON-RPC commands over WebSocket

**Scrcpy Integration - Post-MVP (FR35-FR38):**
- FR35: Users can start high-fidelity screen mirroring via scrcpy
- FR36: Users can control devices through scrcpy video stream
- FR37: System can relay scrcpy H.264 video via WebSocket
- FR38: Users can record scrcpy sessions for later review

### Non-Functional Requirements (18 Total)

**Performance (NFR1-NFR6):**
- NFR1: Screenshot capture latency (HTTP) <500ms end-to-end
- NFR2: Screenshot streaming latency (WebSocket) <200ms per frame
- NFR3: API response time (non-streaming) <100ms
- NFR4: Device connection establishment <3s
- NFR5: Batch operation execution <50ms per device
- NFR6: UI hierarchy inspection <2s to load

**Reliability (NFR7-NFR10):**
- NFR7: System uptime during business hours 99.5%+
- NFR8: Device reconnection after network recovery automatic within 30s
- NFR9: WebSocket connection stability - no drops in 1-hour session
- NFR10: Memory stability <500MB for 50 devices

**Scalability (NFR11-NFR14):**
- NFR11: Concurrent connected devices 50+ per server
- NFR12: WebSocket concurrent streams 100+ per server
- NFR13: Connection pool capacity 1200 entries
- NFR14: Screenshot cache hit rate >80%

**Integration (NFR15-NFR18):**
- NFR15: ATX Agent protocol compatibility - Python uiautomator2 compatible
- NFR16: ADB command execution <1s for standard commands
- NFR17: scrcpy video stream latency <100ms
- NFR18: REST API compatibility - OpenAPI 3.0 spec compliant

### Additional Requirements

**From Architecture:**
- Existing stack: Rust 2021, actix-web 4.x, tokio 1.x, sqlx 0.8, moka 0.12, tera 1.x
- Service layer pattern with `Result<T, String>` error handling
- Connection pooling with moka LRU cache (1200 max, 600s TTL)
- Brownfield project - no new starter template required
- 90 existing tests (52 unit, 38 integration, 20 E2E)
- No authentication required (internal API)
- Project structure: db/, device/, models/, pool/, routes/, services/, utils/

**From UX Design:**
- Design system: Tailwind CSS 3.x
- Platform: Desktop-first web browser (1280px minimum)
- Device grid dashboard with status badges (green/yellow/red)
- Keyboard shortcuts for power users
- Screenshot preview panel with live streaming
- Batch operation visualization with progress indicators
- Browser support: Chrome 90+, Firefox 88+, Safari 14+, Edge 90+

### FR Coverage Map

| FR | Epic | Description |
|----|------|-------------|
| FR1 | Epic 1A | WiFi device discovery on port 7912/9008 |
| FR2 | Epic 1A | USB device discovery with ADB forwarding |
| FR3 | Epic 1A | ATX Agent protocol connection |
| FR4 | Epic 1A | Manual WiFi device addition |
| FR5 | Epic 1A | Disconnect detection |
| FR6 | Epic 1A | Auto-reconnection after network recovery |
| FR7 | Epic 1B | Device list with status indicators |
| FR8 | Epic 1B | Device metadata display |
| FR9 | Epic 1B | State persistence across restarts |
| FR10 | Epic 1B | Device tagging/labeling |
| FR11 | Epic 1B | Connection history & uptime |
| FR12 | Epic 1B | Manual disconnect from UI |
| FR13 | Epic 2 | Single screenshot capture |
| FR14 | Epic 2 | Real-time WebSocket streaming |
| FR15 | Epic 2 | Configurable quality levels |
| FR16 | Epic 2 | Multi-device screenshots |
| FR17 | Epic 2 | Screenshot resizing |
| FR18 | Epic 2 | Download as JPEG/PNG |
| FR19 | Epic 3 | Tap commands |
| FR20 | Epic 3 | Swipe gestures |
| FR21 | Epic 3 | Text input |
| FR22 | Epic 3 | Key events (home, back, volume, power) |
| FR23 | Epic 3 | UI hierarchy inspector |
| FR24 | Epic 3 | Shell command execution |
| FR25 | Epic 4 | Multi-device selection |
| FR26 | Epic 4 | Synchronized operations |
| FR27 | Epic 4 | Action recording |
| FR28 | Epic 4 | Recording session control |
| FR29 | Epic 4 | Batch test reports |
| FR30 | Epic 5 | REST API for device operations |
| FR31 | Epic 5 | WebSocket API for streaming |
| FR32 | Epic 5 | Device status API |
| FR33 | Epic 5 | CI/CD integration |
| FR34 | Epic 5 | JSON-RPC commands |
| FR35 | Epic 6 | Scrcpy screen mirroring |
| FR36 | Epic 6 | Scrcpy device control |
| FR37 | Epic 6 | H.264 WebSocket relay |
| FR38 | Epic 6 | Session recording |

## Epic List

### Epic 1A: Device Connection & Discovery (MVP)

**User Outcome:** System automatically discovers and maintains stable connections to Android devices via WiFi and USB.

**Business Value:** Enables the foundational connectivity layer that all other features depend on. Without reliable device connections, no other functionality is possible.

**FRs Covered:** FR1-FR6

**Key Stories:**
- WiFi device discovery (mDNS/network scanning on port 7912/9008)
- USB device discovery with automatic ADB port forwarding
- ATX Agent protocol handshake and authentication
- Manual device addition by IP:port
- Connection health monitoring and disconnect detection
- Automatic reconnection after network recovery

**Dependencies:** None (foundational epic)

---

### Epic 1B: Device Dashboard & Management (MVP)

**User Outcome:** Users view all connected devices in a unified dashboard with status indicators and management capabilities.

**Business Value:** Provides the primary interface for device farm management. QA engineers can see device health at a glance and operators can manage the fleet efficiently.

**FRs Covered:** FR7-FR12

**Key Stories:**
- Device grid dashboard with status badges (green/yellow/red)
- Device metadata panel (model, Android version, battery, resolution)
- SQLite persistence for device state
- Device tagging and labeling system
- Connection history and uptime statistics
- Manual disconnect functionality

**Dependencies:** Epic 1A (requires connected devices to display)

---

### Epic 2: Real-Time Visual Monitoring (MVP)

**User Outcome:** Users can view device screens in real-time and capture screenshots for analysis and debugging.

**Business Value:** Enables visual verification of device state, critical for debugging issues across multiple devices without physical access.

**FRs Covered:** FR13-FR18

**Key Stories:**
- Single screenshot capture via HTTP
- Real-time WebSocket screenshot streaming
- Configurable JPEG quality levels
- Multi-device screenshot batch capture
- Screenshot resizing for bandwidth optimization
- Download screenshots as JPEG or PNG files

**Dependencies:** Epic 1A (connected devices), Epic 1B (device selection in dashboard)

---

### Epic 3: Remote Device Control (MVP)

**User Outcome:** Users can remotely interact with devices using touch gestures, text input, key events, and shell commands.

**Business Value:** Enables complete remote control of devices, allowing QA engineers to test applications and support technicians to troubleshoot issues without physical device access.

**FRs Covered:** FR19-FR24

**Key Stories:**
- Tap command with coordinate targeting
- Swipe gestures with direction and duration
- Text input into focused fields
- Physical key events (home, back, volume, power)
- UI hierarchy inspector for accessibility debugging
- Shell command execution interface

**Dependencies:** Epic 1A (connected devices), Epic 2 (visual feedback for control actions)

---

### Epic 4: Multi-Device Batch Operations (MVP)

**User Outcome:** QA engineers can test multiple devices simultaneously with synchronized actions and export test reports.

**Business Value:** Multiplies testing efficiency by allowing single actions to execute across all selected devices simultaneously. Transforms hours of manual testing into minutes.

**FRs Covered:** FR25-FR29

**Key Stories:**
- Multi-device selection UI (click, ctrl-click, shift-click)
- Synchronized tap/swipe/input across selected devices
- Action recording and playback
- Recording session management (start/stop/pause)
- Batch test report export with per-device results

**Dependencies:** Epic 1A (connected devices), Epic 2 (screenshot feedback), Epic 3 (control operations)

---

### Epic 5: External API & CI/CD Integration (Growth)

**User Outcome:** Automation engineers can integrate cloudcontrol with CI/CD pipelines and external tools via REST and WebSocket APIs.

**Business Value:** Enables automation of device testing in CI/CD pipelines, supporting continuous testing practices and reducing manual intervention.

**FRs Covered:** FR30-FR34

**Key Stories:**
- REST API endpoints for all device operations
- WebSocket API for real-time screenshot streaming
- Device status and health API endpoints
- CI/CD pipeline integration examples
- JSON-RPC command interface over WebSocket

**Dependencies:** Epic 1A-4 (APIs expose existing functionality)

**Phase:** Growth (Post-MVP enhancement)

---

### Epic 6: High-Fidelity Screen Mirroring (Post-MVP)

**User Outcome:** Users can view high-fidelity video streams of device screens with recording capability for detailed analysis.

**Business Value:** Provides superior visual quality for scenarios where screenshots are insufficient, such as debugging animations or video playback issues.

**FRs Covered:** FR35-FR38

**Key Stories:**
- Scrcpy session management (start/stop)
- Device control through scrcpy video stream
- H.264 video relay via WebSocket
- Session recording and playback

**Dependencies:** Epic 1A-4 (builds on existing device infrastructure)

---

# Epic 5: External API & CI/CD Integration - Stories

EOF
