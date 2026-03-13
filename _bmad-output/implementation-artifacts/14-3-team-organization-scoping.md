# Story 14.3: Team/Organization Scoping

Status: review

## Story

As a **System Administrator**,
I want to create teams and assign users/devices to them,
so that access is controlled at the organizational level.

## Acceptance Criteria

```gherkin
Scenario: Admin creates a new team
  Given an admin user is authenticated
  When a POST request is sent to /api/v1/admin/teams with:
    | Authorization: Bearer <admin_access_token>
    | name: "Marketing Team Alpha"
    | description: "Primary marketing operations team"
  Then the response status is 201 Created
  And the response contains a team object with id, name, description
  And an audit log entry is created for team creation

Scenario: Non-admin attempts team creation
  Given a non-admin user (role "agent") is authenticated
  When a POST request is sent to /api/v1/admin/teams with:
    | Authorization: Bearer <agent_access_token>
    | name: "Unauthorized Team"
  Then the response status is 403 Forbidden
  And the error code is CC-AUTH-104
  And the error message is "Insufficient permissions"

Scenario: Admin assigns user to team
  Given an admin user is authenticated
  And a team exists with id "team_abc123"
  And a user exists with id "user_xyz789" and team_id is null
  When a POST request is sent to /api/v1/admin/teams/team_abc123/members with:
    | Authorization: Bearer <admin_access_token>
    | user_id: "user_xyz789"
  Then the response status is 200 OK
  And the user's team_id is updated to "team_abc123"
  And an audit log entry is created for team membership change

Scenario: Admin assigns device to team
  Given an admin user is authenticated
  And a team exists with id "team_abc123"
  And a device exists with udid "CC-047" and team_id is null
  When a POST request is sent to /api/v1/admin/devices/CC-047/team with:
    | Authorization: Bearer <admin_access_token>
    | team_id: "team_abc123"
  Then the response status is 200 OK
  And the device's team_id is updated to "team_abc123"
  And an audit log entry is created for device team assignment

Scenario: Admin removes device from team
  Given an admin user is authenticated
  And a device exists with udid "CC-047" and team_id "team_abc123"
  When a DELETE request is sent to /api/v1/admin/devices/CC-047/team with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And the device's team_id is set to null
  And an audit log entry is created for device team removal

Scenario: User sees only their team's devices - Agent role
  Given a user with role "agent" is authenticated
  And the user's team_id is "team_abc123"
  And device "CC-001" has team_id "team_abc123"
  And device "CC-002" has team_id "team_xyz789"
  And device "CC-003" has team_id null
  When a GET request is sent to /api/v1/devices with:
    | Authorization: Bearer <agent_access_token>
  Then the response status is 200 OK
  And only device "CC-001" is returned
  And devices "CC-002" and "CC-003" are not visible

Scenario: Admin sees all devices regardless of team
  Given an admin user is authenticated
  And multiple devices exist with different team_ids
  When a GET request is sent to /api/v1/devices with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And all devices are returned regardless of team assignment

Scenario: Admin lists all teams
  Given an admin user is authenticated
  And multiple teams exist
  When a GET request is sent to /api/v1/admin/teams with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And all teams are returned with member counts

Scenario: Admin gets team details with members
  Given an admin user is authenticated
  And a team exists with id "team_abc123" and 3 members
  When a GET request is sent to /api/v1/admin/teams/team_abc123 with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And the response contains team details with member list
  And the response contains device count for the team

Scenario: Admin updates team information
  Given an admin user is authenticated
  And a team exists with id "team_abc123"
  When a PUT request is sent to /api/v1/admin/teams/team_abc123 with:
    | Authorization: Bearer <admin_access_token>
    | name: "Marketing Team Alpha - Updated"
    | description: "Updated description"
  Then the response status is 200 OK
  And the team's name and description are updated

Scenario: Admin deletes empty team
  Given an admin user is authenticated
  And a team exists with id "team_empty" and 0 members and 0 devices
  When a DELETE request is sent to /api/v1/admin/teams/team_empty with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And the team is removed from the database

Scenario: Admin cannot delete team with members
  Given an admin user is authenticated
  And a team exists with id "team_abc123" and 3 members
  When a DELETE request is sent to /api/v1/admin/teams/team_abc123 with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 400 Bad Request
  And the error indicates team has members and cannot be deleted

Scenario: Admin removes user from team
  Given an admin user is authenticated
  And a user with id "user_xyz789" has team_id "team_abc123"
  When a DELETE request is sent to /api/v1/admin/teams/team_abc123/members/user_xyz789 with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And the user's team_id is set to null
  And an audit log entry is created

Scenario: User without team sees no devices
  Given a user with role "agent" is authenticated
  And the user's team_id is null
  When a GET request is sent to /api/v1/devices with:
    | Authorization: Bearer <agent_access_token>
  Then the response status is 200 OK
  And an empty device list is returned
```

