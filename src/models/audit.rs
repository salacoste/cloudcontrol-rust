//! Audit log model and DTOs (Story 14-5)
//!
//! Provides types for activity audit logging and querying.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Audit action types with standardized naming convention
/// Note: Actions are stored as strings in the database for flexibility
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum AuditAction {
    // User authentication events
    #[serde(rename = "user.login")]
    UserLogin,
    #[serde(rename = "user.logout")]
    UserLogout,
    #[serde(rename = "user.login_failed")]
    UserLoginFailed,
    #[serde(rename = "user.role_changed")]
    UserRoleChanged,

    // Session events
    #[serde(rename = "session.revoked")]
    SessionRevoked,
    #[serde(rename = "session.refresh")]
    SessionRefresh,

    // Team events (already implemented in team_service)
    #[serde(rename = "team.created")]
    TeamCreated,
    #[serde(rename = "team.updated")]
    TeamUpdated,
    #[serde(rename = "team.deleted")]
    TeamDeleted,
    #[serde(rename = "team.member_added")]
    TeamMemberAdded,
    #[serde(rename = "team.member_removed")]
    TeamMemberRemoved,
    #[serde(rename = "team.device_assigned")]
    TeamDeviceAssigned,
    #[serde(rename = "team.device_removed")]
    TeamDeviceRemoved,

    // Device events
    #[serde(rename = "device.connected")]
    DeviceConnected,
    #[serde(rename = "device.disconnected")]
    DeviceDisconnected,

    // Profile events (future - Story 15)
    #[serde(rename = "profile.checkout")]
    ProfileCheckout,
    #[serde(rename = "profile.checkin")]
    ProfileCheckin,
    #[serde(rename = "profile.created")]
    ProfileCreated,
    #[serde(rename = "profile.deleted")]
    ProfileDeleted,

    // Custom action for extensibility - must be last for unagged serde
    Other(String),
}

impl From<String> for AuditAction {
    fn from(s: String) -> Self {
        match s.as_str() {
            "user.login" => AuditAction::UserLogin,
            "user.logout" => AuditAction::UserLogout,
            "user.login_failed" => AuditAction::UserLoginFailed,
            "user.role_changed" => AuditAction::UserRoleChanged,
            "session.revoked" => AuditAction::SessionRevoked,
            "session.refresh" => AuditAction::SessionRefresh,
            "team.created" => AuditAction::TeamCreated,
            "team.updated" => AuditAction::TeamUpdated,
            "team.deleted" => AuditAction::TeamDeleted,
            "team.member_added" => AuditAction::TeamMemberAdded,
            "team.member_removed" => AuditAction::TeamMemberRemoved,
            "team.device_assigned" => AuditAction::TeamDeviceAssigned,
            "team.device_removed" => AuditAction::TeamDeviceRemoved,
            "device.connected" => AuditAction::DeviceConnected,
            "device.disconnected" => AuditAction::DeviceDisconnected,
            "profile.checkout" => AuditAction::ProfileCheckout,
            "profile.checkin" => AuditAction::ProfileCheckin,
            "profile.created" => AuditAction::ProfileCreated,
            "profile.deleted" => AuditAction::ProfileDeleted,
            _ => AuditAction::Other(s),
        }
    }
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditAction::UserLogin => write!(f, "user.login"),
            AuditAction::UserLogout => write!(f, "user.logout"),
            AuditAction::UserLoginFailed => write!(f, "user.login_failed"),
            AuditAction::UserRoleChanged => write!(f, "user.role_changed"),
            AuditAction::SessionRevoked => write!(f, "session.revoked"),
            AuditAction::SessionRefresh => write!(f, "session.refresh"),
            AuditAction::TeamCreated => write!(f, "team.created"),
            AuditAction::TeamUpdated => write!(f, "team.updated"),
            AuditAction::TeamDeleted => write!(f, "team.deleted"),
            AuditAction::TeamMemberAdded => write!(f, "team.member_added"),
            AuditAction::TeamMemberRemoved => write!(f, "team.member_removed"),
            AuditAction::TeamDeviceAssigned => write!(f, "team.device_assigned"),
            AuditAction::TeamDeviceRemoved => write!(f, "team.device_removed"),
            AuditAction::DeviceConnected => write!(f, "device.connected"),
            AuditAction::DeviceDisconnected => write!(f, "device.disconnected"),
            AuditAction::ProfileCheckout => write!(f, "profile.checkout"),
            AuditAction::ProfileCheckin => write!(f, "profile.checkin"),
            AuditAction::ProfileCreated => write!(f, "profile.created"),
            AuditAction::ProfileDeleted => write!(f, "profile.deleted"),
            AuditAction::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Target types for audit log entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuditTargetType {
    User,
    Team,
    Device,
    Profile,
    Session,
}

impl std::fmt::Display for AuditTargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditTargetType::User => write!(f, "user"),
            AuditTargetType::Team => write!(f, "team"),
            AuditTargetType::Device => write!(f, "device"),
            AuditTargetType::Profile => write!(f, "profile"),
            AuditTargetType::Session => write!(f, "session"),
        }
    }
}

