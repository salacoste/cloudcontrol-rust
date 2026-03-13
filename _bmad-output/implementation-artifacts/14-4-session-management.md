# Story 14.4: Session Management

Status: done

## Story

As a **User**,
I want to view and revoke my active sessions,
so that I can monitor and control my account security.

## Acceptance Criteria

```gherkin
Scenario: User lists their active sessions
  Given a user is authenticated
  And the user has 3 active refresh tokens (sessions)
  When a GET request is sent to /api/v1/auth/sessions with:
    | Authorization: Bearer <access_token>
  Then the response status is 200 OK
  And the response contains a list of sessions with:
    | id: session identifier
    | created_at: when session was created
    | expires_at: when session expires
    | last_used_at: when token was last refreshed (optional)
    | user_agent: client identifier (optional)
    | ip_address: originating IP (optional)
  And revoked sessions are NOT included in the list

Scenario: User revokes a specific session
  Given a user is authenticated
  And the user has an active session with id "sess_abc123"
  When a DELETE request is sent to /api/v1/auth/sessions/sess_abc123 with:
    | Authorization: Bearer <access_token>
  Then the response status is 200 OK
  And the session is marked as revoked
  And the user can no longer use that refresh token

Scenario: User cannot revoke another user's session
  Given user "alice" is authenticated
  And user "bob" has an active session with id "sess_xyz789"
  When a DELETE request is sent to /api/v1/auth/sessions/sess_xyz789 with:
    | Authorization: Bearer <alice_access_token>
  Then the response status is 404 Not Found
  And the error code is CC-AUTH-107
  And the session is NOT revoked

Scenario: User revokes all other sessions
  Given a user is authenticated
  And the user has 5 active sessions
  When a DELETE request is sent to /api/v1/auth/sessions with:
    | Authorization: Bearer <access_token>
    | X-Current-Session: keep (optional header)
  Then the response status is 200 OK
  And 4 sessions are revoked (current session preserved)
  And the response indicates how many sessions were revoked

Scenario: Session list is empty when no active sessions
  Given a user is authenticated
  And the user has no active refresh tokens
  When a GET request is sent to /api/v1/auth/sessions with:
    | Authorization: Bearer <access_token>
  Then the response status is 200 OK
  And the response contains an empty sessions array

Scenario: Unauthenticated request is rejected
  Given no authentication token is provided
  When a GET request is sent to /api/v1/auth/sessions
  Then the response status is 401 Unauthorized
  And the error code is CC-AUTH-103
```

## Tasks / Subtasks

