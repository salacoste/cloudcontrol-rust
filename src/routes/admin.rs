//! Admin routes (Story 14-2, Story 14-3, Story 14-5)
//!
//! Handles admin-only operations like role assignment, team management, and audit logs.

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::str::FromStr;

use crate::middleware::RequireAdmin;
use crate::models::audit::{AuditQueryParams, CreateAuditEntry};
use crate::models::team::{
    AddMemberRequest, AssignDeviceRequest, CreateTeamRequest, UpdateTeamRequest,
};
use crate::models::user::{UserRole, User};
use crate::services::auth_service::AuthError;
use crate::services::team_service::{TeamError, TeamService};
use crate::state::AppState;

/// Request body for role assignment
#[derive(Debug, Clone, Deserialize)]
pub struct RoleAssignmentRequest {
    pub role: String,
}

/// Response for role assignment
#[derive(Debug, Clone, Serialize)]
pub struct RoleAssignmentResponse {
    pub id: String,
    pub email: String,
    pub role: String,
    pub team_id: Option<String>,
    pub created_at: String,
    pub last_login_at: Option<String>,
}

impl From<User> for RoleAssignmentResponse {
    fn from(user: User) -> Self {
        RoleAssignmentResponse {
            id: user.id,
            email: user.email,
            role: user.role,
            team_id: user.team_id,
            created_at: user.created_at,
            last_login_at: user.last_login_at,
        }
    }
}

/// Error response for admin operations
#[derive(Debug, Serialize)]
pub struct AdminError {
    pub status: String,
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// POST /api/v1/admin/users/{id}/role
///
/// Assign a new role to a user. Requires admin role.
pub async fn assign_role(
    _admin: RequireAdmin,
    path: web::Path<String>,
    body: web::Json<RoleAssignmentRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let target_user_id = path.into_inner();

    // Parse and validate the new role
    let new_role = match UserRole::from_str(&body.role) {
        Ok(role) => role,
        Err(_) => {
            return HttpResponse::BadRequest().json(AdminError {
                status: "error".to_string(),
                error: "CC-SYS-901".to_string(),
                message: "Validation error".to_string(),
                details: Some(format!(
                    "Invalid role: '{}'. Valid roles: admin, agent, viewer, renter",
                    body.role
                )),
            });
        }
    };

    // Get auth service
    let auth_service = match &state.auth_service {
        Some(service) => service,
        None => {
            return HttpResponse::InternalServerError().json(AdminError {
                status: "error".to_string(),
                error: "CC-AUTH-500".to_string(),
                message: "Authentication not configured".to_string(),
                details: None,
            });
        }
    };

    // Prevent self-modification
    if target_user_id == _admin.user.id {
        return HttpResponse::BadRequest().json(AdminError {
            status: "error".to_string(),
            error: "CC-SYS-901".to_string(),
            message: "Validation error".to_string(),
            details: Some("Cannot modify your own role".to_string()),
        });
    }

    // Get the old role before updating (for audit logging - Story 14-5)
    let old_role = match auth_service.get_user(&target_user_id).await {
        Ok(Some(user)) => user.role,
        Ok(None) => {
            return HttpResponse::NotFound().json(AdminError {
                status: "error".to_string(),
                error: "CC-AUTH-107".to_string(),
                message: "User not found".to_string(),
                details: None,
            });
        }
        Err(e) => {
            tracing::error!("Failed to get user for role check: {}", e);
            return HttpResponse::InternalServerError().json(AdminError {
                status: "error".to_string(),
                error: "CC-AUTH-500".to_string(),
                message: "Failed to get user".to_string(),
                details: None,
            });
        }
    };

    // Update the user's role
    match auth_service.update_user_role(&target_user_id, &new_role.to_string()).await {
        Ok(user) => {
            // Log role change (Story 14-5)
            if let Some(audit_service) = &state.audit_service {
                let entry = CreateAuditEntry::user_role_changed(
                    &_admin.user.id,
                    &_admin.user.email,
                    &target_user_id,
                    &old_role,
                    &new_role.to_string(),
                );
                let audit_service = audit_service.clone();
                actix_web::rt::spawn(async move {
                    if let Err(e) = audit_service.log_event(&entry).await {
                        tracing::warn!("Failed to log audit event: {}", e);
                    }
                });
            }

            tracing::info!(
                admin_user_id = % _admin.user.id,
                target_user_id = % target_user_id,
                new_role = % new_role,
                "Role updated by admin"
            );
            HttpResponse::Ok().json(RoleAssignmentResponse::from(user))
        }
        Err(AuthError::UserNotFound) => {
            HttpResponse::NotFound().json(AdminError {
                status: "error".to_string(),
                error: "CC-AUTH-107".to_string(),
                message: "User not found".to_string(),
                details: None,
            })
        }
        Err(e) => {
            tracing::error!("Failed to update user role: {}", e);
            HttpResponse::InternalServerError().json(AdminError {
                status: "error".to_string(),
                error: "CC-AUTH-500".to_string(),
                message: "Failed to update user role".to_string(),
                details: None,
            })
        }
    }
}

/// GET /api/v1/admin/users
///
/// List all users. Requires admin role.
pub async fn list_users(
    _admin: RequireAdmin,
    state: web::Data<AppState>,
) -> HttpResponse {
    let auth_service = match &state.auth_service {
        Some(service) => service,
        None => {
            return HttpResponse::InternalServerError().json(AdminError {
                status: "error".to_string(),
                error: "CC-AUTH-500".to_string(),
                message: "Authentication not configured".to_string(),
                details: None,
            });
        }
    };

    match auth_service.list_users().await {
        Ok(users) => {
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "data": users
            }))
        }
        Err(e) => {
            tracing::error!("Failed to list users: {}", e);
            HttpResponse::InternalServerError().json(AdminError {
                status: "error".to_string(),
                error: "CC-AUTH-500".to_string(),
                message: "Failed to list users".to_string(),
                details: None,
            })
        }
    }
}

