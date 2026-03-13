//! Session model and DTOs (Story 14-4)
//!
//! Implements session management DTOs for viewing and revoking active sessions.
//! Sessions are based on refresh tokens with optional metadata (user_agent, ip_address).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Session record from database (extended refresh_tokens table)
#[derive(Debug, Clone, FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    #[allow(dead_code)]
    pub token_hash: String, // Never expose in API responses
    pub expires_at: String,
    pub revoked: i32,
    pub created_at: String,
    /// When the token was last refreshed (optional)
    pub last_used_at: Option<String>,
    /// Client user agent from login request (optional)
    pub user_agent: Option<String>,
    /// Originating IP address from login request (optional)
    pub ip_address: Option<String>,
}

/// Session info returned in API responses (excludes sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session identifier (same as refresh token ID)
    pub id: String,
    /// When the session was created
    pub created_at: String,
    /// When the session expires
    pub expires_at: String,
    /// When the token was last refreshed (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    /// Client user agent from login request (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Originating IP address from login request (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
}

impl From<Session> for SessionInfo {
    fn from(session: Session) -> Self {
        SessionInfo {
            id: session.id,
            created_at: session.created_at,
            expires_at: session.expires_at,
            last_used_at: session.last_used_at,
            user_agent: session.user_agent,
            ip_address: session.ip_address,
        }
    }
}

/// Response for listing sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListResponse {
    /// List of active sessions
    pub sessions: Vec<SessionInfo>,
    /// Total number of sessions
    pub total: usize,
}

/// Response for revoking a single session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRevokeResponse {
    /// Success message
    pub message: String,
    /// ID of the revoked session
    pub session_id: String,
}

/// Response for revoking all other sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeAllSessionsResponse {
    /// Success message
    pub message: String,
    /// Number of sessions revoked
    pub revoked_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_to_session_info() {
        let session = Session {
            id: "sess_abc123".to_string(),
            user_id: "user_xyz".to_string(),
            token_hash: "hash_secret".to_string(),
            expires_at: "2026-03-20T10:30:00Z".to_string(),
            revoked: 0,
            created_at: "2026-03-13T10:30:00Z".to_string(),
            last_used_at: Some("2026-03-14T15:45:00Z".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
            ip_address: Some("192.168.1.100".to_string()),
        };

        let info: SessionInfo = session.into();

        assert_eq!(info.id, "sess_abc123");
        assert_eq!(info.created_at, "2026-03-13T10:30:00Z");
        assert_eq!(info.expires_at, "2026-03-20T10:30:00Z");
        assert_eq!(info.last_used_at, Some("2026-03-14T15:45:00Z".to_string()));
        assert_eq!(info.user_agent, Some("Mozilla/5.0".to_string()));
        assert_eq!(info.ip_address, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_session_info_serialization() {
        let info = SessionInfo {
            id: "sess_abc123".to_string(),
            created_at: "2026-03-13T10:30:00Z".to_string(),
            expires_at: "2026-03-20T10:30:00Z".to_string(),
            last_used_at: None,
            user_agent: None,
            ip_address: None,
        };

        let json = serde_json::to_string(&info).unwrap();

        // Should not include null fields
        assert!(json.contains("\"id\":\"sess_abc123\""));
        assert!(!json.contains("last_used_at"));
        assert!(!json.contains("user_agent"));
        assert!(!json.contains("ip_address"));
    }

    #[test]
    fn test_session_list_response() {
        let response = SessionListResponse {
            sessions: vec![
                SessionInfo {
                    id: "sess_1".to_string(),
                    created_at: "2026-03-13T10:00:00Z".to_string(),
                    expires_at: "2026-03-20T10:00:00Z".to_string(),
                    last_used_at: None,
                    user_agent: None,
                    ip_address: None,
                },
            ],
            total: 1,
        };

        assert_eq!(response.sessions.len(), 1);
        assert_eq!(response.total, 1);
    }

    #[test]
    fn test_revoke_all_sessions_response() {
        let response = RevokeAllSessionsResponse {
            message: "All other sessions revoked".to_string(),
            revoked_count: 3,
        };

        assert_eq!(response.revoked_count, 3);
    }
}