- [x] Task 1: Session model and DTOs (AC: #1, #2)
  - [x] Create Session struct in `src/models/auth.rs` or new `src/models/session.rs`
  - [x] Session: id, user_id, created_at, expires_at, last_used_at, user_agent, ip_address
  - [x] SessionInfo response DTO (excludes sensitive data like token_hash)
  - [x] SessionListResponse with sessions array and total count

- [x] Task 2: Extend refresh_tokens schema (AC: #1)
  - [x] Add optional columns to refresh_tokens table in `src/db/sqlite.rs`:
    - last_used_at TEXT (updated on token refresh)
    - user_agent TEXT (from login request)
    - ip_address TEXT (from login request)
  - [x] Use ALTER TABLE migration pattern (silently fail if exists)
  - [x] Update login() to capture user_agent and ip_address

- [x] Task 3: Session service methods (AC: #1, #2, #3, #4)
  - [x] Add list_sessions(user_id) to AuthService
  - [x] Add revoke_session(user_id, session_id) to AuthService
  - [x] Add revoke_all_other_sessions(user_id, current_session_id) to AuthService
  - [x] Ensure user can only access their own sessions

- [x] Task 4: Session API routes (AC: all)
  - [x] Add GET /api/v1/auth/sessions route in `src/routes/auth.rs`
  - [x] Add DELETE /api/v1/auth/sessions/{id} route
  - [x] Add DELETE /api/v1/auth/sessions (revoke all others) route
  - [x] All routes require authentication via JwtAuth middleware

- [x] Task 5: Route wiring (AC: all)
  - [x] Add session routes to main.rs under /api/v1/auth scope
  - [x] Wrap with JwtAuth middleware

- [x] Task 6: Unit tests (AC: all)
  - [x] Test session listing filters revoked tokens
  - [x] Test session revocation by owner
  - [x] Test session revocation prevents cross-user access

- [x] Task 7: Integration tests (AC: all)
  - [x] Create tests in `tests/test_auth.rs` or new `tests/test_sessions.rs`
  - [x] Test full session lifecycle: login → list → revoke → verify revoked
  - [x] Test cross-user session access prevention
  - [x] Test revoke all other sessions

## Dev Notes

### Architecture Patterns (MUST FOLLOW)

**From `architecture.md` - ADR-008: JWT + Refresh Token Pattern:**

The refresh_tokens table already exists with:
```sql
CREATE TABLE refresh_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked BOOLEAN DEFAULT FALSE,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
```

**Extension for Session Management:**
```sql
-- Add optional metadata columns (use ALTER TABLE pattern)
ALTER TABLE refresh_tokens ADD COLUMN last_used_at TEXT;
ALTER TABLE refresh_tokens ADD COLUMN user_agent TEXT;
ALTER TABLE refresh_tokens ADD COLUMN ip_address TEXT;
```

**Session ID Format:** Sessions use the refresh token's `id` field (format: `rt_{uuid}` - generated by `generate_refresh_token_id()`)

### Existing Code to Leverage

**From `src/services/auth_service.rs`:**
- `logout(refresh_token)` - revokes single token by hash
- `logout_all(user_id)` - revokes all tokens for user
- Both methods already exist and work correctly

**New Methods Needed:**
```rust
impl AuthService {
    /// List active sessions for a user
    pub async fn list_sessions(&self, user_id: &str) -> Result<Vec<SessionInfo>, AuthError> {
        // Query refresh_tokens where user_id = ? AND revoked = 0 AND expires_at > now
    }

    /// Revoke a specific session (must belong to user)
    pub async fn revoke_session(&self, user_id: &str, session_id: &str) -> Result<bool, AuthError> {
        // UPDATE refresh_tokens SET revoked = 1
        // WHERE id = ? AND user_id = ? AND revoked = 0
        // Return true if row was affected
    }
}
```

### API Response Format

**GET /api/v1/auth/sessions**
```json
{
  "status": "success",
  "data": {
    "sessions": [
      {
        "id": "sess_abc123def456",
        "created_at": "2026-03-13T10:30:00Z",
        "expires_at": "2026-03-20T10:30:00Z",
        "last_used_at": "2026-03-14T15:45:00Z",
        "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
        "ip_address": "192.168.1.100"
      }
    ],
    "total": 1
  }
}
```

**DELETE /api/v1/auth/sessions/{id}**
```json
{
  "status": "success",
  "data": {
    "message": "Session revoked successfully",
    "session_id": "sess_abc123def456"
  }
}
```

**DELETE /api/v1/auth/sessions (revoke all others)**
```json
{
  "status": "success",
  "data": {
    "message": "All other sessions revoked",
    "revoked_count": 3
  }
}
```

### Error Codes

From ADR-017 Error Code System:
- CC-AUTH-101: Invalid credentials
- CC-AUTH-103: Invalid/expired token
- CC-AUTH-107: Resource not found (session not found or not owned)

### Project Structure Notes

**Files to Modify:**
- `src/services/auth_service.rs` — Add list_sessions, revoke_session methods
- `src/routes/auth.rs` — Add session endpoints
- `src/db/sqlite.rs` — Add migration for new columns
- `src/main.rs` — Wire up new routes (if not auto-mounted)

**New Files (optional):**
- `src/models/session.rs` — Session DTOs (can also go in auth.rs)

### Testing Standards

**Unit Tests (in auth_service.rs):**
```rust
#[cfg(test)]
mod tests {
    #[test]
    async fn test_list_sessions_excludes_revoked() { ... }
    #[test]
    async fn test_revoke_session_owner_only() { ... }
}
```

**Integration Tests (tests/test_sessions.rs):**
- Use existing test patterns from `tests/common/mod.rs`
- Test session lifecycle with authenticated requests
- Test cross-user access prevention

### Dependencies

**Stories That Must Be Complete:**
- Story 14.1: User Registration and Login ✅ (refresh_tokens table exists)
- Story 14.2: Role-Based Access Control ✅ (JwtAuth middleware exists)

**No New Dependencies Required** — Uses existing sqlx, actix-web, serde

### Security Considerations

1. **Session Isolation:** Users can only see/revoke their own sessions
2. **Token Hash Security:** Never expose token_hash in API responses
3. **Current Session Preservation:** When revoking "all other sessions", preserve the current session
4. **Audit Trail:** Consider logging session revocations for security auditing

### References

- [Source: `_bmad-output/planning-artifacts/architecture.md#ADR-008`] - JWT + Refresh Token pattern
- [Source: `_bmad-output/planning-artifacts/epics-phase3.md#Story-14.4`] - Story definition
- [Source: `src/services/auth_service.rs`] - Existing logout methods
- [Source: `src/routes/auth.rs`] - Auth route patterns
- [Source: `src/db/sqlite.rs`] - Database migration patterns
- [Source: Story 14-1 implementation] - refresh_tokens table schema

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6-20250528)

### Debug Log References

None - all tests passed.

### Completion Notes List

- Task 1: Created `src/models/session.rs` with Session, SessionInfo, SessionListResponse, SessionRevokeResponse, and RevokeAllSessionsResponse DTOs
- Task 2: Added ALTER TABLE migration for last_used_at, user_agent, ip_address columns to refresh_tokens
- Task 3: Added list_sessions, revoke_session, revoke_all_other_sessions methods to AuthService with SessionNotFound error variant
- Task 4: Added list_sessions, revoke_session, revoke_all_other_sessions routes to auth.rs
- Task 5: Wired session routes in main.rs under /api/v1/auth scope with JwtAuth middleware
- Task 6: Unit tests in session model (4 tests) + auth_service session tests (3 tests)
- Task 7: Integration tests in test_sessions.rs (5 tests)
- Total tests: 227 passed, 0 failed

**Code Review Fixes Applied:**
1. ✅ Fixed `last_used_at` not being updated on token refresh
2. ✅ Added unit tests for Session DTOs (SessionInfo, SessionListResponse, SessionRevokeResponse, RevokeAllSessionsResponse)
3. ✅ Fixed integration test to use actual session ID from list response
4. ✅ Added integration test for revoke-all-other-sessions with X-Current-Session header
5. ✅ Added #[allow(dead_code)] to jwt_secret field to suppress warning
6. ✅ Updated session ID format documentation (rt_{uuid} not sess_{uuid})

### File List

- `src/models/session.rs` — NEW: Session DTOs and response types
- `src/models/mod.rs` — MODIFY: Add session module
- `src/services/auth_service.rs` — MODIFY: Add session management methods, SessionNotFound error, last_used_at update, unit tests
- `src/routes/auth.rs` — MODIFY: Add session management endpoints and SessionNotFound error handler
- `src/db/sqlite.rs` — MODIFY: Add metadata columns migration to refresh_tokens
- `src/main.rs` — MODIFY: Wire session routes under /api/v1/auth
- `tests/test_sessions.rs` — NEW: Session management integration tests (5 tests)
