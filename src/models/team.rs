//! Team model and DTOs (Story 14-3)
//!
//! Handles team/organization scoping for multi-tenant access control.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Generate a unique team ID with prefix
pub fn generate_team_id() -> String {
    format!("team_{}", Uuid::new_v4().simple())
}

/// Team entity stored in database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Request body for creating a new team
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Request body for updating a team
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTeamRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl UpdateTeamRequest {
    /// Validate team update request
    pub fn validate(&self) -> Result<(), String> {
        if let Some(name) = &self.name {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err("Team name cannot be empty".to_string());
            }
            if trimmed.len() > 100 {
                return Err("Team name cannot exceed 100 characters".to_string());
            }
        }
        if let Some(desc) = &self.description {
            if desc.len() > 500 {
                return Err("Description cannot exceed 500 characters".to_string());
            }
        }
        Ok(())
    }
}

/// Request body for adding a member to a team
#[derive(Debug, Clone, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: String,
}

/// Request body for assigning a device to a team
#[derive(Debug, Clone, Deserialize)]
pub struct AssignDeviceRequest {
    pub team_id: Option<String>, // None to unassign
}

/// Team member information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub id: String,
    pub email: String,
    pub role: String,
}

/// Team details response with members and device count
#[derive(Debug, Clone, Serialize)]
pub struct TeamDetails {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub member_count: i64,
    pub device_count: i64,
    pub members: Vec<TeamMember>,
    pub created_at: String,
    pub updated_at: String,
}

/// Team list item with counts
#[derive(Debug, Clone, Serialize)]
pub struct TeamListItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub member_count: i64,
    pub device_count: i64,
}

/// Response for device team assignment
#[derive(Debug, Clone, Serialize)]
pub struct DeviceTeamAssignmentResponse {
    pub udid: String,
    pub team_id: Option<String>,
    pub message: String,
}

impl Team {
    /// Create a new team with generated ID and timestamps
    pub fn new(name: String, description: Option<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Team {
            id: generate_team_id(),
            name,
            description,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

impl CreateTeamRequest {
    /// Validate team creation request
    pub fn validate(&self) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Team name cannot be empty".to_string());
        }
        if name.len() > 100 {
            return Err("Team name cannot exceed 100 characters".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_team_id() {
        let id = generate_team_id();
        assert!(id.starts_with("team_"));
        assert!(id.len() > 10);
    }

    #[test]
    fn test_team_new() {
        let team = Team::new("Test Team".to_string(), Some("A test team".to_string()));
        assert!(team.id.starts_with("team_"));
        assert_eq!(team.name, "Test Team");
        assert_eq!(team.description, Some("A test team".to_string()));
    }

    #[test]
    fn test_create_team_request_validation() {
        // Valid request
        let req = CreateTeamRequest {
            name: "Valid Team".to_string(),
            description: None,
        };
        assert!(req.validate().is_ok());

        // Empty name
        let req = CreateTeamRequest {
            name: "".to_string(),
            description: None,
        };
        assert!(req.validate().is_err());

        // Name too long
        let req = CreateTeamRequest {
            name: "x".repeat(101),
            description: None,
        };
        assert!(req.validate().is_err());

        // Whitespace-only name
        let req = CreateTeamRequest {
            name: "   ".to_string(),
            description: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_team_serialization() {
        let team = Team::new("Test Team".to_string(), None);
        let json = serde_json::to_string(&team).unwrap();
        assert!(json.contains("Test Team"));
        assert!(json.contains("team_"));
    }
}
