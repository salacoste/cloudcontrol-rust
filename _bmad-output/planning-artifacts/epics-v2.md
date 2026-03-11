---
stepsCompleted: ["step-01-validate-prerequisites", "step-02-design-epics", "step-03-create-stories", "step-04-final-validation"]
inputDocuments:
  - "_bmad-output/planning-artifacts/prd.md"
  - "_bmad-output/architecture.md"
  - "Code review analysis (2026-03-10)"
  - "Legacy feature analysis (2026-03-10)"
---

# cloudcontrol-rust - Epic Breakdown (Phase 2: Legacy Modernization & Infrastructure)

## Overview

This document provides the epic and story breakdown for the second development phase of cloudcontrol-rust. Phase 1 (39 stories, 7 epics) delivered all core functionality. Phase 2 focuses on legacy modernization, completing unfinished features, hardening infrastructure, and improving code quality.

Source: Comprehensive code review and legacy analysis performed 2026-03-10.

## Requirements Inventory

### Functional Requirements

FR-A1: All browser-to-device HTTP calls must be routed through server proxy endpoints
FR-A2: Shell command execution from frontend must use server proxy (`/inspector/{udid}/shell` or `/api/devices/{udid}/shell`)
FR-A3: Screenshot capture from frontend must use server proxy (`/inspector/{udid}/screenshot`)
FR-A4: Device rotation fix must use server proxy endpoint (new: `POST /inspector/{udid}/rotation`)
FR-A5: File upload to device must use server proxy (`/inspector/{udid}/upload`)
FR-A6: Screen streaming must use server-proxied WebSocket (NIO or scrcpy), not direct minicap/minitouch
FR-A7: No hardcoded IP addresses or ports in frontend code
FR-A8: `deviceUrl` computed property must be removed from all JS files, replaced with server-relative URLs

FR-B1: Product catalog CRUD — browse, create, edit device product specs (brand, model, CPU, GPU)
FR-B2: Device-product association — link a connected device to a product catalog entry
FR-B3: Provider/node management — register, monitor, and manage distributed farm nodes
FR-B4: Provider presence tracking — online/offline status with heartbeat
FR-B5: Asset/property tracking — assign inventory numbers to devices
FR-B6: Server version endpoint — `GET /api/v1/version` returning server info
FR-B7: Device reservation/locking — WebSocket-based lease preventing concurrent access
FR-B8: Video recording — capture screen stream frames and compose into video file
FR-B9: Video download/management — list, download, delete recorded videos
FR-B10: Diagnostic test page — modernized test.html using NIO/scrcpy endpoints
FR-B11: Screenshot refresh after touch — fix `window.refersh()` typo, trigger canvas update

FR-C1: API key authentication middleware for REST and WebSocket endpoints
FR-C2: Rate limiting middleware with configurable thresholds
FR-C3: Graceful shutdown — clean up background tasks (detector, discovery, scrcpy, playback) on SIGTERM
FR-C4: Config file path via CLI argument or environment variable
FR-C5: Fix UDID extraction — use proper `X-Device-UDID` header instead of `Access-Control-Allow-Origin`
FR-C6: Non-blocking image processing — `tokio::task::spawn_blocking` for `resize_jpeg`
FR-C7: Working screenshot latency metrics — call `record_screenshot_latency()` in capture paths
FR-C8: Working WebSocket connection counter — increment/decrement in all WS handlers
FR-C9: Crash protection — replace `try_into().unwrap()` with safe parsing in scrcpy_ws binary handler
FR-C10: Configurable connection pool size and timeouts via config file

FR-D1: Replace `unwrap()` with proper error handling in all request-handling paths
FR-D2: Extract shared `get_device_client` into reusable module
FR-D3: Consolidate duplicate batch operation handlers
FR-D4: Share `PhoneService` via `AppState` instead of per-request construction
FR-D5: Use `VecDeque` for MetricsTracker instead of `Vec::remove(0)`
FR-D6: Define error enum (`thiserror`) for recording/playback instead of string matching
FR-D7: Replace deprecated `document.execCommand('copy')` with Clipboard API
FR-D8: Replace deprecated `mousewheel` event with `wheel`
FR-D9: Integration tests for critical HTTP endpoints using `actix-web::test`
FR-D10: Unit tests for shell command blocklist (`is_dangerous_command`, `has_dangerous_metacharacters`)
FR-D11: Update/remove deprecated dependencies (`serde_yaml`, `urlencoding`)
FR-D12: Remove unused legacy config fields (redis, kafka, influxdb) and fix `descption` typo

