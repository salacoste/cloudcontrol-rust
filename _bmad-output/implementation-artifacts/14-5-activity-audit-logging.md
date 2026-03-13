# Story 14.5: Activity Audit Logging

Status: done

## Story

As a **System Administrator**,
I want to view audit logs of user activity,
so that I can track security and compliance.

## Acceptance Criteria

```gherkin
Scenario: Admin retrieves audit log with pagination
  Given an admin user is authenticated
  And the system has audit log entries
  When a GET request is sent to /api/v1/admin/audit-log with:
    | Authorization: Bearer <admin_access_token>
    | page: 1 (default)
    | per_page: 20 (default, max 100)
  Then the response status is 200 OK
  And the response contains a paginated list of audit entries with:
    | id: entry identifier
    | action: action type (e.g., "user.login", "team.member_added")
    | actor_id: user who performed the action
    | actor_email: email of the actor (if available)
    | target_type: type of target (user, team, device, profile, session)
    | target_id: identifier of the target
    | team_id: associated team (if applicable)
    | details: JSON object with additional context
    | created_at: timestamp of the event
  And the response includes pagination metadata (total, page, per_page, total_pages)

Scenario: Admin filters audit log by user
  Given an admin user is authenticated
  And the system has audit log entries for multiple users
  When a GET request is sent to /api/v1/admin/audit-log?user_id=user_abc123 with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And only entries where actor_id matches user_abc123 are returned

Scenario: Admin filters audit log by action type
  Given an admin user is authenticated
  And the system has various audit log entries
  When a GET request is sent to /api/v1/admin/audit-log?action=user.login with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And only entries with action "user.login" are returned

Scenario: Admin filters audit log by date range
  Given an admin user is authenticated
  And the system has audit log entries spanning multiple days
  When a GET request is sent to /api/v1/admin/audit-log?start_date=2026-03-01&end_date=2026-03-10 with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And only entries within the date range are returned

Scenario: Admin filters audit log by target type
  Given an admin user is authenticated
  And the system has audit log entries for various target types
  When a GET request is sent to /api/v1/admin/audit-log?target_type=device with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And only entries with target_type "device" are returned

Scenario: Admin combines multiple filters
  Given an admin user is authenticated
  When a GET request is sent to /api/v1/admin/audit-log?user_id=user_abc&action=team.member_added&start_date=2026-03-01 with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And entries are filtered by all provided criteria (AND logic)

Scenario: Non-admin cannot access audit log
  Given a non-admin user (role "agent") is authenticated
  When a GET request is sent to /api/v1/admin/audit-log with:
    | Authorization: Bearer <agent_access_token>
  Then the response status is 403 Forbidden
  And the error code is CC-AUTH-104

Scenario: Login event is logged automatically
  Given a user with email "maria@example.com" exists
  When the user successfully logs in via POST /api/v1/auth/login
  Then an audit log entry is created with:
    | action: "user.login"
    | actor_id: the user's id
    | actor_email: "maria@example.com"
    | target_type: "session"
    | target_id: the session id (refresh token id)
    | details: JSON with ip_address and user_agent
  And the entry is queryable via the audit log API

Scenario: Failed login attempt is logged
  Given a user with email "maria@example.com" exists
  When a login attempt fails with invalid password
  Then an audit log entry is created with:
    | action: "user.login_failed"
    | actor_id: "anonymous"
    | actor_email: "maria@example.com"
    | target_type: "session"
    | target_id: "n/a"
    | details: JSON with reason="invalid_credentials" and ip_address

Scenario: Logout event is logged
  Given an authenticated user
  When the user logs out via POST /api/v1/auth/logout
  Then an audit log entry is created with:
    | action: "user.logout"
    | actor_id: the user's id
    | target_type: "session"
    | target_id: the revoked session id

Scenario: Role change is logged
  Given an admin user is authenticated
  And a user exists with id "user_xyz"
  When the admin changes the user's role via PUT /api/v1/admin/users/user_xyz/role
  Then an audit log entry is created with:
    | action: "user.role_changed"
    | actor_id: the admin's id
    | target_type: "user"
    | target_id: "user_xyz"
    | details: JSON with old_role and new_role

Scenario: Session revocation is logged
  Given an authenticated user
  And the user has an active session with id "rt_abc123"
  When the user revokes the session via DELETE /api/v1/auth/sessions/rt_abc123
  Then an audit log entry is created with:
    | action: "session.revoked"
    | actor_id: the user's id
    | target_type: "session"
    | target_id: "rt_abc123"

Scenario: Empty audit log returns empty list
  Given an admin user is authenticated
  And the system has no audit log entries
  When a GET request is sent to /api/v1/admin/audit-log with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And the response contains an empty entries array
  And total is 0

Scenario: Pagination limits are enforced
  Given an admin user is authenticated
  When a GET request is sent to /api/v1/admin/audit-log?per_page=500 with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And per_page is capped at 100
```

