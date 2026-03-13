# Story 14.2: Role-Based Access Control

Status: done

## Story

As a **System Administrator**,
I want to assign roles to users and control feature access,
so that users only see and do what they're authorized for.

## Acceptance Criteria

```gherkin
Scenario: Admin assigns role to user
  Given an admin user is authenticated
  And a user exists with id "user_abc123" and role "agent"
  When a POST request is sent to /api/v1/admin/users/user_abc123/role with:
    | Authorization: Bearer <admin_access_token>
    | role: "viewer"
  Then the response status is 200 OK
  And the user's role is updated to "viewer"
  And the response contains the updated user object
  And an audit log entry is created for role change

Scenario: Non-admin attempts role assignment
  Given a non-admin user (role "agent") is authenticated
  And a user exists with id "user_xyz789"
  When a POST request is sent to /api/v1/admin/users/user_xyz789/role with:
    | Authorization: Bearer <agent_access_token>
    | role: "admin"
  Then the response status is 403 Forbidden
  And the error code is CC-AUTH-104
  And the error message is "Insufficient permissions"
  And the user's role is unchanged

Scenario: Admin assigns invalid role
  Given an admin user is authenticated
  And a user exists with id "user_abc123"
  When a POST request is sent to /api/v1/admin/users/user_abc123/role with:
    | Authorization: Bearer <admin_access_token>
    | role: "superuser"
  Then the response status is 400 Bad Request
  And the error indicates invalid role value

Scenario: Role-based device visibility - Agent role
  Given a user with role "agent" is authenticated
  When a GET request is sent to /api/v1/devices with:
    | Authorization: Bearer <agent_access_token>
  Then the response status is 200 OK
  And the user can see all devices (no tenant filtering in MVP)
  And the user can perform device operations (tap, swipe, text)

Scenario: Role-based device visibility - Viewer role
  Given a user with role "viewer" is authenticated
  When a GET request is sent to /api/v1/devices with:
    | Authorization: Bearer <viewer_access_token>
  Then the response status is 200 OK
  And the user can see all devices
  And write operations return 403 with CC-AUTH-104

Scenario: Role-based profile visibility - Owner scoping
  Given a user with role "agent" is authenticated
  And the user owns profile "prof_abc123"
  And another user owns profile "prof_xyz789"
  When a GET request is sent to /api/v1/profiles with:
    | Authorization: Bearer <agent_access_token>
  Then the response status is 200 OK
  And only profiles owned by the user are returned
  And profiles owned by other users are not visible

Scenario: Admin can view all profiles
  Given an admin user is authenticated
  And multiple users own profiles
  When a GET request is sent to /api/v1/profiles with:
    | Authorization: Bearer <admin_access_token>
  Then the response status is 200 OK
  And all profiles across all users are returned

Scenario: Viewer cannot create profiles
  Given a user with role "viewer" is authenticated
  When a POST request is sent to /api/v1/profiles with:
    | Authorization: Bearer <viewer_access_token>
    | name: "Test Profile"
  Then the response status is 403 Forbidden
  And the error code is CC-AUTH-104

Scenario: Renter role - limited device access
  Given a user with role "renter" is authenticated
  And devices CC-001 through CC-010 are assigned to renter's team
  When a GET request is sent to /api/v1/devices with:
    | Authorization: Bearer <renter_access_token>
  Then the response status is 200 OK
  And only devices assigned to the renter's team are returned

Scenario: Self-role modification prevented
  Given an admin user is authenticated with id "admin_001"
  When a POST request is sent to /api/v1/admin/users/admin_001/role with:
    | Authorization: Bearer <admin_access_token>
    | role: "agent"
  Then the response status is 400 Bad Request
  And the error message indicates cannot modify own role
```

## Tasks / Subtasks

- [x] Task 1: Define UserRole enum and permissions (AC: all)
  - [x] Add UserRole enum to `src/models/user.rs` with variants: Admin, Agent, Viewer, Renter
  - [x] Implement Display and FromStr for UserRole
  - [x] Add sqlx mapping for UserRole (TEXT column)
  - [x] Define Permission enum with all system permissions
  - [x] Implement `UserRole::permissions()` returning granted permissions
  - [x] Add unit tests for role-permission mapping

- [x] Task 2: Extend auth middleware with RBAC (AC: role-based access)
  - [x] Create RBAC extractors in `src/middleware.rs` (RequireAdmin, RequireAnyRole, check_permission)
  - [x] Implement `RequireAdmin` extractor for admin-only endpoints
  - [x] Implement `RequireAnyRole` for non-viewer access
  - [x] Add `check_permission()` helper function for permission checks
  - [x] Return 403 with CC-AUTH-104 for insufficient permissions
  - [x] Add `get_user_role()` helper function

