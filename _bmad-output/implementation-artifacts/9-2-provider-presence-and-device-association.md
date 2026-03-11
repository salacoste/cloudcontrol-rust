# Story 9.2: Provider Presence & Device Association

Status: done

## Story

As a **device farm administrator**,
I want **providers to show online/offline status and their connected devices**,
so that **I can monitor the health of my distributed farm**.

## Acceptance Criteria

1. **Given** a provider is registered **When** I call `POST /api/v1/providers/{id}/heartbeat` **Then** the provider's `present` field becomes `true` and `presenceChangedAt` is set to the current Unix timestamp
2. **Given** a provider is sending heartbeats **When** the provider misses heartbeats for more than 60 seconds **Then** the provider's `present` field becomes `false` and `presenceChangedAt` is updated to the time it went offline
3. **Given** a provider is registered **When** I call `GET /api/v1/providers` **Then** each provider in the response includes a `devices` array listing all devices with matching `provider` field from the devices table
4. **Given** a provider is registered **When** I call `GET /api/v1/providers/{id}` **Then** the response includes the `devices` array for that provider
5. **Given** the `providers.html` page is loaded **When** providers exist with devices associated **Then** the Devices column shows the device count with a clickable link or icon
6. **Given** a provider goes offline (heartbeat timeout) **When** I view the providers list **Then** the provider's present icon disappears and the uptime column goes blank
7. **Given** a provider is online **When** I view the providers list **Then** the uptime column shows the time since the provider came online (via `presenceChangedAt | timeSince` filter)

## Tasks / Subtasks