## Tasks / Subtasks

- [ ] Task 1: Audit log model and DTOs (AC: #1, #2)
  - [ ] Create AuditEntry struct in `src/models/audit.rs`
  - [ ] AuditEntry: id, action, actor_id, actor_email, target_type, target_id, team_id, details (JSON), created_at
  - [ ] AuditListResponse with entries array and pagination (total, page, per_page, total_pages)
  - [ ] AuditQueryParams for filtering (user_id, action, target_type, start_date, end_date, page, per_page)
  - [ ] Define AuditAction enum with all action types (user.login, user.logout, user.login_failed, user.role_changed, session.revoked, team.*, etc.)

- [ ] Task 2: Audit service methods (AC: #1-#8, #11)
  - [ ] Create `src/services/audit_service.rs`
  - [ ] Implement list_entries() with pagination support
  - [ ] Implement filtering: by actor_id, action, target_type, date range
  - [ ] Implement log_event() helper for consistent event logging
  - [ ] Add compile-time verified queries with sqlx

- [ ] Task 3: Audit API routes (AC: #1-#7)
  - [ ] Create `src/routes/audit.rs` or add to `src/routes/admin.rs`
  - [ ] Add GET /api/v1/admin/audit-log route
  - [ ] Parse query parameters for filtering
  - [ ] Require AdminAudit permission (or AdminUsers as fallback)
  - [ ] Return 403 for non-admin with CC-AUTH-104

- [ ] Task 4: Integrate audit logging into auth events (AC: #8, #9, #10)
  - [ ] Modify `src/services/auth_service.rs` login() to log user.login on success
  - [ ] Modify login() to log user.login_failed on failure
  - [ ] Modify logout() to log user.logout
  - [ ] Log should include ip_address and user_agent from request context

- [ ] Task 5: Integrate audit logging into session management (AC: #12)
  - [ ] Modify `src/services/auth_service.rs` revoke_session() to log session.revoked
  - [ ] Modify revoke_all_other_sessions() to log session.revoked with count

- [ ] Task 6: Integrate audit logging into role management (AC: #11)
  - [ ] Modify `src/routes/admin.rs` update_user_role to log user.role_changed
  - [ ] Include old_role and new_role in details JSON

- [ ] Task 7: Route wiring (AC: all)
  - [ ] Add audit routes to main.rs under /api/v1/admin scope
  - [ ] Ensure RequireAdmin middleware is applied

- [ ] Task 8: Unit tests (AC: all)
  - [ ] Test AuditEntry model serialization
  - [ ] Test AuditQueryParams validation
  - [ ] Test AuditAction enum formatting
  - [ ] Test list_entries filtering logic

- [ ] Task 9: Integration tests (AC: all)
  - [ ] Create tests in `tests/test_audit.rs`
  - [ ] Test audit log retrieval with pagination
  - [ ] Test filtering by user, action, date range
  - [ ] Test non-admin access denied
  - [ ] Test login creates audit entry
  - [ ] Test failed login creates audit entry
  - [ ] Test role change creates audit entry

## Dev Notes

### Architecture Patterns (MUST FOLLOW)

**From `architecture.md` - ADR-017 Error Code System:**
- Format: CC-{CATEGORY}-{NUMBER}
- Categories: AUTH (1xx), PROF (2xx), DEV (3xx), AGENT (4xx), NOTIF (5xx), SYS (9xx)
- CC-AUTH-104: Insufficient permissions (403)

**Existing Audit Log Table (from Story 14-3):**
```sql
-- Already exists in src/db/sqlite.rs
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    action TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    actor_email TEXT,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    team_id TEXT,
    details TEXT,  -- JSON
    created_at TEXT NOT NULL
);

-- Indexes already exist:
-- idx_audit_log_actor ON audit_log(actor_id)
-- idx_audit_log_target ON audit_log(target_type, target_id)
-- idx_audit_log_team ON audit_log(team_id)
```

**New Index Needed for Date Filtering:**
```sql
CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log(created_at);
```

### Audit Action Types

**Standardized Action Naming Convention:**
```
{entity}.{action}

Examples:
- user.login
- user.logout
- user.login_failed
- user.role_changed
- session.revoked
- session.refresh
- team.created (already implemented in 14-3)
- team.member_added (already implemented in 14-3)
- device.assigned (already implemented in 14-3)
- profile.checkout (future - Story 15.6a)
- profile.checkin (future - Story 15.6b)
```

### Existing Code to Leverage

**From `src/services/team_service.rs` (Story 14-3):**
- `audit_log()` helper method exists - can be extracted into shared service
- Pattern: async fn audit_log(action, actor_id, actor_email, target_type, target_id, team_id, details)
- AuditTargetType enum exists: User, Team, Device, Profile, Session

**From `src/routes/admin.rs` (Story 14-2/14-3):**
- RequireAdmin extractor pattern
- Permission checking via Permission::AdminUsers or new Permission::AdminAudit

**From `src/services/auth_service.rs` (Story 14-1/14-4):**
- login(), logout(), refresh() methods
- Session management methods
- IP address and user agent captured on login (Story 14-4)

### API Response Format

**GET /api/v1/admin/audit-log**
```json
{
  "status": "success",
  "data": {
    "entries": [
      {
        "id": 123,
        "action": "user.login",
        "actor_id": "user_abc123",
        "actor_email": "maria@example.com",
        "target_type": "session",
        "target_id": "rt_xyz789",
        "team_id": null,
        "details": {
          "ip_address": "192.168.1.100",
          "user_agent": "Mozilla/5.0..."
        },
        "created_at": "2026-03-13T10:30:00Z"
      }
    ],
    "pagination": {
      "total": 150,
      "page": 1,
      "per_page": 20,
      "total_pages": 8
    }
  }
}
```

**Query Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| user_id | string | Filter by actor_id |
| action | string | Filter by action type |
| target_type | string | Filter by target type (user, team, device, session, profile) |
| start_date | string | ISO date string (inclusive) |
| end_date | string | ISO date string (inclusive) |
| page | integer | Page number (default: 1) |
| per_page | integer | Items per page (default: 20, max: 100) |

### Project Structure Notes

**Files to Create:**
```
src/
├── models/
│   └── audit.rs           # NEW: AuditEntry, AuditListResponse, AuditQueryParams
├── services/
│   └── audit_service.rs   # NEW: Audit service with list_entries, log_event
```

**Files to Modify:**
- `src/models/mod.rs` — Export audit module
- `src/services/mod.rs` — Export audit_service module
- `src/services/auth_service.rs` — Add audit logging to login/logout/refresh
- `src/services/team_service.rs` — Consider using shared audit_service (optional refactor)
- `src/routes/admin.rs` — Add audit log endpoint
- `src/main.rs` — Wire audit routes
- `src/db/sqlite.rs` — Add created_at index
- `src/middleware.rs` — Verify RequireAdmin works for audit endpoint

### Testing Standards

**Unit Tests (in audit.rs and audit_service.rs):**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_audit_action_formatting() { ... }
    #[test]
    fn test_audit_query_params_defaults() { ... }
    #[test]
    fn test_audit_entry_serialization() { ... }
}
```

**Integration Tests (tests/test_audit.rs):**
- Test audit log retrieval
- Test filtering combinations
- Test pagination
- Test access control (non-admin denied)
- Test login creates audit entry
- Test role change creates audit entry

### Dependencies

**Stories That Must Be Complete:**
- Story 14.1: User Registration and Login ✅ (users table, auth_service)
- Story 14.2: Role-Based Access Control ✅ (RequireAdmin middleware)
- Story 14.3: Team/Organization Scoping ✅ (audit_log table exists)
- Story 14.4: Session Management ✅ (session methods to audit)

**No New External Dependencies Required** — Uses existing sqlx, actix-web, serde, chrono

### Security Considerations

1. **Access Control:** Only admins can view audit logs
2. **Sensitive Data:** Details field may contain IP addresses - consider retention policy
3. **Audit Integrity:** Audit entries should be append-only (no updates/deletes via API)
4. **Performance:** Date range queries need index on created_at
5. **Pagination Required:** Prevent memory exhaustion with large result sets

### References

- [Source: `_bmad-output/planning-artifacts/architecture.md#ADR-017`] - Error code system
- [Source: `_bmad-output/planning-artifacts/epics-phase3.md#Story-14.5`] - Story definition
- [Source: `src/services/team_service.rs`] - Existing audit_log method pattern
- [Source: `src/db/sqlite.rs`] - Existing audit_log table schema
- [Source: `src/routes/admin.rs`] - Admin routes and RequireAdmin pattern
- [Source: Story 14-3 implementation] - audit_log table creation and team audit logging
- [Source: Story 14-4 implementation] - Session management to audit

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List