## Tasks / Subtasks

- [x] Task 1: Team database schema (AC: all)
  - [x] Add teams table in `src/db/sqlite.rs` within `ensure_initialized()`
  - [x] Schema: id (TEXT PRIMARY KEY), name (TEXT NOT NULL), description (TEXT), created_at, updated_at
  - [x] Add team_id column to devices table (if not exists)
  - [x] Create index on devices(team_id) for team-filtered queries
  - [x] Note: users table already has team_id from Story 14.1

- [x] Task 2: Team model and DTOs (AC: all)
  - [x] Create `src/models/team.rs` with Team, NewTeam, UpdateTeam structs
  - [x] Implement sqlx::FromRow for Team
  - [x] Create TeamMember, TeamDetails response DTOs
  - [x] Add team_id filtering support to existing models
  - [x] Update `src/models/mod.rs` to export team module

- [x] Task 3: Team service implementation (AC: all)
  - [x] Create `src/services/team_service.rs`
  - [x] Implement create_team() method
  - [x] Implement update_team() method
  - [x] Implement delete_team() with member/device check
  - [x] Implement get_team() and list_teams() methods
  - [x] Implement add_member() and remove_member() methods
  - [x] Implement assign_device() and remove_device() methods
  - [x] Implement get_team_members() and get_team_devices() methods
  - [x] Update `src/services/mod.rs` to export team_service

- [x] Task 4: Team admin routes (AC: team CRUD)
  - [x] Add team endpoints to `src/routes/admin.rs`
  - [x] POST /api/v1/admin/teams - create team
  - [x] GET /api/v1/admin/teams - list all teams
  - [x] GET /api/v1/admin/teams/{id} - get team details
  - [x] PUT /api/v1/admin/teams/{id} - update team
  - [x] DELETE /api/v1/admin/teams/{id} - delete team
  - [x] POST /api/v1/admin/teams/{id}/members - add member
  - [x] DELETE /api/v1/admin/teams/{id}/members/{user_id} - remove member
  - [x] Require AdminTeams permission for all team endpoints

- [x] Task 5: Device team assignment routes (AC: device assignment)
  - [x] Add device team endpoints to `src/routes/admin.rs`
  - [x] PUT /api/v1/admin/devices/{udid}/team - assign/remove device to/from team
  - [x] Validate device exists before assignment
  - [x] Create audit log for device team changes

- [x] Task 6: Team-scoped device filtering (AC: device visibility)
  - [x] Modify device list query to filter by user's team_id
  - [x] Admin role bypasses team filtering (sees all)
  - [x] Add optional team_id field to DeviceInfo response
  - [x] Update existing device routes to apply team scoping
  - [ ] Ensure WebSocket subscriptions respect team scoping (partial)

- [ ] Task 7: Team-scoped profile filtering (AC: profile visibility) - **BLOCKED by Story 15-1**
  - [ ] Modify profile list query to filter by user's team_id
  - [ ] Admin role bypasses team filtering (sees all)
  - [ ] Owner-based filtering AND team-based filtering combined
  - [ ] Profiles owned by user OR in user's team are visible
  - **Note**: Profile system does not exist yet (Story 15-1 is in backlog)