/// Configure admin routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/api/v1/admin/users/{id}/role")
            .route(web::post().to(assign_role))
    )
    .service(
        web::resource("/api/v1/admin/users")
            .route(web::get().to(list_users))
    )
    // Team management routes (Story 14-3)
    .service(
        web::resource("/api/v1/admin/teams")
            .route(web::post().to(create_team))
            .route(web::get().to(list_teams))
    )
    .service(
        web::resource("/api/v1/admin/teams/{id}")
            .route(web::get().to(get_team))
            .route(web::put().to(update_team))
            .route(web::delete().to(delete_team))
    )
    .service(
        web::resource("/api/v1/admin/teams/{id}/members")
            .route(web::post().to(add_team_member))
    )
    .service(
        web::resource("/api/v1/admin/teams/{id}/members/{user_id}")
            .route(web::delete().to(remove_team_member))
    )
    // Device team assignment routes (Story 14-3)
    .service(
        web::resource("/api/v1/admin/devices/{udid}/team")
            .route(web::put().to(assign_device_team))
    )
    // Audit log routes (Story 14-5)
    .service(
        web::resource("/api/v1/admin/audit-log")
            .route(web::get().to(list_audit_log))
    );
}

// ============================================================================
// Team Management Routes (Story 14-3)
// ============================================================================

/// Helper to get team service from state
fn get_team_service(state: &web::Data<AppState>) -> Result<Arc<TeamService>, HttpResponse> {
    state.team_service.clone().ok_or_else(|| {
        HttpResponse::InternalServerError().json(AdminError {
            status: "error".to_string(),
            error: "CC-TEAM-500".to_string(),
            message: "Team service not configured".to_string(),
            details: None,
        })
    })
}

/// Map TeamError to HttpResponse
fn team_error_to_response(e: TeamError) -> HttpResponse {
    match e {
        TeamError::TeamNotFound => HttpResponse::NotFound().json(AdminError {
            status: "error".to_string(),
            error: "CC-TEAM-404".to_string(),
            message: e.to_string(),
            details: None,
        }),
        TeamError::UserNotFound => HttpResponse::NotFound().json(AdminError {
            status: "error".to_string(),
            error: "CC-TEAM-404".to_string(),
            message: e.to_string(),
            details: None,
        }),
        TeamError::DeviceNotFound => HttpResponse::NotFound().json(AdminError {
            status: "error".to_string(),
            error: "CC-TEAM-404".to_string(),
            message: e.to_string(),
            details: None,
        }),
        TeamError::UserAlreadyInTeam => HttpResponse::Conflict().json(AdminError {
            status: "error".to_string(),
            error: "CC-TEAM-409".to_string(),
            message: e.to_string(),
            details: None,
        }),
        TeamError::CannotDeleteTeamWithMembers | TeamError::CannotDeleteTeamWithDevices => {
            HttpResponse::BadRequest().json(AdminError {
                status: "error".to_string(),
                error: "CC-TEAM-400".to_string(),
                message: e.to_string(),
                details: None,
            })
        }
        TeamError::ValidationError(msg) => HttpResponse::BadRequest().json(AdminError {
            status: "error".to_string(),
            error: "CC-TEAM-901".to_string(),
            message: "Validation error".to_string(),
            details: Some(msg),
        }),
        TeamError::DatabaseError(msg) => {
            tracing::error!("Team database error: {}", msg);
            HttpResponse::InternalServerError().json(AdminError {
                status: "error".to_string(),
                error: "CC-TEAM-500".to_string(),
                message: "Database error".to_string(),
                details: None,
            })
        }
    }
}

