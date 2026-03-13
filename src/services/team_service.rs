//! Team service (Story 14-3)
//!
//! Handles team CRUD operations, membership management, and device assignment.

use chrono::Utc;
use serde_json::json;
use sqlx::SqlitePool;
use tracing::info;

use crate::models::team::{
    CreateTeamRequest, Team, TeamDetails, TeamListItem,
    TeamMember, UpdateTeamRequest,
};
use crate::models::user::User;

/// Team service errors
#[derive(Debug, Clone)]
pub enum TeamError {
    TeamNotFound,
    UserNotFound,
    DeviceNotFound,
    UserAlreadyInTeam,
    CannotDeleteTeamWithMembers,
    CannotDeleteTeamWithDevices,
    DatabaseError(String),
    ValidationError(String),
}

impl std::fmt::Display for TeamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamError::TeamNotFound => write!(f, "Team not found"),
            TeamError::UserNotFound => write!(f, "User not found"),
            TeamError::DeviceNotFound => write!(f, "Device not found"),
            TeamError::UserAlreadyInTeam => write!(f, "User is already in a team"),
            TeamError::CannotDeleteTeamWithMembers => {
                write!(f, "Cannot delete team with members. Remove members first.")
            }
            TeamError::CannotDeleteTeamWithDevices => {
                write!(f, "Cannot delete team with devices. Remove devices first.")
            }
            TeamError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            TeamError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for TeamError {}

/// Audit log target types
pub enum AuditTargetType {
    Team,
    User,
    Device,
}

impl std::fmt::Display for AuditTargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditTargetType::Team => write!(f, "team"),
            AuditTargetType::User => write!(f, "user"),
            AuditTargetType::Device => write!(f, "device"),
        }
    }
}

/// Team service for CRUD operations and membership management
pub struct TeamService {
    pool: SqlitePool,
}

impl TeamService {
    pub fn new(pool: SqlitePool) -> Self {
        TeamService { pool }
    }

    /// Log an audit event for team operations (Story 14-3 AC requirement)
    async fn audit_log(
        &self,
        action: &str,
        actor_id: &str,
        actor_email: Option<&str>,
        target_type: AuditTargetType,
        target_id: &str,
        team_id: Option<&str>,
        details: Option<&str>,
    ) -> Result<(), TeamError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO audit_log (action, actor_id, actor_email, target_type, target_id, team_id, details, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(action)
        .bind(actor_id)
        .bind(actor_email)
        .bind(target_type.to_string())
        .bind(target_id)
        .bind(team_id)
        .bind(details)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        info!(
            action = %action,
            actor_id = %actor_id,
            target_type = %target_type,
            target_id = %target_id,
            "Audit log entry created"
        );

        Ok(())
    }