### Non-Functional Requirements

NFR1: All device communication from browser must be server-proxied — no direct browser-to-device calls
NFR2: API endpoints must require authentication (API key minimum)
NFR3: Server must handle SIGTERM/SIGINT gracefully, cleaning up all active sessions
NFR4: No panics from malformed external input — all `unwrap()` on untrusted data must be eliminated
NFR5: Consistent API versioning — all functional API endpoints available under `/api/v1/` prefix
NFR6: Response time for proxied operations must not exceed 2x direct device calls
NFR7: Connection pool and cache sizes must be configurable without code changes

### Additional Requirements

- Existing frontend pages (`remote.html`, `device_synchronous.html`) must continue working during migration
- Legacy endpoints must remain functional until `/api/v1/` equivalents are proven stable
- New features (product catalog, providers) need SQLite schema migrations
- Video recording requires FFmpeg binary or equivalent Rust crate for frame compositing
- Provider system should support future multi-server deployment

### FR Coverage Map

| FR | Epic | Stories |
|----|------|---------|
| FR-A1..A8 | Epic 7: Server Proxy Migration | 7-1, 7-2, 7-3 |
| FR-B1..B2 | Epic 8: Product Catalog | 8-1, 8-2 |
| FR-B3..B4 | Epic 9: Provider Management | 9-1, 9-2 |
| FR-B5 | Epic 8: Product Catalog | 8-3 |
| FR-B6 | Epic 10: Platform Features | 10-1 |
| FR-B7 | Epic 10: Platform Features | 10-2 |
| FR-B8..B9 | Epic 11: Video Recording | 11-1, 11-2 |
| FR-B10..B11 | Epic 10: Platform Features | 10-3, 10-4 |
| FR-C1..C10 | Epic 12: Infrastructure Hardening | 12-1..12-6 |
| FR-D1..D12 | Epic 13: Code Quality & Tests | 13-1..13-5 |

## Epic List

| Epic | Title | Priority | Phase | Stories |
|------|-------|----------|-------|---------|
| 7 | Server Proxy Migration | P0 | Immediate | 3 |
| 8 | Product Catalog & Asset Tracking | P1 | Sprint 1 | 3 |
| 9 | Provider & Node Management | P1 | Sprint 1 | 2 |
| 10 | Platform Features Completion | P1 | Sprint 2 | 4 |
| 11 | Video Recording & Playback | P2 | Sprint 3 | 2 |
| 12 | Infrastructure Hardening | P1 | Sprint 1-2 | 6 |
| 13 | Code Quality & Tests | P2 | Sprint 2-3 | 5 |

---

## Epic 7: Server Proxy Migration

**Goal:** Eliminate all direct browser-to-device calls by routing everything through the CloudControl server. This is critical for the system to work when the browser is not on the same network as the devices.

### Story 7.1: Proxy Remote Page Device Calls

As a **device farm operator**,
I want **the remote control page to communicate with devices through the server**,
So that **I can control devices from any network, not just the local LAN**.

**Acceptance Criteria:**

**Given** the remote control page is open in a browser
**When** any operation (shell, screenshot, rotation fix, upload, minicap) is triggered
**Then** the request goes to the CloudControl server, not directly to the device IP
**And** the `deviceUrl` computed property is removed from `remote.js`
**And** all `this.deviceUrl + "/..."` calls are replaced with server proxy endpoints
**And** the whatsinput WebSocket (port 6677) is proxied or gracefully disabled
**And** no hardcoded IP addresses remain in `remote.js`

### Story 7.2: Proxy Batch Control Page Device Calls

As a **device farm operator**,
I want **the batch control page to use server-proxied endpoints**,
So that **multi-device operations work from any browser location**.

**Acceptance Criteria:**