impl From<String> for AuditTargetType {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "user" => AuditTargetType::User,
            "team" => AuditTargetType::Team,
            "device" => AuditTargetType::Device,
            "profile" => AuditTargetType::Profile,
            "session" => AuditTargetType::Session,
            _ => AuditTargetType::User, // Default fallback
        }
    }
}

/// Audit log entry from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub action: String,
    pub actor_id: String,
    pub actor_email: Option<String>,
    pub target_type: String,
    pub target_id: String,
    pub team_id: Option<String>,
    pub details: Option<String>, // JSON string
    pub created_at: String,
}

/// Audit entry for API responses (parsed details as JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntryResponse {
    pub id: i64,
    pub action: String,
    pub actor_id: String,
    pub actor_email: Option<String>,
    pub target_type: String,
    pub target_id: String,
    pub team_id: Option<String>,
    pub details: Option<serde_json::Value>,
    pub created_at: String,
}

impl From<AuditEntry> for AuditEntryResponse {
    fn from(entry: AuditEntry) -> Self {
        let details = entry.details.and_then(|d| {
            serde_json::from_str(&d).ok()
        });

        AuditEntryResponse {
            id: entry.id,
            action: entry.action,
            actor_id: entry.actor_id,
            actor_email: entry.actor_email,
            target_type: entry.target_type,
            target_id: entry.target_id,
            team_id: entry.team_id,
            details,
            created_at: entry.created_at,
        }
    }
}

/// Pagination metadata for audit log responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

/// Response for audit log list endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditListResponse {
    pub entries: Vec<AuditEntryResponse>,
    pub pagination: PaginationInfo,
}

/// Query parameters for filtering audit logs
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuditQueryParams {
    /// Filter by actor (user) ID
    #[serde(default)]
    pub user_id: Option<String>,

    /// Filter by action type (e.g., "user.login")
    #[serde(default)]
    pub action: Option<String>,

    /// Filter by target type (user, team, device, session, profile)
    #[serde(default)]
    pub target_type: Option<String>,

    /// Filter by start date (ISO 8601 date string, inclusive)
    #[serde(default)]
    pub start_date: Option<String>,

    /// Filter by end date (ISO 8601 date string, inclusive)
    #[serde(default)]
    pub end_date: Option<String>,

    /// Page number (1-indexed, default: 1)
    #[serde(default = "default_page")]
    pub page: i64,

    /// Items per page (default: 20, max: 100)
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    20
}