/// POST /api/v1/admin/teams
///
/// Create a new team. Requires admin role.
pub async fn create_team(
    _admin: RequireAdmin,
    body: web::Json<CreateTeamRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let team_service = match get_team_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let actor_email: Option<&str> = Some(_admin.user.email.as_str());

    match team_service.create_team(&body.into_inner(), &_admin.user.id, actor_email).await {
        Ok(team) => {
            tracing::info!(
                admin_user_id = %_admin.user.id,
                team_id = %team.id,
                team_name = %team.name,
                "Team created by admin"
            );
            HttpResponse::Created().json(serde_json::json!({
                "status": "success",
                "data": team
            }))
        }
        Err(e) => team_error_to_response(e),
    }
}

/// GET /api/v1/admin/teams
///
/// List all teams. Requires admin role.
pub async fn list_teams(
    _admin: RequireAdmin,
    state: web::Data<AppState>,
) -> HttpResponse {
    let team_service = match get_team_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };

    match team_service.list_teams().await {
        Ok(teams) => {
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "data": teams
            }))
        }
        Err(e) => team_error_to_response(e),
    }
}

/// GET /api/v1/admin/teams/{id}
///
/// Get team details with members. Requires admin role.
pub async fn get_team(
    _admin: RequireAdmin,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let team_id = path.into_inner();
    let team_service = match get_team_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };

    match team_service.get_team_details(&team_id).await {
        Ok(details) => {
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "data": details
            }))
        }
        Err(e) => team_error_to_response(e),
    }
}

/// PUT /api/v1/admin/teams/{id}
///
/// Update a team. Requires admin role.
pub async fn update_team(
    _admin: RequireAdmin,
    path: web::Path<String>,
    body: web::Json<UpdateTeamRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let team_id = path.into_inner();
    let team_service = match get_team_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };

    let actor_email = Some(_admin.user.email.as_str());
    match team_service.update_team(&team_id, &body.into_inner(), &_admin.user.id, actor_email).await {
        Ok(team) => {
            tracing::info!(
                admin_user_id = %_admin.user.id,
                team_id = %team_id,
                "Team updated by admin"
            );
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "data": team
            }))
        }
        Err(e) => team_error_to_response(e),
    }
}

/// DELETE /api/v1/admin/teams/{id}
///
/// Delete a team. Requires admin role.
/// Team must have no members or devices.
pub async fn delete_team(
    _admin: RequireAdmin,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let team_id = path.into_inner();
    let team_service = match get_team_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };

    match team_service.delete_team(&team_id).await {
        Ok(()) => {
            tracing::info!(
                admin_user_id = %_admin.user.id,
                team_id = %team_id,
                "Team deleted by admin"
            );
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": "Team deleted"
            }))
        }
        Err(e) => team_error_to_response(e),
    }
}

/// POST /api/v1/admin/teams/{id}/members
///
/// Add a member to a team. Requires admin role.
pub async fn add_team_member(
    _admin: RequireAdmin,
    path: web::Path<String>,
    body: web::Json<AddMemberRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let team_id = path.into_inner();
    let user_id = body.user_id.clone();
    let team_service = match get_team_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };

    let actor_email = Some(_admin.user.email.as_str());
    match team_service.add_member(&team_id, &user_id, &_admin.user.id, actor_email).await {
        Ok(user) => {
            tracing::info!(
                admin_user_id = %_admin.user.id,
                team_id = %team_id,
                user_id = %user_id,
                "Member added to team by admin"
            );
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "data": RoleAssignmentResponse::from(user)
            }))
        }
        Err(e) => team_error_to_response(e),
    }
}

/// DELETE /api/v1/admin/teams/{id}/members/{user_id}
///
/// Remove a member from a team. Requires admin role.
pub async fn remove_team_member(
    _admin: RequireAdmin,
    path: web::Path<(String, String)>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let (team_id, user_id) = path.into_inner();
    let team_service = match get_team_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let actor_email = Some(_admin.user.email.as_str());

    match team_service.remove_member(&team_id, &user_id, &_admin.user.id, actor_email).await {
        Ok(()) => {
            tracing::info!(
                admin_user_id = %_admin.user.id,
                team_id = %team_id,
                user_id = %user_id,
                "Member removed from team by admin"
            );
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": "Member removed from team"
            }))
        }
        Err(e) => team_error_to_response(e),
    }
}