**Given** the batch control page (`device_synchronous.html` / `remote_synchronous.js`) is open
**When** screen streaming, touch input, shell commands, or screenshots are triggered
**Then** all communication goes through server WebSocket (NIO) or HTTP endpoints
**And** direct `ws://device_ip/minicap` and `ws://device_ip/minitouch` connections are replaced
**And** the `deviceUrl` computed property is removed from `remote_synchronous.js` and `device_synchronous.html`
**And** the batch keyevent uses `/inspector/{udid}/keyevent` instead of `/devices/shell/...`
**And** `window.refersh()` typo is fixed (trigger screenshot refresh or remove)

### Story 7.3: Server-Side Proxy Endpoints

As a **backend developer**,
I want **all missing server-side proxy endpoints implemented**,
So that **the frontend can route all device communication through the server**.

**Acceptance Criteria:**

**Given** the server is running
**When** the frontend calls a proxy endpoint
**Then** `POST /inspector/{udid}/rotation` forwards rotation fix to device ATX agent
**And** existing endpoints (`/inspector/{udid}/shell`, `/inspector/{udid}/screenshot`, `/inspector/{udid}/upload`) work correctly for proxied calls
**And** `test.html` is updated to use NIO/scrcpy WebSocket endpoints instead of hardcoded IPs

---

## Epic 8: Product Catalog & Asset Tracking

**Goal:** Complete the product catalog system for associating devices with hardware specs, and asset tracking for inventory management.

### Story 8.1: Product Catalog CRUD

As a **device farm administrator**,
I want **to manage a catalog of device products (brand, model, CPU, GPU, specs)**,
So that **I can standardize device information across the farm**.

**Acceptance Criteria:**

**Given** the server has a `products` SQLite table
**When** I call `GET /api/v1/products?brand=X&model=Y`
**Then** matching products are returned as JSON array
**And** `POST /api/v1/products` creates a new product entry
**And** `PUT /api/v1/products/{id}` updates an existing product
**And** products have fields: id, brand, model, name, cpu, gpu, link, coverage

### Story 8.2: Device-Product Association

As a **device farm administrator**,
I want **to link a connected device to a product catalog entry via the edit page**,
So that **each device has standardized hardware specifications**.

**Acceptance Criteria:**

**Given** a device is connected and a product exists in the catalog
**When** I open `/devices/{udid}/edit` and select a product
**Then** `PUT /devices/{udid}/product` saves the association
**And** `edit.html` loads product data from the server API (not hardcoded)
**And** the device info endpoint includes the linked product data

### Story 8.3: Asset Property Tracking

As a **device farm administrator**,
I want **to assign inventory/asset numbers to devices**,
So that **I can track physical device assets**.

**Acceptance Criteria:**

**Given** a device is connected
**When** I open the property page and enter an asset number
**Then** `POST /api/v1/devices/{udid}/property` saves the property ID
**And** the device info includes a `property_id` field
**And** `property.html` form works with the server endpoint

---

## Epic 9: Provider & Node Management

**Goal:** Implement provider (server node) management for distributed device farm topology.

### Story 9.1: Provider Registry

As a **device farm administrator**,
I want **to register and manage provider nodes in the farm**,
So that **I can see which server hosts which devices**.

**Acceptance Criteria:**

**Given** the server has a `providers` SQLite table
**When** I call `GET /api/v1/providers` (or `GET /providers?json`)
**Then** all registered providers are returned with IP, notes, status, device count
**And** `PUT /api/v1/providers/{id}` updates provider notes
**And** `providers.html` displays the provider list correctly

### Story 9.2: Provider Presence & Device Association

As a **device farm administrator**,
I want **providers to show online/offline status and their connected devices**,
So that **I can monitor the health of my distributed farm**.

**Acceptance Criteria:**

**Given** a provider is registered
**When** the provider sends heartbeats
**Then** its `present` status and `presenceChangedAt` timestamp are updated
**And** the provider's device list shows all devices connected to that node
**And** providers that miss heartbeats are marked offline

---

## Epic 10: Platform Features Completion

**Goal:** Complete unfinished platform features — version endpoint, device reservation, diagnostic tools.

### Story 10.1: Server Version Endpoint

As a **developer/operator**,
I want **`GET /api/v1/version` to return server version info**,
So that **the frontend can verify server compatibility and display version**.

**Acceptance Criteria:**

**Given** the server is running
**When** `GET /api/v1/version` is called
**Then** response includes `{name, version, server: "cloudcontrol-rust"}`
**And** `remote.js:checkVersion()` uses this endpoint instead of legacy weditor check

