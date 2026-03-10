# Epic 1A Retrospective: Device Connection & Discovery

**Date:** 2026-03-10
**Epic:** 1A -- Device Connection & Discovery
**Status:** Complete (6/6 stories done)
**FRs Covered:** FR1, FR2, FR3, FR4, FR5, FR6

## Delivery Metrics

| Metric | Value |
|--------|-------|
| Stories Completed | 6/6 (100%) |
| Key Source Files | 7 (wifi_discovery.rs, adb.rs, atx_client.rs, atx_init.rs, device_detector.rs, phone_service.rs, host_ip.rs) |
| Inline Tests | 17 (wifi_discovery: 7, adb: 4, connection_pool: 4, screenshot_cache: 2) |
| New Dependencies | reqwest, tokio, tokio-util (CancellationToken), futures |
| Production Incidents | 0 |

## Architecture Evolution

Each story added a distinct connectivity layer:

```
1a-1: WiFi Discovery    ->  WifiDiscovery subnet scanner + concurrent batched probing
1a-2: USB/ADB           ->  Adb wrapper (tokio::process) + forward/push/screencap
1a-3: ATX Agent          ->  AtxClient JSON-RPC + AtxInit u2.jar lifecycle
1a-4: Manual Addition    ->  PhoneService.on_connected() with IP-based registration
1a-5: Health Monitoring  ->  HeartbeatSession DashMap + missed_count offline detection
1a-6: Auto-Reconnect     ->  DeviceDetector 1s polling + WifiDiscovery periodic scan
```

## Successes

1. **Dual protocol support** -- `AtxClient` seamlessly supports both old atx-agent (port 7912, GET /info) and new u2.jar (port 9008, JSON-RPC deviceInfo) without caller awareness. The `probe_device` method tries both automatically.
2. **Batched concurrent scanning** -- `WifiDiscovery` scans 253 IPs using configurable `max_concurrent_probes` (default 50) with `futures::join_all`, keeping scan times under 5 seconds.
3. **Graceful shutdown** -- `CancellationToken` in WifiDiscovery enables clean shutdown via `tokio::select!`, avoiding orphaned background tasks on server restart.
4. **Configurable everything** -- `WifiDiscoveryConfig` with serde defaults means production can tune ports, probe timeout, scan interval, and offline retry count without code changes.
5. **UDID generation consistency** -- Both WiFi and USB paths use the same `{serial}-{model}` pattern with space-to-underscore normalization, preventing duplicate device entries.

## Challenges

1. **Subnet auto-detection** -- Early versions hardcoded subnet from host IP. Had to add `get_local_subnets()` utility to discover all active network interfaces, supporting multi-NIC servers.
2. **USB device IP resolution** -- USB devices have no direct IP. Solved by running `ip route | grep wlan0` via ADB shell to get the device's WiFi IP, falling back to ADB port forwarding when WiFi is unavailable.
3. **ADB forward port discovery** -- `adb forward tcp:0` returns the assigned port inconsistently across ADB versions (stdout vs stderr vs nothing). Added a three-tier fallback: parse stdout, parse stderr, then `adb forward --list` lookup.
4. **Offline detection timing** -- Single missed scan should not trigger offline status. Implemented `missed_count` with configurable `offline_retry_count` (default 3) to prevent flapping.

## Patterns Discovered

| Pattern | Story | Reused In |
|---------|-------|-----------|
| Dual-protocol probe (GET + JSON-RPC) | 1a-1 | 1a-3 (AtxClient) |
| Batched concurrent futures with join_all | 1a-1 | Epic 4 (batch operations) |
| CancellationToken for graceful shutdown | 1a-1 | Epic 6 (scrcpy) |
| Shared Arc<Client> for HTTP connection reuse | 1a-1 | All HTTP paths |
| UDID = serial-model normalization | 1a-2 | All epics |
| Background polling with tokio::spawn | 1a-6 | Epic 2 (streaming), Epic 6 |

## Technical Debt

| Item | Severity | Description |
|------|----------|-------------|
| Scan duplication | Medium | `scan_subnet()` and the background polling loop duplicate the scan logic. Should extract into a shared scanner struct. |
| Single subnet limitation | Low | `scan_subnet` only scans the first detected subnet. Multi-subnet environments require explicit configuration. |
| No ADB authentication | Low | ADB connection assumes authorized host. No handling for ADB authorization prompts. |
| Blocking u2 init | Low | `AtxInit::init_device` blocks up to 15s polling for server readiness. Non-blocking but occupies a tokio task. |

## Action Items

1. Extract scan logic into a reusable `SubnetScanner` struct to eliminate duplication between initial and periodic scans
2. Add multi-subnet scanning support for enterprise deployments
3. Consider implementing ADB connection authentication detection and user guidance
