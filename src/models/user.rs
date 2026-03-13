//! User model and authentication DTOs (Story 14-1)
//!
//! Implements the user data model with role-based access control (RBAC)
//! and JWT authentication support.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use validator::Validate;

/// User role for RBAC (Role-Based Access Control)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,   // Full access to all resources and admin endpoints
    Agent,   // Device operations, own profiles (default for new users)
    Viewer,  // Read-only access
    Renter,  // Team-scoped device access (for Growth phase)
}

/// System permissions for RBAC (Story 14-2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Returns all permissions granted to this role
    pub fn permissions(&self) -> Vec<Permission> {
        match self {
            UserRole::Admin => vec![
                Permission::DeviceRead,
                Permission::DeviceWrite,
                Permission::ProfileRead,
                Permission::ProfileWrite,
                Permission::ProfileCheckout,
                Permission::AdminUsers,
                Permission::AdminAudit,
                Permission::AdminTeams,
            ],
            UserRole::Agent => vec![
                Permission::DeviceRead,
                Permission::DeviceWrite,
                Permission::ProfileRead,
                Permission::ProfileWrite,
                Permission::ProfileCheckout,
            ],
            UserRole::Viewer => vec![
                Permission::DeviceRead,
                Permission::ProfileRead,
            ],
            UserRole::Renter => vec![
                Permission::DeviceRead,
                Permission::DeviceWrite,
                Permission::ProfileRead,
                Permission::ProfileWrite,
                Permission::ProfileCheckout,
            ],
        }
    }

    /// Check if this role has a specific permission
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions().contains(&permission)
    }
}

impl Default for UserRole {
    fn default() -> Self {
        Self::Agent
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::Agent => write!(f, "agent"),
            UserRole::Viewer => write!(f, "viewer"),
            UserRole::Renter => write!(f, "renter"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(UserRole::Admin),
            "agent" => Ok(UserRole::Agent),
            "viewer" => Ok(UserRole::Viewer),
            "renter" => Ok(UserRole::Renter),
            _ => Err(format!("Invalid role: '{}'. Valid roles: admin, agent, viewer, renter", s)),
        }
    }
}

/// User record from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub team_id: Option<String>,
    pub created_at: String,
    pub last_login_at: Option<String>,
}

/// New user registration request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}

/// User login request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    pub password: String,
}

/// Token refresh request
#[derive(Debug, Clone, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Logout request
#[derive(Debug, Clone, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

/// Successful login response
#[derive(Debug, Clone, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub expires_in: u64,
    pub user: UserInfo,
    /// Session ID for audit logging (not serialized to client - Story 14-5)
    #[serde(skip)]
    pub session_id: String,
}

/// User info returned in auth responses
#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub role: String,
    /// Team ID for team-scoped access control (Story 14-3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
}

/// User registration response
#[derive(Debug, Clone, Serialize)]
pub struct RegisterResponse {
    pub id: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
}

/// Token refresh response
#[derive(Debug, Clone, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub expires_in: u64,
}

/// JWT claims for access tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,      // User ID
    pub email: String,
    pub role: String,
    /// Team ID for team-scoped access control (Story 14-3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    pub exp: i64,         // Expiration timestamp
    pub iat: i64,         // Issued at timestamp
}

/// Refresh token record from database
#[derive(Debug, Clone, FromRow)]
pub struct RefreshToken {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: String,
    pub revoked: i32,
    pub created_at: String,
}

/// New user to insert into database
#[derive(Debug, Clone)]
pub struct NewUser {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub role: UserRole,
}

/// Password validation result
#[derive(Debug, Clone)]
pub struct PasswordValidation {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

/// Validate password strength
/// Requirements: min 8 chars, 1 uppercase, 1 lowercase, 1 number
pub fn validate_password(password: &str) -> PasswordValidation {
    let mut errors = Vec::new();

    if password.len() < 8 {
        errors.push("Password must be at least 8 characters".to_string());
    }

    if !password.chars().any(|c| c.is_uppercase()) {
        errors.push("Password must contain at least one uppercase letter".to_string());
    }

    if !password.chars().any(|c| c.is_lowercase()) {
        errors.push("Password must contain at least one lowercase letter".to_string());
    }

    if !password.chars().any(|c| c.is_numeric()) {
        errors.push("Password must contain at least one number".to_string());
    }

    PasswordValidation {
        is_valid: errors.is_empty(),
        errors,
    }
}

/// Generate a unique user ID with prefix
pub fn generate_user_id() -> String {
    format!("user_{}", uuid::Uuid::new_v4().simple())
}

/// Generate a unique refresh token ID with prefix
pub fn generate_refresh_token_id() -> String {
    format!("rt_{}", uuid::Uuid::new_v4().simple())
}

/// Generate a cryptographically secure refresh token
pub fn generate_refresh_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let token: [u8; 32] = rng.gen();
    format!("rt_{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_user_role_display() {
        assert_eq!(UserRole::Admin.to_string(), "admin");
        assert_eq!(UserRole::Agent.to_string(), "agent");
        assert_eq!(UserRole::Viewer.to_string(), "viewer");
        assert_eq!(UserRole::Renter.to_string(), "renter");
    }

