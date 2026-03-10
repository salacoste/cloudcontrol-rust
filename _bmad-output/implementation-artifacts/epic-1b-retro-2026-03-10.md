# Epic 1B Retrospective: Device Dashboard & Management

**Date:** 2026-03-10
**Epic:** 1B -- Device Dashboard & Management
**Status:** Complete (6/6 stories done)
**FRs Covered:** FR7, FR8, FR9, FR10, FR11, FR12

## Delivery Metrics

| Metric | Value |
|--------|-------|
| Stories Completed | 6/6 (100%) |
| Key Source Files | 6 (sqlite.rs, phone_service.rs, state.rs, config.rs, main.rs routes, 11 HTML templates) |
| Database Tables | 2 (devices, connection_events) |
| HTML Templates | 11 (index, remote, property, providers, edit, file, async, device_synchronous, test, 404, 500) |
| New Dependencies | sqlx (SQLite), tera (templates), moka (cache), dashmap |
| Production Incidents | 0 |

## Architecture Evolution

Each story added a management capability on top of the 1A connectivity layer:

```
1b-1: Device Grid       ->  Tera templates + /api/list JSON endpoint + index.html grid
1b-2: Metadata Panel     ->  property.html detail view + PhoneService.query_info_by_udid()
1b-3: State Persistence  ->  SQLite via sqlx + restore_devices() on startup
1b-4: Tagging System     ->  tags column (JSON array in SQLite TEXT) + add_tags/remove_tag
1b-5: Connection History ->  connection_events table + get_connection_history_with_durations()
1b-6: Manual Disconnect  ->  PhoneService.offline_connected() + ConnectionPool.remove()
```

## Successes

1. **SQLite field mapping layer** -- `FIELD_MAPPING` constant maps between JSON camelCase keys (e.g., `agentVersion`) and SQLite snake_case columns (e.g., `agent_version`). The `column_to_json_key()` reverse mapping ensures consistent API output without leaking storage details.
2. **JSON fields in SQLite** -- Complex nested objects (display, battery, memory, cpu, tags) are stored as serialized JSON TEXT columns. The `JSON_FIELDS` constant drives automatic serialization/deserialization in `upsert()` and row-to-JSON conversion.
3. **Device state persistence across restarts** -- `restore_devices()` loads all persisted devices and marks them offline. Discovery services then re-detect online devices naturally, preserving tags, metadata, and history while ensuring accurate online status.
4. **Moka-based caching** -- `device_info_cache` (5-min TTL, 500 capacity) and `ConnectionPool` (moka LRU, 10-min idle timeout, 1200 capacity) prevent redundant DB queries and HTTP client creation during high-frequency screenshot streaming.
5. **Connection history with computed durations** -- `get_connection_history_with_durations()` pairs connect/disconnect events and calculates session durations server-side, avoiding client-side computation.

## Challenges

1. **Boolean field handling in SQLite** -- SQLite has no native boolean type. Required explicit `BOOL_FIELDS` constant and conversion logic: JSON `true/false` stored as INTEGER `1/0`, reversed on read. Easy to miss on new boolean fields.
2. **Tag storage as JSON array** -- Tags stored as serialized JSON array in a TEXT column means no indexed tag queries. `find_devices_by_tag()` uses SQLite `json_each()` function which performs a full table scan. Acceptable at current scale but would need a junction table for 1000+ devices.
3. **Tera template integration** -- Tera templates require explicit variable passing for each route. No automatic global context injection, so shared data (host IP, version) must be manually added to each template render call.
4. **DashMap vs Mutex for heartbeats** -- Chose DashMap for `heartbeat_sessions` to avoid mutex contention during concurrent heartbeat updates. Trade-off: slightly higher memory overhead per entry vs lock-free concurrent access.

## Patterns Discovered

| Pattern | Story | Reused In |
|---------|-------|-----------|
| JSON-in-SQLite for nested data | 1b-3 | Epic 4 (recordings, batch_reports) |
| Field mapping constant for DB abstraction | 1b-3 | All DB operations |
| Moka cache with TTL for hot data | 1b-1 | Epic 2 (screenshot_cache), Epic 3, Epic 5 |
| restore_devices() startup pattern | 1b-3 | Server lifecycle |
| DashMap for concurrent state | 1b-5 | Epic 6 (ScrcpyManager sessions) |

## Technical Debt

| Item | Severity | Description |
|------|----------|-------------|
| Tag indexing | Medium | JSON array tags in TEXT column cannot be indexed. Fine for <100 devices, needs junction table for scale. |
| No migration framework | Medium | Schema changes require manual ALTER TABLE. sqlx migrations are available but not configured. |
| Template context duplication | Low | Each route handler manually constructs Tera context with the same base variables. Should use a middleware or shared context builder. |
| Connection stats computation | Low | `get_connection_stats()` computes uptime percentage from raw events on every call. Should cache or precompute for dashboards. |

## Action Items

1. Evaluate sqlx migrations for schema versioning as the project adds more tables
2. Consider a tags junction table if device count exceeds 100
3. Create a Tera context middleware to inject common variables (host_ip, version) automatically