- [x] Task 8: State and service integration (AC: all)
  - [x] Add team_service to AppState
  - [x] Initialize team_service in main.rs with database pool
  - [x] Update existing services to respect team scoping
  - [x] Add team_service reference where needed

- [x] Task 9: Audit logging for team operations (AC: all)
  - [x] Log team creation, update, deletion
  - [x] Log member additions and removals
  - [x] Log device assignments and removals
  - [x] Include: admin_user_id, team_id, target_id, action, timestamp

- [x] Task 10: Unit tests (AC: all)
  - [x] Test Team model serialization/deserialization
  - [x] Test team service CRUD operations
  - [x] Test team scoping logic in device queries
  - [x] Test admin bypass for team filtering

- [x] Task 11: Integration tests (AC: all)
  - [x] Create `tests/test_teams.rs`
  - [x] Test team creation flow
  - [x] Test member assignment flow
  - [x] Test device assignment flow
  - [x] Test team-scoped device visibility
  - [x] Test admin sees all devices
  - [x] Test team deletion constraints

## Dev Notes

### Architecture Patterns (MUST FOLLOW)

**From `architecture.md`:**

1. **Team Scoping (ADR-008 extension)**

   ```rust
   // Team model
   #[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
   pub struct Team {
       pub id: String,           // team_{uuid}
       pub name: String,
       pub description: Option<String>,
       pub created_at: String,
       pub updated_at: String,
   }

   // Team membership is stored in users.team_id (single team per user in MVP)
   // Device assignment is stored in devices.team_id
   ```

2. **Database Schema**

   ```sql
   -- Teams table (NEW in Story 14.3)
   CREATE TABLE teams (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL,
       description TEXT,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
   );

   CREATE INDEX idx_teams_name ON teams(name);

   -- Users table (from Story 14.1 - team_id already exists)
   -- team_id column already present, just needs to be utilized

   -- Devices table extension (add team_id column)
   ALTER TABLE devices ADD COLUMN team_id TEXT REFERENCES teams(id);
   CREATE INDEX idx_devices_team ON devices(team_id);
   ```

3. **Error Codes (CC-AUTH-xxx)**
   - CC-AUTH-104: Insufficient permissions (403) — from Story 14.2
   - CC-AUTH-107: User not found (404) — from Story 14.1
   - CC-DEV-301: Device not found (404)
   - CC-SYS-901: Validation error (400)

4. **API Response Format**

   ```json
   // Success - Team creation
   {
     "id": "team_abc123def456",
     "name": "Marketing Team Alpha",
     "description": "Primary marketing operations team",
     "created_at": "2026-03-13T10:30:00Z",
     "updated_at": "2026-03-13T10:30:00Z"
   }

   // Success - Team details with members
   {
     "id": "team_abc123def456",
     "name": "Marketing Team Alpha",
     "description": "Primary marketing operations team",
     "member_count": 3,
     "device_count": 5,
     "members": [
       {"id": "user_001", "email": "maria@example.com", "role": "agent"},
       {"id": "user_002", "email": "chen@example.com", "role": "viewer"}
     ],
     "created_at": "2026-03-13T10:30:00Z"
   }

   // Error - Cannot delete team with members
   {
     "error": {
       "code": "CC-SYS-901",
       "message": "Validation error",
       "details": "Cannot delete team with 3 members. Remove members first.",
       "request_id": "req_xyz789",
       "timestamp": "2026-03-13T14:30:00Z"
     }
   }
   ```

### Project Structure Notes

**New Files:**
```
src/
├── models/
│   └── team.rs           # NEW: Team model and DTOs
├── services/
│   └── team_service.rs   # NEW: Team CRUD and membership operations
```

**Files to Modify:**
- `src/db/sqlite.rs` — Add teams table creation, devices.team_id column
- `src/models/mod.rs` — Export team module
- `src/services/mod.rs` — Export team_service module
- `src/routes/admin.rs` — Add team management endpoints
- `src/routes/api_v1.rs` — Modify device queries for team scoping
- `src/state.rs` — Add team_service to AppState
- `src/main.rs` — Initialize team_service