### Story 10.2: Device Reservation System

As a **device farm operator**,
I want **to reserve a device so only one user controls it at a time**,
So that **concurrent access doesn't cause conflicts**.

**Acceptance Criteria:**

**Given** a device is available
**When** a user opens the remote page and the WebSocket to `/devices/{udid}/reserved` connects
**Then** the device is marked as reserved in AppState
**And** other users see the device as "in use"
**And** when the WebSocket disconnects, the device is released
**And** attempting to reserve an already-reserved device returns an error

### Story 10.3: Diagnostic Test Page

As a **developer**,
I want **test.html to use modern NIO/scrcpy endpoints**,
So that **I can diagnose screen streaming and touch issues without hardcoded IPs**.

**Acceptance Criteria:**

**Given** test.html is loaded in a browser
**When** a device UDID is specified
**Then** screen streaming connects via `/nio/{udid}/ws` or `/scrcpy/{udid}/ws`
**And** touch input goes through `/inspector/{udid}/touch`
**And** no hardcoded IP addresses exist in the page

### Story 10.4: Legacy Endpoint Cleanup

As a **developer**,
I want **legacy endpoints to have `/api/v1/` equivalents**,
So that **the API is consistent and versioned**.

**Acceptance Criteria:**

**Given** legacy endpoints exist (e.g., `/list`, `/inspector/...`)
**When** `/api/v1/` equivalents are needed
**Then** missing v1 endpoints are created (hierarchy, upload, rotation)
**And** legacy endpoints remain functional for backwards compatibility
**And** `LOCAL_URL`, `LOCAL_VERSION` constants are removed from `remote.js`
**And** dead methods (`fixMinicap`, `connectImage2VideoWebSocket`) are removed

---

## Epic 11: Video Recording & Playback

**Goal:** Implement screen recording to video file with compositing and download.

### Story 11.1: Screen-to-Video Recording

As a **QA tester**,
I want **to record the device screen stream into a video file**,
So that **I can review and share test sessions**.

**Acceptance Criteria:**

**Given** a device has an active screen stream (NIO or scrcpy)
**When** I start recording via the frontend
**Then** a WebSocket endpoint `ws://.../video/convert` accepts JPEG frames
**And** frames are composited into a video file (MP4) using FFmpeg or equivalent
**And** recording can be stopped, producing a downloadable video file
**And** recording metadata (device, duration, file path) is stored

### Story 11.2: Video Management

As a **QA tester**,
I want **to list, download, and delete recorded videos**,
So that **I can manage test recordings**.

**Acceptance Criteria:**

**Given** videos have been recorded
**When** I call `GET /api/v1/videos`
**Then** all recordings are listed with metadata
**And** `GET /api/v1/videos/{id}/download` serves the video file
**And** `DELETE /api/v1/videos/{id}` removes the recording

---

## Epic 12: Infrastructure Hardening

**Goal:** Add security, stability, and operational improvements to the server infrastructure.

### Story 12.1: API Authentication

As a **system administrator**,
I want **API endpoints protected by API key authentication**,
So that **unauthorized users cannot control devices**.

**Acceptance Criteria:**

**Given** an API key is configured in the server config
**When** a request is made without a valid `Authorization` header or `api_key` query param
**Then** the server returns 401 Unauthorized
**And** WebSocket upgrade requests also require authentication
**And** frontend pages served by the server are exempt (same-origin)

### Story 12.2: Rate Limiting

As a **system administrator**,
I want **rate limiting on API endpoints**,
So that **the server is protected from abuse and resource exhaustion**.

**Acceptance Criteria:**

**Given** rate limits are configured (e.g., 100 req/min per IP)
**When** a client exceeds the rate limit
**Then** the server returns 429 Too Many Requests
**And** rate limits are configurable per endpoint category

### Story 12.3: Graceful Shutdown

As a **system administrator**,
I want **the server to clean up all background tasks on shutdown**,
So that **no resources are leaked and devices are properly released**.

**Acceptance Criteria:**

**Given** the server is running with active background tasks
**When** SIGTERM or SIGINT is received
**Then** device detector, WiFi discovery, scrcpy sessions, and playback tasks are stopped
**And** active WebSocket sessions are closed gracefully
**And** the server exits cleanly within 10 seconds