    #[test]
    fn test_user_role_from_str() {
        assert!(matches!(UserRole::from_str("admin"), Ok(UserRole::Admin)));
        assert!(matches!(UserRole::from_str("ADMIN"), Ok(UserRole::Admin)));
        assert!(matches!(UserRole::from_str("agent"), Ok(UserRole::Agent)));
        assert!(matches!(UserRole::from_str("viewer"), Ok(UserRole::Viewer)));
        assert!(matches!(UserRole::from_str("renter"), Ok(UserRole::Renter)));
        assert!(UserRole::from_str("superuser").is_err());
    }

    #[test]
    fn test_password_validation_valid() {
        let result = validate_password("SecureP@ss123");
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_password_validation_too_short() {
        let result = validate_password("Short1");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("8 characters")));
    }

    #[test]
    fn test_password_validation_no_uppercase() {
        let result = validate_password("lowercase123");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("uppercase")));
    }

    #[test]
    fn test_password_validation_no_lowercase() {
        let result = validate_password("UPPERCASE123");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("lowercase")));
    }

    #[test]
    fn test_password_validation_no_number() {
        let result = validate_password("NoNumbers!");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("number")));
    }

    #[test]
    fn test_generate_user_id() {
        let id = generate_user_id();
        assert!(id.starts_with("user_"));
        assert_eq!(id.len(), 4 + 32 + 1); // "user_" + uuid
    }

    #[test]
    fn test_generate_refresh_token() {
        let token = generate_refresh_token();
        assert!(token.starts_with("rt_"));
        // Base64 of 32 bytes is 43 characters
        assert!(token.len() > 10);
    }

    #[test]
    fn test_register_request_validation() {
        let valid = RegisterRequest {
            email: "test@example.com".to_string(),
            password: "SecureP@ss123".to_string(),
        };
        assert!(valid.validate().is_ok());

        let invalid_email = RegisterRequest {
            email: "not-an-email".to_string(),
            password: "SecureP@ss123".to_string(),
        };
        assert!(invalid_email.validate().is_err());

        let short_password = RegisterRequest {
            email: "test@example.com".to_string(),
            password: "short".to_string(),
        };
        assert!(short_password.validate().is_err());
    }

    // ========================================
    // RBAC Permission Tests (Story 14-2)
    // ========================================

    #[test]
    fn test_admin_has_all_permissions() {
        let admin = UserRole::Admin;
        assert!(admin.has_permission(Permission::AdminUsers));
        assert!(admin.has_permission(Permission::AdminAudit));
        assert!(admin.has_permission(Permission::AdminTeams));
        assert!(admin.has_permission(Permission::DeviceRead));
        assert!(admin.has_permission(Permission::DeviceWrite));
        assert!(admin.has_permission(Permission::ProfileRead));
        assert!(admin.has_permission(Permission::ProfileWrite));
        assert!(admin.has_permission(Permission::ProfileCheckout));
        assert_eq!(admin.permissions().len(), 8);
    }

    #[test]
    fn test_agent_has_device_and_profile_permissions() {
        let agent = UserRole::Agent;
        assert!(agent.has_permission(Permission::DeviceRead));
        assert!(agent.has_permission(Permission::DeviceWrite));
        assert!(agent.has_permission(Permission::ProfileRead));
        assert!(agent.has_permission(Permission::ProfileWrite));
        assert!(agent.has_permission(Permission::ProfileCheckout));
        assert!(!agent.has_permission(Permission::AdminUsers));
        assert!(!agent.has_permission(Permission::AdminAudit));
        assert!(!agent.has_permission(Permission::AdminTeams));
        assert_eq!(agent.permissions().len(), 5);
    }

    #[test]
    fn test_viewer_read_only() {
        let viewer = UserRole::Viewer;
        assert!(viewer.has_permission(Permission::DeviceRead));
        assert!(viewer.has_permission(Permission::ProfileRead));
        assert!(!viewer.has_permission(Permission::DeviceWrite));
        assert!(!viewer.has_permission(Permission::ProfileWrite));
        assert!(!viewer.has_permission(Permission::ProfileCheckout));
        assert!(!viewer.has_permission(Permission::AdminUsers));
        assert_eq!(viewer.permissions().len(), 2);
    }

    #[test]
    fn test_renter_has_write_permissions() {
        let renter = UserRole::Renter;
        assert!(renter.has_permission(Permission::DeviceRead));
        assert!(renter.has_permission(Permission::DeviceWrite));
        assert!(renter.has_permission(Permission::ProfileRead));
        assert!(renter.has_permission(Permission::ProfileWrite));
        assert!(renter.has_permission(Permission::ProfileCheckout));
        assert!(!renter.has_permission(Permission::AdminUsers));
        assert!(!renter.has_permission(Permission::AdminAudit));
        assert!(!renter.has_permission(Permission::AdminTeams));
        assert_eq!(renter.permissions().len(), 5);
    }

    #[test]
    fn test_permission_equality() {
        assert_eq!(Permission::DeviceRead, Permission::DeviceRead);
        assert_ne!(Permission::DeviceRead, Permission::DeviceWrite);
    }
}