### API Request/Response Examples

**POST /api/v1/admin/teams**
```json
// Request
{
  "name": "Marketing Team Alpha",
  "description": "Primary marketing operations team"
}

// Response 201 Created
{
  "status": "success",
  "data": {
    "id": "team_abc123def456",
    "name": "Marketing Team Alpha",
    "description": "Primary marketing operations team",
    "created_at": "2026-03-13T10:30:00Z",
    "updated_at": "2026-03-13T10:30:00Z"
  }
}
```

**GET /api/v1/admin/teams**
```json
// Response 200 OK
{
  "status": "success",
  "data": {
    "teams": [
      {
        "id": "team_abc123",
        "name": "Marketing Team Alpha",
        "member_count": 3,
        "device_count": 5
      },
      {
        "id": "team_xyz789",
        "name": "QA Team",
        "member_count": 2,
        "device_count": 3
      }
    ]
  }
}
```

**POST /api/v1/admin/teams/{id}/members**
```json
// Request
{
  "user_id": "user_xyz789abc123"
}

// Response 200 OK
{
  "status": "success",
  "data": {
    "id": "user_xyz789abc123",
    "email": "maria@example.com",
    "role": "agent",
    "team_id": "team_abc123def456"
  }
}
```

**POST /api/v1/admin/devices/{udid}/team**
```json
// Request
{
  "team_id": "team_abc123def456"
}

// Response 200 OK
{
  "status": "success",
  "data": {
    "udid": "CC-047",
    "team_id": "team_abc123def456",
    "message": "Device assigned to team"
  }
}
```

### Team Scoping Logic

**Device Query Modification:**
```rust
// In device list query (pseudo-code)
fn list_devices_for_user(user: &AuthenticatedUser) -> Vec<Device> {
    if user.role == UserRole::Admin {
        // Admin sees all devices
        sqlx::query_as("SELECT * FROM devices")
    } else {
        // Non-admin sees only their team's devices
        sqlx::query_as("SELECT * FROM devices WHERE team_id = ?")
            .bind(user.team_id)
    }
}
```

**Profile Query Modification:**
```rust
// In profile list query (pseudo-code)
fn list_profiles_for_user(user: &AuthenticatedUser) -> Vec<Profile> {
    if user.role == UserRole::Admin {
        // Admin sees all profiles
        sqlx::query_as("SELECT * FROM profiles")
    } else {
        // Non-admin sees own profiles OR team's profiles
        sqlx::query_as("SELECT * FROM profiles WHERE owner_id = ? OR team_id = ?")
            .bind(&user.id)
            .bind(user.team_id)
    }
}
```

### RBAC Integration

**From Story 14.2:**
- Use `RequireRole::admin()` extractor for admin-only endpoints
- Use `check_permission(req, Permission::AdminTeams)` for team operations
- Return CC-AUTH-104 for insufficient permissions

**Permission Matrix Update:**

| Permission | Admin | Agent | Viewer | Renter |
|------------|-------|-------|--------|--------|
| AdminTeams | ✅ | ❌ | ❌ | ❌ |
| DeviceRead | ✅ (all) | ✅ (team) | ✅ (team) | ✅ (team) |
| DeviceWrite | ✅ (all) | ✅ (team) | ❌ | ✅ (team) |
| ProfileRead | ✅ (all) | ✅ (own+team) | ✅ (own+team) | ✅ (own+team) |

### Existing Patterns to Follow

**From `src/services/auth_service.rs` (Story 14.1):**
- Use `sqlx::query_as` for typed queries
- Return custom error types (create TeamError if needed)
- Use tracing for audit logging

**From `src/routes/auth.rs` (Story 14.1):**
- Use `web::Json<T>` for request/response
- Return `HttpResponse::Created().json()` for 201 responses
- Use `auth_error_to_response()` pattern for errors