/// PUT /api/v1/admin/devices/{udid}/team
///
/// Assign or unassign a device to/from a team. Requires admin role.
pub async fn assign_device_team(
    _admin: RequireAdmin,
    path: web::Path<String>,
    body: web::Json<AssignDeviceRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let udid = path.into_inner();
    let team_service = match get_team_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };

    let actor_email = Some(_admin.user.email.as_str());
    let result = match &body.team_id {
        Some(team_id) => team_service.assign_device(&udid, team_id, &_admin.user.id, actor_email).await.map(|_| {
            tracing::info!(
                admin_user_id = %_admin.user.id,
                udid = %udid,
                team_id = %team_id,
                "Device assigned to team by admin"
            );
            serde_json::json!({
                "udid": udid,
                "team_id": team_id,
                "message": "Device assigned to team"
            })
        }),
        None => team_service.remove_device(&udid, &_admin.user.id, actor_email).await.map(|_| {
            tracing::info!(
                admin_user_id = %_admin.user.id,
                udid = %udid,
                "Device removed from team by admin"
            );
            serde_json::json!({
                "udid": udid,
                "team_id": serde_json::Value::Null,
                "message": "Device removed from team"
            })
        }),
    };

    match result {
        Ok(response) => HttpResponse::Ok().json(serde_json::json!({
            "status": "success",
            "data": response
        })),
        Err(e) => team_error_to_response(e),
    }
}

// ============================================================================
// Audit Log Routes (Story 14-5)
// ============================================================================

/// GET /api/v1/admin/audit-log
///
/// List audit log entries with optional filtering.
/// Requires admin role.
pub async fn list_audit_log(
    _admin: RequireAdmin,
    query: web::Query<AuditQueryParams>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let audit_service = match &state.audit_service {
        Some(service) => service,
        None => {
            return HttpResponse::InternalServerError().json(AdminError {
                status: "error".to_string(),
                error: "CC-AUDIT-500".to_string(),
                message: "Audit service not configured".to_string(),
                details: None,
            });
        }
    };

    match audit_service.list_entries(&query.into_inner()).await {
        Ok(response) => {
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "data": response
            }))
        }
        Err(e) => {
            tracing::error!("Failed to list audit entries: {}", e);
            HttpResponse::InternalServerError().json(AdminError {
                status: "error".to_string(),
                error: "CC-AUDIT-500".to_string(),
                message: "Failed to retrieve audit log".to_string(),
                details: Some(e.to_string()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::AssignDeviceRequest;

    #[test]
    fn test_role_assignment_request_deserialization() {
        let json = r#"{"role":"viewer"}"#;
        let req: RoleAssignmentRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.role, "viewer");
    }

    #[test]
    fn test_role_assignment_response_serialization() {
        let resp = RoleAssignmentResponse {
            id: "user_123".to_string(),
            email: "test@example.com".to_string(),
            role: "viewer".to_string(),
            team_id: None,
            created_at: "2026-03-13T10:00:00Z".to_string(),
            last_login_at: Some("2026-03-13T14:30:00Z".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("user_123"));
        assert!(json.contains("viewer"));
    }

    #[test]
    fn test_admin_error_serialization() {
        let err = AdminError {
            status: "error".to_string(),
            error: "CC-AUTH-104".to_string(),
            message: "Insufficient permissions".to_string(),
            details: Some("Admin role required".to_string()),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("CC-AUTH-104"));
    }

    // Team endpoint tests
    #[test]
    fn test_create_team_request_deserialization() {
        let json = r#"{"name":"Engineering","description":"Engineering team"}"#;
        let req: CreateTeamRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Engineering");
        assert_eq!(req.description, Some("Engineering team".to_string()));
    }

    #[test]
    fn test_create_team_request_minimal() {
        let json = r#"{"name":"Test Team"}"#;
        let req: CreateTeamRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Test Team");
        assert_eq!(req.description, None);
    }

    #[test]
    fn test_update_team_request_deserialization() {
        let json = r#"{"name":"Updated Name","description":"Updated desc"}"#;
        let req: UpdateTeamRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("Updated Name".to_string()));
        assert_eq!(req.description, Some("Updated desc".to_string()));
    }

    #[test]
    fn test_update_team_request_partial() {
        let json = r#"{"name":"New Name"}"#;
        let req: UpdateTeamRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("New Name".to_string()));
        // description should use default (None)
        assert!(req.description.is_none() || req.description == Some(String::new()));
    }

    #[test]
    fn test_add_member_request_deserialization() {
        let json = r#"{"user_id":"user_abc123"}"#;
        let req: AddMemberRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.user_id, "user_abc123");
    }

    #[test]
    fn test_assign_device_request_deserialization() {
        let json = r#"{"team_id":"team_xyz789"}"#;
        let req: AssignDeviceRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.team_id, Some("team_xyz789".to_string()));
    }

    #[test]
    fn test_assign_device_request_unassign() {
        let json = r#"{"team_id":null}"#;
        let req: AssignDeviceRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.team_id, None);
    }
}