### Story 12.4: Configurable Server Settings

As a **system administrator**,
I want **config file path, pool sizes, and timeouts to be configurable**,
So that **I can tune the server for different environments**.

**Acceptance Criteria:**

**Given** the server binary
**When** started with `--config /path/to/config.yaml` or `CONFIG_PATH` env var
**Then** the specified config file is loaded
**And** connection pool size, cache TTL, and timeouts are configurable in the YAML
**And** the `descption` typo is fixed to `description`
**And** unused legacy config sections (redis, kafka, influxdb) are removed

### Story 12.5: Crash Protection & Error Handling

As a **system administrator**,
I want **the server to never crash from malformed client input**,
So that **the service remains stable under all conditions**.

**Acceptance Criteria:**

**Given** the server is running
**When** a malformed binary WebSocket message is received by scrcpy_ws
**Then** the connection is closed with an error, not a panic
**And** the UDID extraction from `Access-Control-Allow-Origin` header is replaced with `X-Device-UDID`
**And** image resizing uses `tokio::task::spawn_blocking` to avoid blocking the async runtime

### Story 12.6: Working Metrics & Monitoring

As a **system administrator**,
I want **metrics endpoints to report real data**,
So that **I can monitor server health and performance**.

**Acceptance Criteria:**

**Given** the server handles screenshots and WebSocket connections
**When** `GET /api/v1/metrics` is called
**Then** screenshot latency percentiles reflect actual measurements
**And** active WebSocket connection count is accurate
**And** `MetricsTracker` uses `VecDeque` for O(1) operations

---

## Epic 13: Code Quality & Tests

**Goal:** Reduce technical debt, eliminate code duplication, and establish test coverage.

### Story 13.1: Shared Device Resolution Module

As a **developer**,
I want **`get_device_client` and batch handlers extracted into shared modules**,
So that **code duplication between `control.rs` and `api_v1.rs` is eliminated**.

**Acceptance Criteria:**

**Given** duplicate code exists in control.rs and api_v1.rs
**When** refactored
**Then** `get_device_client` and `resolve_device_connection` are in `src/services/device_resolver.rs`
**And** batch operation logic is in a shared function used by both route modules
**And** `PhoneService` is stored in `AppState` instead of constructed per-request

### Story 13.2: Error Handling Modernization

As a **developer**,
I want **proper error types instead of string matching**,
So that **error handling is type-safe and maintainable**.

**Acceptance Criteria:**

**Given** recording/playback uses string-based error discrimination
**When** refactored
**Then** a `thiserror` error enum replaces string matching
**And** critical `unwrap()` calls in request handlers are replaced with `?` or `.unwrap_or`
**And** `serde_json::to_string().unwrap()` in WS handlers uses `.unwrap_or_default()`

### Story 13.3: Frontend Modernization

As a **developer**,
I want **deprecated browser APIs replaced with modern equivalents**,
So that **the frontend works reliably in modern browsers**.

**Acceptance Criteria:**

**Given** deprecated APIs are used
**When** updated
**Then** `document.execCommand('copy')` is replaced with `navigator.clipboard.writeText()`
**And** `mousewheel` event is replaced with `wheel` event
**And** `event.wheelDeltaY` is replaced with `event.deltaY`

### Story 13.4: Integration Test Suite

As a **developer**,
I want **integration tests for critical HTTP and WebSocket endpoints**,
So that **regressions are caught automatically**.

**Acceptance Criteria:**

**Given** the test framework `actix-web::test` is available
**When** tests are run
**Then** device info, screenshot, touch, shell, and batch endpoints are tested
**And** error cases (device not found, invalid input) are covered
**And** shell command blocklist security tests verify dangerous command detection

### Story 13.5: Dependency Cleanup

As a **developer**,
I want **deprecated and unused dependencies removed or updated**,
So that **the project uses maintained libraries**.

**Acceptance Criteria:**

**Given** `serde_yaml` is deprecated and `urlencoding` is unused
**When** cleaned up
**Then** `serde_yaml` is replaced with `serde_yml` or alternative
**And** `urlencoding` is removed from `Cargo.toml`
**And** `rand` is updated to current version
**And** unused legacy config fields are removed from `AppConfig`