**From `src/db/sqlite.rs`:**
- Add table creation to `ensure_initialized()` method
- Use `CREATE TABLE IF NOT EXISTS` pattern
- Create indexes immediately after table creation

### Dependencies

**Stories That Must Be Complete:**
- Story 14.1: User Registration and Login ✅
  - Users table with team_id column exists
  - JWT middleware provides AuthenticatedUser
- Story 14.2: Role-Based Access Control
  - RBAC middleware with RequireRole extractor
  - Permission enum with AdminTeams

**New Dependencies (add to Cargo.toml if not present):**
- None required — uses existing sqlx, actix-web, serde

### Testing Standards

**Unit Tests (inline in source files):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_team_id() {
        let id = generate_team_id();
        assert!(id.starts_with("team_"));
    }

    #[test]
    fn test_team_scoping_filters_devices() {
        // Test that non-admin only sees team devices
    }
}
```

**Integration Tests (`tests/test_teams.rs`):**
- Test team CRUD flow
- Test member assignment
- Test device assignment
- Test team-scoped visibility
- Test deletion constraints
- Use test database pattern from `tests/common/mod.rs`

### Migration Considerations

**Database Migration Strategy:**
- Teams table created in `ensure_initialized()`
- Devices table ALTER to add team_id column
- Handle existing devices: default team_id to NULL
- No data migration required (new feature)

**Backward Compatibility:**
- Existing users have team_id = NULL (no team)
- Existing devices have team_id = NULL (no team)
- Users without team see empty device list (expected behavior)
- Admin can assign users and devices to teams post-deployment

### References

- [Source: `_bmad-output/planning-artifacts/architecture.md#ADR-008`] - Team scoping pattern
- [Source: `_bmad-output/planning-artifacts/epics-phase3.md#Story-14.3`] - Story definition
- [Source: `_bmad-output/planning-artifacts/prd-phase3-cloudcontrol-rust.md#FR5-FR6`] - Functional requirements
- [Source: `src/services/auth_service.rs`] - Auth service patterns
- [Source: `src/db/sqlite.rs`] - Database initialization patterns
- [Source: Story 14-1 implementation] - Users table schema with team_id
- [Source: Story 14-2 story file] - RBAC patterns and permission system

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### Remaining Work

1. **Task 7: Team-scoped profile filtering** - **BLOCKED** by Story 15-1 (Profile Data Model and Storage). The profile system does not exist yet. This task will be addressed when the profile system is implemented.
2. **Task 6 (partial): WebSocket team scoping** - WebSocket subscriptions (`/video/convert`, `/scrcpy/{udid}/ws`) currently lack authentication middleware. Team scoping for WebSockets would require:
   - JWT authentication on WebSocket handshake
   - Device team ownership verification before connection
   - This is deferred as the core AC scenarios for device visibility via REST API are complete.

### File List

- `src/models/team.rs` — NEW: Team model and DTOs (Team, CreateTeamRequest, UpdateTeamRequest, etc.)
- `src/models/mod.rs` — MODIFY: Export team module
- `src/models/api_response.rs` — MODIFY: Add team_id field to DeviceInfo
- `src/models/user.rs` — MODIFY: User struct with team_id field
- `src/services/team_service.rs` — NEW: Team CRUD service with audit logging
- `src/services/mod.rs` — MODIFY: Export team_service module
- `src/services/auth_service.rs` — MODIFY: User management with team support
- `src/routes/admin.rs` — NEW: Admin routes for team management endpoints
- `src/routes/mod.rs` — MODIFY: Export admin module
- `src/routes/api_v1.rs` — MODIFY: Add team scoping to device queries
- `src/middleware.rs` — MODIFY: RequireAdmin extractor for team auth
- `src/db/sqlite.rs` — MODIFY: Add teams table, audit_log table, devices.team_id
- `src/state.rs` — MODIFY: Add team_service to AppState
- `src/main.rs` — MODIFY: Initialize team_service, wire admin routes
- `tests/test_teams.rs` — NEW: Team integration tests
- `tests/common/mod.rs` — MODIFY: Team test helper functions