- [x] Task 3: Admin role assignment endpoint (AC: role assignment)
  - [x] Create `src/routes/admin.rs` for admin-only endpoints
  - [x] Implement POST /api/v1/admin/users/{id}/role handler
  - [x] Add RoleAssignmentRequest and RoleAssignmentResponse DTOs
  - [x] Validate role is valid UserRole variant
  - [x] Prevent self-role modification
  - [x] Update user's role in database via AuthService::update_user_role()
  - [x] Add tracing audit log for role changes
  - [x] Implement GET /api/v1/admin/users to list users
  - [x] Update `src/routes/mod.rs` to include admin module
  - [x] Register admin routes in `src/main.rs`

- [ ] Task 4: Profile ownership scoping (AC: profile visibility)
  > **BLOCKED**: Profile model and endpoints not yet implemented (Epic 15)
  > Will implement when Epic 15 (Profile Management System) is complete
  - [ ] Add owner_id filtering to profile list queries
  - [ ] Admin role bypasses owner filtering
  - [ ] Viewer role can view but not modify profiles
  - [ ] Implement `ProfileStore::list_for_user()` method
  - [ ] Add permission check on profile CRUD operations

- [x] Task 5: Device access control (AC: device visibility)
  - [x] Add permission checks to device operation endpoints (tap, swipe, input, keyevent)
  - [x] Viewer role: read-only access to devices (RequireAnyRole blocks write ops)
  - [x] Agent role: full device operations
  - [ ] Renter role: team-scoped device access (preparation for 14.3 - needs team filtering)
  - [x] Return 403 for unauthorized write operations (via RequireAnyRole extractor)

- [x] Task 6: Permission constants and checks (AC: all)
  - [x] Define Permission enum in `src/models/user.rs`
  - [x] Permissions: DeviceRead, DeviceWrite, ProfileRead, ProfileWrite, ProfileCheckout, AdminUsers, AdminAudit, AdminTeams
  - [x] Implement `UserRole::has_permission()` helper method
  - [x] Implement `check_permission()` helper in middleware

- [x] Task 7: Audit logging for role changes (AC: role assignment)
  - [x] Log role assignment events via tracing
  - [x] Include: admin_user_id, target_user_id, new_role
  - [x] Basic audit logging in admin routes

- [x] Task 8: Unit tests (AC: all)
  - [x] Test UserRole enum parsing and display (in models/user.rs)
  - [x] Test permission mapping for each role (in models/user.rs)
  - [x] Test RequireAdmin/RequireAnyRole extractors (in middleware.rs)
  - [x] Test role assignment validation (in routes/admin.rs)

- [x] Task 9: Integration tests (AC: all)
  - [x] Create `tests/test_rbac.rs`
  - [x] Test admin role assignment requires admin role
  - [x] Test non-admin rejection for user listing
  - [x] Test role validation in assignment

- [x] Task 10: Update existing routes with RBAC (AC: role-based access)
  - [x] Add RBAC checks to `/api/v1/devices/*` write endpoints (tap, swipe, input, keyevent)
  - [ ] Add RBAC checks to `/api/v1/profiles/*` endpoints (when implemented in Epic 15)
  - [x] Add RBAC checks to `/api/v1/batch/*` endpoints (batch_tap, batch_swipe, batch_input)
  - [x] Document which roles can access which endpoints (see Endpoint Protection Matrix)

## Dev Notes

### Architecture Patterns (MUST FOLLOW)

**From `architecture.md`:**