    /// Create a new team
    pub async fn create_team(&self, req: &CreateTeamRequest, actor_id: &str, actor_email: Option<&str>) -> Result<Team, TeamError> {
        req.validate().map_err(TeamError::ValidationError)?;

        let team = Team::new(req.name.clone(), req.description.clone());

        sqlx::query(
            r#"
            INSERT INTO teams (id, name, description, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&team.id)
        .bind(&team.name)
        .bind(&team.description)
        .bind(&team.created_at)
        .bind(&team.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        // Create audit log entry
        let details = json!({
            "team_name": team.name,
            "description": team.description
        }).to_string();
        self.audit_log(
            "team.created",
            actor_id,
            actor_email,
            AuditTargetType::Team,
            &team.id,
            None,
            Some(&details),
        ).await?;

        info!(
            team_id = %team.id,
            team_name = %team.name,
            "Team created"
        );

        Ok(team)
    }

    /// Update a team
    pub async fn update_team(&self, team_id: &str, req: &UpdateTeamRequest, actor_id: &str, actor_email: Option<&str>) -> Result<Team, TeamError> {
        // Validate request
        req.validate().map_err(TeamError::ValidationError)?;

        // Check if team exists
        let existing: Option<Team> = sqlx::query_as("SELECT * FROM teams WHERE id = ?")
            .bind(team_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        if existing.is_none() {
            return Err(TeamError::TeamNotFound);
        }

        let now = chrono::Utc::now().to_rfc3339();
        let existing = existing.unwrap();
        let new_name = req.name.as_ref().unwrap_or(&existing.name);
        let new_description = req.description.as_ref().or(existing.description.as_ref());

        sqlx::query(
            r#"
            UPDATE teams SET name = ?, description = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(new_name)
        .bind(new_description)
        .bind(&now)
        .bind(team_id)
        .execute(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        let updated: Team = sqlx::query_as("SELECT * FROM teams WHERE id = ?")
            .bind(team_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        // Create audit log entry
        let details = json!({
            "old_name": existing.name,
            "new_name": new_name,
            "description_updated": req.description.is_some()
        }).to_string();
        self.audit_log(
            "team.updated",
            actor_id,
            actor_email,
            AuditTargetType::Team,
            team_id,
            Some(team_id),
            Some(&details),
        ).await?;

        info!(team_id = %team_id, "Team updated");

        Ok(updated)
    }

    /// Delete a team (only if no members or devices)
    pub async fn delete_team(&self, team_id: &str) -> Result<(), TeamError> {
        // Check member count
        let member_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM users WHERE team_id = ?",
        )
        .bind(team_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        if member_count.0 > 0 {
            return Err(TeamError::CannotDeleteTeamWithMembers);
        }

        // Check device count
        let device_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM devices WHERE team_id = ?",
        )
        .bind(team_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        if device_count.0 > 0 {
            return Err(TeamError::CannotDeleteTeamWithDevices);
        }

        sqlx::query("DELETE FROM teams WHERE id = ?")
            .bind(team_id)
            .execute(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        info!(team_id = %team_id, "Team deleted");

        Ok(())
    }

    /// Get a team by ID
    pub async fn get_team(&self, team_id: &str) -> Result<Team, TeamError> {
        let team: Option<Team> = sqlx::query_as("SELECT * FROM teams WHERE id = ?")
            .bind(team_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        team.ok_or(TeamError::TeamNotFound)
    }

    /// List all teams with member and device counts (optimized single query)
    pub async fn list_teams(&self) -> Result<Vec<TeamListItem>, TeamError> {
        // Single query with LEFT JOINs to avoid N+1 problem
        let rows: Vec<(String, String, Option<String>, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                t.id,
                t.name,
                t.description,
                COALESCE(u.member_count, 0) as member_count,
                COALESCE(d.device_count, 0) as device_count
            FROM teams t
            LEFT JOIN (
                SELECT team_id, COUNT(*) as member_count
                FROM users
                WHERE team_id IS NOT NULL
                GROUP BY team_id
            ) u ON t.id = u.team_id
            LEFT JOIN (
                SELECT team_id, COUNT(*) as device_count
                FROM devices
                WHERE team_id IS NOT NULL
                GROUP BY team_id
            ) d ON t.id = d.team_id
            ORDER BY t.name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        let result = rows
            .into_iter()
            .map(|(id, name, description, member_count, device_count)| TeamListItem {
                id,
                name,
                description,
                member_count,
                device_count,
            })
            .collect();

        Ok(result)
    }

    /// Get team details with members and device count
    pub async fn get_team_details(&self, team_id: &str) -> Result<TeamDetails, TeamError> {
        let team = self.get_team(team_id).await?;

        // Get member count
        let member_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM users WHERE team_id = ?",
        )
        .bind(team_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        // Get device count
        let device_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM devices WHERE team_id = ?",
        )
        .bind(team_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        // Get team members
        let members: Vec<TeamMember> = sqlx::query_as(
            r#"
            SELECT id, email, role FROM users WHERE team_id = ?
            "#,
        )
        .bind(team_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(id, email, role): (String, String, String)| TeamMember { id, email, role })
                .collect()
        })
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        Ok(TeamDetails {
            id: team.id,
            name: team.name,
            description: team.description,
            member_count: member_count.0,
            device_count: device_count.0,
            members,
            created_at: team.created_at,
            updated_at: team.updated_at,
        })
    }

    /// Add a member to a team
    pub async fn add_member(&self, team_id: &str, user_id: &str, actor_id: &str, actor_email: Option<&str>) -> Result<User, TeamError> {
        // Check team exists
        let team_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM teams WHERE id = ?")
            .bind(team_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        if team_exists.0 == 0 {
            return Err(TeamError::TeamNotFound);
        }

        // Check user exists and get current team
        let current_team: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT team_id FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        if current_team.is_none() {
            return Err(TeamError::UserNotFound);
        }

        // Update user's team
        sqlx::query("UPDATE users SET team_id = ? WHERE id = ?")
            .bind(team_id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        // Create audit log entry
        let details = json!({
            "action": "member_added",
            "team_id": team_id,
            "user_id": user_id
        }).to_string();
        self.audit_log(
            "team.member_added",
            actor_id,
            actor_email,
            AuditTargetType::User,
            user_id,
            Some(team_id),
            Some(&details),
        ).await?;

        info!(team_id = %team_id, user_id = %user_id, "Member added to team");

        // Fetch and return updated user
        let user: Option<(String, String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
            "SELECT id, email, role, team_id, created_at, last_login_at FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        match user {
            Some((id, email, role, team_id, created_at, last_login_at)) => Ok(User {
                id,
                email,
                password_hash: String::new(),
                role,
                team_id,
                created_at,
                last_login_at,
            }),
            None => Err(TeamError::UserNotFound),
        }
    }

    /// Remove a member from a team
    pub async fn remove_member(&self, team_id: &str, user_id: &str, actor_id: &str, actor_email: Option<&str>) -> Result<(), TeamError> {
        // Verify user is in this team
        let current_team: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT team_id FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        if current_team.is_none() {
            return Err(TeamError::UserNotFound);
        }

        if current_team.unwrap().0.as_deref() != Some(team_id) {
            return Err(TeamError::UserNotFound);
        }

        // Remove from team
        sqlx::query("UPDATE users SET team_id = NULL WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        // Create audit log entry
        let details = json!({
            "action": "member_removed",
            "team_id": team_id,
            "user_id": user_id
        }).to_string();
        self.audit_log(
            "team.member_removed",
            actor_id,
            actor_email,
            AuditTargetType::User,
            user_id,
            Some(team_id),
            Some(&details),
        ).await?;

        info!(team_id = %team_id, user_id = %user_id, "Member removed from team");

        Ok(())
    }

    /// Assign a device to a team
    pub async fn assign_device(&self, udid: &str, team_id: &str, actor_id: &str, actor_email: Option<&str>) -> Result<(), TeamError> {
        // Verify team exists
        let team_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM teams WHERE id = ?")
            .bind(team_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        if team_exists.0 == 0 {
            return Err(TeamError::TeamNotFound);
        }

        // Verify device exists
        let device_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices WHERE udid = ?")
            .bind(udid)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        if device_exists.0 == 0 {
            return Err(TeamError::DeviceNotFound);
        }

        // Assign device to team
        sqlx::query("UPDATE devices SET team_id = ? WHERE udid = ?")
            .bind(team_id)
            .bind(udid)
            .execute(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        // Create audit log entry
        let details = json!({
            "action": "device_assigned",
            "team_id": team_id,
            "udid": udid
        }).to_string();
        self.audit_log(
            "team.device_assigned",
            actor_id,
            actor_email,
            AuditTargetType::Device,
            udid,
            Some(team_id),
            Some(&details),
        ).await?;

        info!(team_id = %team_id, udid = %udid, "Device assigned to team");

        Ok(())
    }

    /// Remove a device from its team
    pub async fn remove_device(&self, udid: &str, actor_id: &str, actor_email: Option<&str>) -> Result<(), TeamError> {
        // Verify device exists
        let device_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices WHERE udid = ?")
            .bind(udid)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        if device_exists.0 == 0 {
            return Err(TeamError::DeviceNotFound);
        }

        // Remove device from team
        sqlx::query("UPDATE devices SET team_id = NULL WHERE udid = ?")
            .bind(udid)
            .execute(&self.pool)
            .await
            .map_err(|e| TeamError::DatabaseError(e.to_string()))?;

        // Create audit log entry
        let details = json!({
            "action": "device_removed",
            "udid": udid
        }).to_string();
        self.audit_log(
            "team.device_removed",
            actor_id,
            actor_email,
            AuditTargetType::Device,
            udid,
            None,
            Some(&details),
        ).await?;

        info!(udid = %udid, "Device removed from team");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_team_error_display() {
        assert_eq!(
            TeamError::TeamNotFound.to_string(),
            "Team not found"
        );
        assert_eq!(
            TeamError::CannotDeleteTeamWithMembers.to_string(),
            "Cannot delete team with members. Remove members first."
        );
    }
}