- [x] Task 1: Add heartbeat database method (AC: #1, #2)
  - [x] 1.1 Add `update_provider_presence(id: i64, present: bool) -> Result<Option<Provider>>` to `sqlite.rs` — updates `present` and `presence_changed_at` fields, returns updated provider (None if not found). Use `rows_affected()` pattern from Story 9.1.
  - [x] 1.2 The `presence_changed_at` should be set to current Unix timestamp (seconds) whenever presence changes
- [x] Task 2: Add `list_devices_by_provider` database method (AC: #3, #4)
  - [x] 2.1 Add `list_devices_by_provider(ip: &str) -> Result<Vec<Value>>` to `sqlite.rs` — `SELECT * FROM devices WHERE provider = ?1 ORDER BY udid`, return as `Vec<Value>` using existing `device_row_to_json()` pattern
- [x] Task 3: Extend `ProviderWithDevices` model (AC: #3, #4)
  - [x] 3.1 Add `devices: Vec<serde_json::Value>` field to `ProviderWithDevices` in `provider.rs` — this is the full device list for each provider
  - [x] 3.2 Update all places that construct `ProviderWithDevices` (list_providers, get_provider, update_provider in `api_v1.rs`) to populate the `devices` field
- [x] Task 4: Add heartbeat API endpoint (AC: #1)
  - [x] 4.1 Add `provider_heartbeat` handler in `api_v1.rs` — `POST /api/v1/providers/{id}/heartbeat`, calls `update_provider_presence(id, true)`, returns provider data with success response
  - [x] 4.2 Register route in `main.rs`: `.route("/api/v1/providers/{id}/heartbeat", web::post().to(routes::api_v1::provider_heartbeat))`
- [x] Task 5: Add provider heartbeat timeout tracking (AC: #2)
  - [x] 5.1 Add `provider_heartbeats: Arc<DashMap<i64, f64>>` to `AppState` in `state.rs` — maps provider ID to expiry timestamp (same pattern as existing `heartbeat_sessions`)
  - [x] 5.2 Initialize in `AppState::new()`: `provider_heartbeats: Arc::new(DashMap::new())`
  - [x] 5.3 In `provider_heartbeat` handler: insert/update provider_heartbeats entry with `now + 60.0` timeout, then spawn a background timeout checker task (same pattern as device heartbeat in `control.rs:2157-2176`)
  - [x] 5.4 The timeout task: poll every 5 seconds, if expired → call `update_provider_presence(id, false)` and remove from DashMap
- [x] Task 6: Update list/get provider endpoints to include devices (AC: #3, #4)
  - [x] 6.1 In `list_providers` handler: after fetching providers, for each provider call `list_devices_by_provider(ip)` and build `ProviderWithDevices` with both `device_count` and `devices` fields
  - [x] 6.2 In `get_provider` handler: same enrichment with `devices` field
  - [x] 6.3 In `update_provider` handler: same enrichment with `devices` field
- [x] Task 7: Regression testing (AC: #1-#7)
  - [x] 7.1 Build succeeds — 0 new warnings
  - [x] 7.2 All existing tests pass (168/177, 9 pre-existing failures)
  - [x] 7.3 No new regressions introduced

## Dev Notes

### Heartbeat Design — Follow Existing Device Heartbeat Pattern

The project already has a heartbeat mechanism for devices in `control.rs:2119-2180`. Story 9.2 uses the EXACT same pattern for providers:

1. **Heartbeat endpoint** receives POST, updates presence to `true`
2. **DashMap** tracks provider ID → expiry timestamp
3. **Background tokio task** polls for expired heartbeats and marks providers offline

**Existing device heartbeat reference** (`control.rs:2119-2180`):
```rust
// Device heartbeat pattern - FOLLOW THIS for provider heartbeat:
pub async fn heartbeat(state, form) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
    if let Some(mut session) = sessions.get_mut(&identifier) {
        session.timer = now + 20.0;  // Reset timer
    } else {
        sessions.insert(identifier, HeartbeatSession { timer: now + 20.0, ... });
        // Spawn background timeout checker
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let expired = sess.get(&ident).map(|s| s.timer < now).unwrap_or(true);
                if expired {
                    sess.remove(&ident);
                    let _ = ps.offline_connected(&ident).await;
                    return;
                }
            }
        });
    }
}
```

**Key differences for provider heartbeat:**
- Use `provider_heartbeats: DashMap<i64, f64>` (provider ID → expiry) instead of `DashMap<String, HeartbeatSession>`
- Timeout = 60 seconds (providers are longer-lived than device heartbeats which use 20s)
- Poll interval = 5 seconds (not 1s — less aggressive for providers)
- On timeout: call `db.update_provider_presence(id, false)` instead of `ps.offline_connected()`

### Provider Presence Database Method

```rust
/// Update a provider's presence status. Sets presence_changed_at to current timestamp.
pub async fn update_provider_presence(
    &self,
    id: i64,
    present: bool,
) -> Result<Option<crate::models::provider::Provider>, sqlx::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let result = sqlx::query(
        "UPDATE providers SET present = ?1, presence_changed_at = ?2 WHERE id = ?3"
    )
    .bind(present)
    .bind(now)
    .bind(id)
    .execute(&self.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }
    self.get_provider(id).await
}
```

### Device List per Provider — Reuse device_row_to_json

The `devices` table already has a `provider` column (TEXT, nullable) that stores the provider's IP. When devices connect via WiFi discovery or heartbeat, the `provider` field is set.

To list devices for a provider, query `SELECT * FROM devices WHERE provider = ?1 ORDER BY udid` and use the existing `device_row_to_json()` method to convert rows to JSON. This is the same pattern used by `find_device_list()` and `find_all_devices()`.

```rust
pub async fn list_devices_by_provider(&self, ip: &str) -> Result<Vec<Value>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM devices WHERE provider = ?1 ORDER BY udid")
        .bind(ip)
        .fetch_all(&self.pool)
        .await?;
    Ok(rows.iter().map(Self::device_row_to_json).collect())
}
```

### ProviderWithDevices — Add devices Field

The current `ProviderWithDevices` struct has `provider` + `device_count`. Add a `devices` field:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ProviderWithDevices {
    #[serde(flatten)]
    pub provider: Provider,
    pub device_count: i64,
    pub devices: Vec<serde_json::Value>,
}
```

**IMPORTANT**: Update ALL 3 places that construct `ProviderWithDevices`:
- `list_providers` handler (`api_v1.rs:935-947`)
- `get_provider` handler (`api_v1.rs:1006-1018`)
- `update_provider` handler (`api_v1.rs:1062-1073`)

Each must now also call `list_devices_by_provider(ip)` and pass the result.

### AppState Extension

Add to `state.rs`:
```rust
pub struct AppState {
    // ... existing fields ...
    /// Provider heartbeat tracking: provider_id → expiry timestamp
    pub provider_heartbeats: Arc<DashMap<i64, f64>>,
}
```

Initialize in `AppState::new()`:
```rust
provider_heartbeats: Arc::new(DashMap::new()),
```

### Heartbeat Endpoint — Provider-specific

```rust
/// POST /api/v1/providers/{id}/heartbeat — provider heartbeat keep-alive
pub async fn provider_heartbeat(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let id = path.into_inner();
    // ... update presence, manage DashMap timeout, return provider data ...
}
```

### What NOT to Implement

- Do NOT modify the device model or its existing `provider` field — it already exists
- Do NOT add automatic provider creation from device discovery — providers are manually registered (Story 9.1)
- Do NOT add provider deletion — not in ACs
- Do NOT add WebSocket-based heartbeat — use simple HTTP POST (Story 9.2 is about simple presence)
- Do NOT modify `providers.html` beyond what's needed — the template already shows `p.present`, `p.presenceChangedAt | timeSince`, and `p.device_count` correctly (fixed in Story 9.1 code review H1)

### Template Notes — No Changes Needed

The `providers.html` template already handles everything needed for Story 9.2:
- `v-if="p.present"` shows/hides the green smile icon ✅
- `{{p.presenceChangedAt | timeSince}}` shows uptime (camelCase fixed in Story 9.1 code review H1) ✅
- `v-text="p.device_count"` shows device count ✅
- `v-show="p.present"` hides uptime when offline ✅

The only consideration is that the template doesn't currently show the list of individual devices, just the count. This is acceptable per the AC — "provider's device list shows all devices connected to that node" is satisfied by the API response including the `devices` array. The template shows the count, which is the user-facing equivalent.

### Error Handling Patterns (from Story 9.1 code review)

- Use `tracing::warn!` on all database errors
- Return 404 with `ERR_PROVIDER_NOT_FOUND` for non-existent provider IDs
- Return consistent `{"status": "success"/"error", "data": ...}` response format
- Trim inputs where applicable
- Use `rows_affected()` for non-existent entity detection

### Project Structure Notes

- Modified: `src/models/provider.rs` — add `devices` field to `ProviderWithDevices`
- Modified: `src/db/sqlite.rs` — add `update_provider_presence()` + `list_devices_by_provider()` methods
- Modified: `src/routes/api_v1.rs` — add heartbeat handler + update list/get/update to include devices
- Modified: `src/state.rs` — add `provider_heartbeats` to AppState
- Modified: `src/main.rs` — register heartbeat route
- NO template changes needed (providers.html already handles all display)

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Epic 9, Story 9.2]
- [Source: docs/project-context.md#Tech Stack — sqlx 0.8, actix-web 4, dashmap 6, tokio 1.x]
- [Source: src/routes/control.rs:2119-2180 — existing device heartbeat pattern]
- [Source: src/state.rs:15-23 — HeartbeatSession struct, DashMap pattern]
- [Source: src/state.rs:90-135 — AppState struct and constructor]
- [Source: src/db/sqlite.rs:385-696 — device_row_to_json(), find_device_list(), device operations]
- [Source: src/db/sqlite.rs:1687-1771 — provider CRUD methods from Story 9.1]
- [Source: src/models/provider.rs — Provider, ProviderWithDevices, CreateProviderRequest]
- [Source: src/models/device.rs:48 — Device.provider field (Option<String>)]
- [Source: src/routes/api_v1.rs:928-1081 — provider API endpoints from Story 9.1]
- [Source: resources/templates/providers.html — template already handles presence display]
- [Source: _bmad-output/implementation-artifacts/9-1-provider-registry.md — Story 9.1 code review fixes, patterns established]

### Git Context

Recent commits establish these patterns:
- Story 9.1 established provider CRUD, `ProviderWithDevices` with `#[serde(flatten)]`, `#[serde(rename = "presenceChangedAt")]`, 409 Conflict for duplicate IPs
- Story 9.1 code review: consistent API response format, `tracing::warn!` on errors, input trimming
- Device heartbeat pattern established early in project — `DashMap` + `tokio::spawn` timeout checker

### Previous Story Intelligence (Story 9.1)

Critical lessons to apply:
- **H1 fix applied**: `presenceChangedAt` serde rename already on Provider struct — do NOT change this
- **M2 fix applied**: All provider endpoints return `ProviderWithDevices` — maintain this consistency when adding `devices` field
- **M3 fix applied**: Duplicate IP returns 409 — no change needed
- **jQuery `body: String` pattern**: Any new endpoint called from templates must use `body: String` not `web::Json` — but heartbeat endpoint will be called programmatically (by provider nodes), so `web::Json` or no-body POST is fine
- **N+1 query**: Adding `list_devices_by_provider()` per provider in list endpoint adds another N+1. Acceptable for small farm sizes, but be aware.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Build: 0 errors, 0 new warnings (5 pre-existing)
- Tests: 168/177 passed (9 pre-existing failures, 0 new regressions)

### Completion Notes List

- All 7 tasks completed with zero compilation errors on first attempt
- Heartbeat mechanism follows existing device heartbeat pattern (DashMap + tokio::spawn timeout checker)
- Provider timeout: 60s (vs 20s for devices), poll interval: 5s (vs 1s)
- `list_devices_by_provider()` reuses `device_row_to_json()` for consistent device JSON format
- All 3 `ProviderWithDevices` construction sites updated with `devices` field
- No template changes needed — providers.html already handles presence display correctly

### Code Review Fixes (2026-03-10)

- **H1 FIXED**: Restructured heartbeat handler — only calls `update_provider_presence(id, true)` for NEW sessions (not in DashMap). Existing sessions just reset timer and fetch data via `get_provider()`, preserving `presenceChangedAt` for correct uptime display
- **M1 FIXED**: Eliminated redundant `count_devices_by_provider()` calls in all 4 provider endpoints — now uses `devices.len() as i64` (saves 1 query per provider)
- **M2 FIXED**: Changed `.unwrap()` to `.unwrap_or_default()` on `SystemTime::now()` in heartbeat handler for safety consistency
- **L1 TRACKED**: Unnecessary DB writes on every heartbeat — resolved by H1 fix
- **L2 TRACKED**: No `AND present != ?1` guard in SQL — acceptable since H1 fix prevents redundant calls

### File List

- src/models/provider.rs (added `devices: Vec<serde_json::Value>` to ProviderWithDevices)
- src/db/sqlite.rs (added `update_provider_presence()` + `list_devices_by_provider()`)
- src/routes/api_v1.rs (added `provider_heartbeat` handler + updated list/get/update to include devices)
- src/state.rs (added `provider_heartbeats: Arc<DashMap<i64, f64>>` to AppState)
- src/main.rs (registered heartbeat route: POST /api/v1/providers/{id}/heartbeat)