1. **RBAC Roles and Permissions (ADR-008 extension)**

   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
   #[sqlx(type_name = "TEXT", rename_all = "lowercase")]
   pub enum UserRole {
       Admin,   // Full access to all resources
       Agent,   // Device operations, own profiles
       Viewer,  // Read-only access
       Renter,  // Team-scoped device access (for Growth phase)
   }

   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum Permission {
       // Device permissions
       DeviceRead,
       DeviceWrite,

       // Profile permissions
       ProfileRead,
       ProfileWrite,
       ProfileCheckout,

       // Admin permissions
       AdminUsers,
       AdminAudit,
       AdminTeams,
   }

   impl UserRole {
       pub fn permissions(&self) -> Vec<Permission> {
           match self {
               UserRole::Admin => vec![
                   Permission::DeviceRead, Permission::DeviceWrite,
                   Permission::ProfileRead, Permission::ProfileWrite,
                   Permission::ProfileCheckout,
                   Permission::AdminUsers, Permission::AdminAudit,
                   Permission::AdminTeams,
               ],
               UserRole::Agent => vec![
                   Permission::DeviceRead, Permission::DeviceWrite,
                   Permission::ProfileRead, Permission::ProfileWrite,
                   Permission::ProfileCheckout,
               ],
               UserRole::Viewer => vec![
                   Permission::DeviceRead,
                   Permission::ProfileRead,
               ],
               UserRole::Renter => vec![
                   Permission::DeviceRead, Permission::DeviceWrite,
                   Permission::ProfileRead, Permission::ProfileWrite,
                   Permission::ProfileCheckout,
               ],
           }
       }

       pub fn has_permission(&self, permission: Permission) -> bool {
           self.permissions().contains(&permission)
       }
   }
   ```

2. **Error Codes (CC-AUTH-xxx)**
   - CC-AUTH-101: Invalid credentials (401) — from Story 14.1
   - CC-AUTH-102: Token expired (401) — from Story 14.1
   - CC-AUTH-103: Token revoked (401) — from Story 14.1
   - **CC-AUTH-104: Insufficient permissions (403)** — NEW in this story
   - CC-AUTH-105: Email already exists (409) — from Story 14.1

3. **Database Schema (already exists from 14.1)**
   ```sql
   -- Users table (from Story 14.1)
   CREATE TABLE users (
       id TEXT PRIMARY KEY,
       email TEXT UNIQUE NOT NULL,
       password_hash TEXT NOT NULL,
       role TEXT NOT NULL DEFAULT 'agent',  -- Uses this field
       team_id TEXT,
       created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
       last_login_at TIMESTAMP
   );
   ```

4. **API Response Format**
   ```json
   // Success - Role assignment
   {
     "id": "user_abc123",
     "email": "maria@example.com",
     "role": "viewer",
     "updated_at": "2026-03-13T14:30:00Z"
   }

   // Error - Insufficient permissions
   {
     "error": {
       "code": "CC-AUTH-104",
       "message": "Insufficient permissions",
       "details": "This action requires admin role",
       "request_id": "req_xyz789",
       "timestamp": "2026-03-13T14:30:00Z"
     }
   }
   ```

### Project Structure Notes

**New Files:**
```
src/
├── middleware/
│   ├── mod.rs            # MODIFY: export rbac module
│   └── rbac.rs           # NEW: Role-based access control middleware
├── routes/
│   ├── mod.rs            # MODIFY: include admin routes
│   └── admin.rs          # NEW: Admin-only endpoints
└── models/
    └── user.rs           # MODIFY: Add Permission enum, permissions()
```

**Files to Modify:**
- `src/models/user.rs` — Add Permission enum, UserRole::permissions()
- `src/middleware/mod.rs` — Export rbac module
- `src/routes/mod.rs` — Include admin routes
- `src/main.rs` — Register admin routes
- Existing route handlers — Add RBAC checks where needed

### API Request/Response Examples

**POST /api/v1/admin/users/{id}/role**
```json
// Request
{
  "role": "viewer"
}

// Response 200 OK
{
  "id": "user_abc123def456",
  "email": "maria@example.com",
  "role": "viewer",
  "team_id": null,
  "created_at": "2026-03-13T10:30:00Z",
  "updated_at": "2026-03-13T14:30:00Z"
}

// Response 403 Forbidden (non-admin)
{
  "error": {
    "code": "CC-AUTH-104",
    "message": "Insufficient permissions",
    "details": "This action requires admin role",
    "request_id": "req_xyz789",
    "timestamp": "2026-03-13T14:30:00Z"
  }
}

// Response 400 Bad Request (invalid role)
{
  "error": {
    "code": "CC-SYS-901",
    "message": "Validation error",
    "details": "Invalid role: 'superuser'. Valid roles: admin, agent, viewer, renter",
    "request_id": "req_xyz789",
    "timestamp": "2026-03-13T14:30:00Z"
  }
}

// Response 400 Bad Request (self-modification)
{
  "error": {
    "code": "CC-SYS-901",
    "message": "Validation error",
    "details": "Cannot modify your own role",
    "request_id": "req_xyz789",
    "timestamp": "2026-03-13T14:30:00Z"
  }
}
```

### RBAC Middleware Pattern

```rust
// src/middleware/rbac.rs
use actix_web::{dev::Payload, Error, FromRequest, HttpRequest, HttpResponse};
use actix_web::error::ErrorForbidden;
use serde::Deserialize;
use std::future::{ready, Ready};

use crate::models::user::{UserRole, Permission};
use crate::middleware::auth::AuthenticatedUser;

/// Extractor that requires specific role
pub struct RequireRole {
    pub user: AuthenticatedUser,
}

impl RequireRole {
    pub fn admin() -> Self {
        Self { user: AuthenticatedUser::default() }
    }
}

impl FromRequest for RequireRole {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        // Get authenticated user from extensions (set by auth middleware)
        let user = req.extensions().get::<AuthenticatedUser>().cloned();

        match user {
            Some(user) if user.role == UserRole::Admin => {
                ready(Ok(RequireRole { user }))
            }
            Some(user) => {
                ready(Err(ErrorForbidden(ApiError::auth_104())))
            }
            None => {
                ready(Err(ErrorForbidden(ApiError::auth_104())))
            }
        }
    }
}