impl Default for AuditQueryParams {
    fn default() -> Self {
        AuditQueryParams {
            user_id: None,
            action: None,
            target_type: None,
            start_date: None,
            end_date: None,
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

impl AuditQueryParams {
    /// Validate and normalize query parameters
    pub fn normalize(&self) -> Self {
        let per_page = self.per_page.clamp(1, 100);
        let page = self.page.max(1);

        AuditQueryParams {
            user_id: self.user_id.clone(),
            action: self.action.clone(),
            target_type: self.target_type.clone(),
            start_date: self.start_date.clone(),
            end_date: self.end_date.clone(),
            page,
            per_page,
        }
    }

    /// Calculate offset for pagination
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.per_page
    }
}

/// Request body for creating an audit log entry (internal use)
#[derive(Debug, Clone)]
pub struct CreateAuditEntry {
    pub action: String,
    pub actor_id: String,
    pub actor_email: Option<String>,
    pub target_type: String,
    pub target_id: String,
    pub team_id: Option<String>,
    pub details: Option<String>,
}

impl CreateAuditEntry {
    /// Create a new audit entry for user login
    pub fn user_login(
        user_id: &str,
        user_email: &str,
        session_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Self {
        let details = serde_json::to_string(&serde_json::json!({
            "ip_address": ip_address,
            "user_agent": user_agent
        })).ok();

        CreateAuditEntry {
            action: "user.login".to_string(),
            actor_id: user_id.to_string(),
            actor_email: Some(user_email.to_string()),
            target_type: "session".to_string(),
            target_id: session_id.to_string(),
            team_id: None,
            details,
        }
    }

    /// Create a new audit entry for failed login attempt
    pub fn user_login_failed(
        email: &str,
        reason: &str,
        ip_address: Option<&str>,
    ) -> Self {
        let details = serde_json::to_string(&serde_json::json!({
            "reason": reason,
            "ip_address": ip_address
        })).ok();

        CreateAuditEntry {
            action: "user.login_failed".to_string(),
            actor_id: "anonymous".to_string(),
            actor_email: Some(email.to_string()),
            target_type: "session".to_string(),
            target_id: "n/a".to_string(),
            team_id: None,
            details,
        }
    }

    /// Create a new audit entry for user logout
    pub fn user_logout(user_id: &str, session_id: &str) -> Self {
        CreateAuditEntry {
            action: "user.logout".to_string(),
            actor_id: user_id.to_string(),
            actor_email: None,
            target_type: "session".to_string(),
            target_id: session_id.to_string(),
            team_id: None,
            details: None,
        }
    }

    /// Create a new audit entry for session revocation
    pub fn session_revoked(user_id: &str, session_id: &str) -> Self {
        CreateAuditEntry {
            action: "session.revoked".to_string(),
            actor_id: user_id.to_string(),
            actor_email: None,
            target_type: "session".to_string(),
            target_id: session_id.to_string(),
            team_id: None,
            details: None,
        }
    }

    /// Create a new audit entry for role change
    pub fn user_role_changed(
        admin_id: &str,
        admin_email: &str,
        target_user_id: &str,
        old_role: &str,
        new_role: &str,
    ) -> Self {
        let details = serde_json::to_string(&serde_json::json!({
            "old_role": old_role,
            "new_role": new_role
        })).ok();

        CreateAuditEntry {
            action: "user.role_changed".to_string(),
            actor_id: admin_id.to_string(),
            actor_email: Some(admin_email.to_string()),
            target_type: "user".to_string(),
            target_id: target_user_id.to_string(),
            team_id: None,
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_action_display() {
        assert_eq!(AuditAction::UserLogin.to_string(), "user.login");
        assert_eq!(AuditAction::UserLogout.to_string(), "user.logout");
        assert_eq!(AuditAction::UserLoginFailed.to_string(), "user.login_failed");
        assert_eq!(AuditAction::UserRoleChanged.to_string(), "user.role_changed");
        assert_eq!(AuditAction::SessionRevoked.to_string(), "session.revoked");
    }

    #[test]
    fn test_audit_action_from_string() {
        assert_eq!(AuditAction::from("user.login".to_string()), AuditAction::UserLogin);
        assert_eq!(AuditAction::from("team.member_added".to_string()), AuditAction::TeamMemberAdded);
        assert_eq!(AuditAction::from("unknown.action".to_string()), AuditAction::Other("unknown.action".to_string()));
    }

    #[test]
    fn test_audit_target_type_display() {
        assert_eq!(AuditTargetType::User.to_string(), "user");
        assert_eq!(AuditTargetType::Team.to_string(), "team");
        assert_eq!(AuditTargetType::Device.to_string(), "device");
        assert_eq!(AuditTargetType::Session.to_string(), "session");
        assert_eq!(AuditTargetType::Profile.to_string(), "profile");
    }

    #[test]
    fn test_audit_query_params_defaults() {
        let params = AuditQueryParams {
            user_id: None,
            action: None,
            target_type: None,
            start_date: None,
            end_date: None,
            page: 0, // Will be normalized to 1
            per_page: 0, // Will be normalized to 1 (min)
        };

        let normalized = params.normalize();
        assert_eq!(normalized.page, 1);
        assert_eq!(normalized.per_page, 1);
    }

    #[test]
    fn test_audit_query_params_normalize_per_page_cap() {
        let params = AuditQueryParams {
            user_id: None,
            action: None,
            target_type: None,
            start_date: None,
            end_date: None,
            page: 1,
            per_page: 500, // Should be capped to 100
        };

        let normalized = params.normalize();
        assert_eq!(normalized.per_page, 100);
    }

    #[test]
    fn test_audit_query_params_offset() {
        let params = AuditQueryParams {
            user_id: None,
            action: None,
            target_type: None,
            start_date: None,
            end_date: None,
            page: 3,
            per_page: 20,
        };

        let normalized = params.normalize();
        assert_eq!(normalized.offset(), 40); // (3-1) * 20
    }

    #[test]
    fn test_audit_entry_response_conversion() {
        let entry = AuditEntry {
            id: 123,
            action: "user.login".to_string(),
            actor_id: "user_abc".to_string(),
            actor_email: Some("test@example.com".to_string()),
            target_type: "session".to_string(),
            target_id: "rt_xyz".to_string(),
            team_id: None,
            details: Some(r#"{"ip":"192.168.1.1"}"#.to_string()),
            created_at: "2026-03-13T10:00:00Z".to_string(),
        };

        let response: AuditEntryResponse = entry.into();
        assert_eq!(response.id, 123);
        assert_eq!(response.action, "user.login");
        assert!(response.details.is_some());
        let details = response.details.unwrap();
        assert_eq!(details["ip"], "192.168.1.1");
    }

    #[test]
    fn test_create_audit_entry_user_login() {
        let entry = CreateAuditEntry::user_login(
            "user_123",
            "test@example.com",
            "rt_abc",
            Some("192.168.1.1"),
            Some("Mozilla/5.0"),
        );

        assert_eq!(entry.action, "user.login");
        assert_eq!(entry.actor_id, "user_123");
        assert_eq!(entry.actor_email, Some("test@example.com".to_string()));
        assert_eq!(entry.target_type, "session");
        assert_eq!(entry.target_id, "rt_abc");
        assert!(entry.details.is_some());
    }

    #[test]
    fn test_create_audit_entry_user_login_failed() {
        let entry = CreateAuditEntry::user_login_failed(
            "test@example.com",
            "invalid_credentials",
            Some("192.168.1.1"),
        );

        assert_eq!(entry.action, "user.login_failed");
        assert_eq!(entry.actor_id, "anonymous");
        assert_eq!(entry.target_type, "session");
        assert_eq!(entry.target_id, "n/a");
        assert!(entry.details.is_some());

        let details: serde_json::Value = serde_json::from_str(&entry.details.unwrap()).unwrap();
        assert_eq!(details["reason"], "invalid_credentials");
        assert_eq!(details["ip_address"], "192.168.1.1");
    }

    #[test]
    fn test_create_audit_entry_user_role_changed() {
        let entry = CreateAuditEntry::user_role_changed(
            "admin_123",
            "admin@example.com",
            "user_456",
            "viewer",
            "agent",
        );

        assert_eq!(entry.action, "user.role_changed");
        assert_eq!(entry.actor_id, "admin_123");
        assert_eq!(entry.target_type, "user");
        assert_eq!(entry.target_id, "user_456");

        let details: serde_json::Value = serde_json::from_str(&entry.details.unwrap()).unwrap();
        assert_eq!(details["old_role"], "viewer");
        assert_eq!(details["new_role"], "agent");
    }

    #[test]
    fn test_pagination_info() {
        let pagination = PaginationInfo {
            total: 150,
            page: 2,
            per_page: 20,
            total_pages: 8,
        };

        let json = serde_json::to_string(&pagination).unwrap();
        assert!(json.contains("\"total\":150"));
        assert!(json.contains("\"page\":2"));
        assert!(json.contains("\"per_page\":20"));
        assert!(json.contains("\"total_pages\":8"));
    }

    #[test]
    fn test_audit_list_response() {
        let response = AuditListResponse {
            entries: vec![
                AuditEntryResponse {
                    id: 1,
                    action: "user.login".to_string(),
                    actor_id: "user_1".to_string(),
                    actor_email: Some("user1@example.com".to_string()),
                    target_type: "session".to_string(),
                    target_id: "rt_1".to_string(),
                    team_id: None,
                    details: None,
                    created_at: "2026-03-13T10:00:00Z".to_string(),
                },
            ],
            pagination: PaginationInfo {
                total: 1,
                page: 1,
                per_page: 20,
                total_pages: 1,
            },
        };

        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.pagination.total, 1);
    }
}