/// Check if user has specific permission
pub fn check_permission(req: &HttpRequest, permission: Permission) -> Result<AuthenticatedUser, Error> {
    let user = req.extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| ErrorForbidden(ApiError::auth_104()))?;

    if user.role.has_permission(permission) {
        Ok(user)
    } else {
        Err(ErrorForbidden(ApiError::auth_104()))
    }
}
```

### Permission Matrix

| Permission | Admin | Agent | Viewer | Renter |
|------------|-------|-------|--------|--------|
| DeviceRead | ✅ | ✅ | ✅ | ✅ (team) |
| DeviceWrite | ✅ | ✅ | ❌ | ✅ (team) |
| ProfileRead | ✅ | ✅ (own) | ✅ (own) | ✅ (own) |
| ProfileWrite | ✅ | ✅ (own) | ❌ | ✅ (own) |
| ProfileCheckout | ✅ | ✅ | ❌ | ✅ (team) |
| AdminUsers | ✅ | ❌ | ❌ | ❌ |
| AdminAudit | ✅ | ❌ | ❌ | ❌ |
| AdminTeams | ✅ | ❌ | ❌ | ❌ |

### Endpoint Protection Matrix

| Endpoint | Admin | Agent | Viewer | Renter |
|----------|-------|-------|--------|--------|
| GET /devices | ✅ | ✅ | ✅ | ✅ (team) |
| POST /devices/{id}/tap | ✅ | ✅ | ❌ | ✅ (team) |
| POST /devices/{id}/swipe | ✅ | ✅ | ❌ | ✅ (team) |
| GET /profiles | ✅ (all) | ✅ (own) | ✅ (own) | ✅ (own) |
| POST /profiles | ✅ | ✅ | ❌ | ✅ |
| POST /profiles/{id}/check-out | ✅ | ✅ | ❌ | ✅ (team) |
| POST /admin/users/{id}/role | ✅ | ❌ | ❌ | ❌ |

### Existing Patterns to Follow

**From `src/middleware/auth.rs` (Story 14.1):**
- Use actix-web extensions for storing authenticated user
- Return ApiError with proper error codes
- Use FromRequest trait for extractors

**From `src/routes/api_v1.rs`:**
- Follow existing handler patterns
- Use web::Json<T> for request/response bodies
- Use HttpResponse for error responses

**From `src/db/sqlite.rs`:**
- Use sqlx for database queries
- Follow existing pattern of `sqlx::query_as!()`

### Testing Standards

**Unit Tests (inline in source files):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_all_permissions() {
        let admin = UserRole::Admin;
        assert!(admin.has_permission(Permission::AdminUsers));
        assert!(admin.has_permission(Permission::DeviceWrite));
    }

    #[test]
    fn test_viewer_read_only() {
        let viewer = UserRole::Viewer;
        assert!(viewer.has_permission(Permission::DeviceRead));
        assert!(!viewer.has_permission(Permission::DeviceWrite));
    }

    #[test]
    fn test_agent_own_profiles_only() {
        let agent = UserRole::Agent;
        assert!(agent.has_permission(Permission::ProfileWrite));
        // Scoping handled at query level, not permission level
    }
}
```

**Integration Tests (`tests/test_rbac.rs`):**
- Test full role assignment flow
- Test permission denial scenarios
- Test profile ownership scoping
- Use test database pattern from `tests/common/mod.rs`

### Dependencies

**Story 14.1 Must Be Complete:**
- User model with role field
- JWT authentication middleware
- AuthenticatedUser extractor
- Error code infrastructure

**New Dependencies (if not already in 14.1):**
None required — uses existing actix-web middleware patterns.

### References

- [Source: `_bmad-output/planning-artifacts/architecture.md#ADR-008`] - JWT + RBAC pattern
- [Source: `_bmad-output/planning-artifacts/architecture.md#Error-Codes`] - CC-AUTH-104
- [Source: `_bmad-output/planning-artifacts/epics-phase3.md#Story-14.2`] - Story definition
- [Source: `_bmad-output/planning-artifacts/prd-phase3-cloudcontrol-rust.md#FR3-FR4`] - Functional requirements
- [Source: `src/middleware/auth.rs`] - Auth middleware from Story 14.1
- [Source: `src/models/user.rs`] - User model from Story 14.1

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List

- `src/models/user.rs` — MODIFY: Add Permission enum, UserRole::permissions()
- `src/middleware/mod.rs` — MODIFY: Export rbac module
- `src/middleware/rbac.rs` — NEW: RBAC middleware and extractors
- `src/routes/mod.rs` — MODIFY: Include admin routes
- `src/routes/admin.rs` — NEW: Admin-only endpoints
- `src/main.rs` — MODIFY: Register admin routes
- `tests/test_rbac.rs` — NEW: RBAC integration tests
