//! Audit service (Story 14-5)
//!
//! Provides centralized audit logging and querying for all system events.

use chrono::Utc;
use sqlx::SqlitePool;
use tracing::info;

use crate::models::audit::{
    AuditEntry, AuditListResponse, AuditQueryParams, CreateAuditEntry, PaginationInfo,
};

/// Audit service errors
#[derive(Debug, Clone)]
pub enum AuditError {
    DatabaseError(String),
    InvalidDateRange(String),
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            AuditError::InvalidDateRange(msg) => write!(f, "Invalid date range: {}", msg),
        }
    }
}

impl std::error::Error for AuditError {}

/// Audit service for centralized audit logging
pub struct AuditService {
    pool: SqlitePool,
}

impl AuditService {
    pub fn new(pool: SqlitePool) -> Self {
        AuditService { pool }
    }

    /// Log an audit event
    pub async fn log_event(&self, entry: &CreateAuditEntry) -> Result<i64, AuditError> {
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            INSERT INTO audit_log (action, actor_id, actor_email, target_type, target_id, team_id, details, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&entry.action)
        .bind(&entry.actor_id)
        .bind(&entry.actor_email)
        .bind(&entry.target_type)
        .bind(&entry.target_id)
        .bind(&entry.team_id)
        .bind(&entry.details)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| AuditError::DatabaseError(e.to_string()))?;

        let id = result.last_insert_rowid();

        info!(
            action = %entry.action,
            actor_id = %entry.actor_id,
            target_type = %entry.target_type,
            target_id = %entry.target_id,
            audit_id = id,
            "Audit log entry created"
        );

        Ok(id)
    }

    /// List audit log entries with filtering and pagination
    pub async fn list_entries(
        &self,
        params: &AuditQueryParams,
    ) -> Result<AuditListResponse, AuditError> {
        let params = params.normalize();

        // Build conditions and get total count
        let total = self.get_filtered_count(&params).await?;

        let total_pages = if params.per_page > 0 {
            (total + params.per_page - 1) / params.per_page
        } else {
            1
        };

        // Get paginated entries
        let entries = self.get_filtered_entries(&params).await?;

        // Convert to response format
        let entry_responses = entries
            .into_iter()
            .map(crate::models::audit::AuditEntryResponse::from)
            .collect();

        Ok(AuditListResponse {
            entries: entry_responses,
            pagination: PaginationInfo {
                total,
                page: params.page,
                per_page: params.per_page,
                total_pages,
            },
        })
    }

    /// Get total count with filters applied
    async fn get_filtered_count(&self, params: &AuditQueryParams) -> Result<i64, AuditError> {
        // Build query based on which filters are present
        let (query, bindings) = self.build_count_query(params);

        // Execute with dynamic bindings
        let total: (i64,) = self.execute_count_query(&query, &bindings).await?;

        Ok(total.0)
    }

    /// Build count query with dynamic conditions
    fn build_count_query(&self, params: &AuditQueryParams) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut bindings = Vec::new();

        if let Some(user_id) = &params.user_id {
            conditions.push("actor_id = ?");
            bindings.push(user_id.clone());
        }

        if let Some(action) = &params.action {
            conditions.push("action = ?");
            bindings.push(action.clone());
        }

        if let Some(target_type) = &params.target_type {
            conditions.push("target_type = ?");
            bindings.push(target_type.clone());
        }

        if let Some(start_date) = &params.start_date {
            conditions.push("created_at >= ?");
            bindings.push(format!("{}T00:00:00Z", start_date));
        }

        if let Some(end_date) = &params.end_date {
            conditions.push("created_at <= ?");
            bindings.push(format!("{}T23:59:59Z", end_date));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let query = format!("SELECT COUNT(*) FROM audit_log {}", where_clause);
        (query, bindings)
    }

    /// Execute count query with dynamic bindings
    async fn execute_count_query(&self, query: &str, bindings: &[String]) -> Result<(i64,), AuditError> {
        // Handle different numbers of bindings
        match bindings.len() {
            0 => sqlx::query_as(query)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            1 => sqlx::query_as(query)
                .bind(&bindings[0])
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            2 => sqlx::query_as(query)
                .bind(&bindings[0])
                .bind(&bindings[1])
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            3 => sqlx::query_as(query)
                .bind(&bindings[0])
                .bind(&bindings[1])
                .bind(&bindings[2])
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            4 => sqlx::query_as(query)
                .bind(&bindings[0])
                .bind(&bindings[1])
                .bind(&bindings[2])
                .bind(&bindings[3])
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            5 => sqlx::query_as(query)
                .bind(&bindings[0])
                .bind(&bindings[1])
                .bind(&bindings[2])
                .bind(&bindings[3])
                .bind(&bindings[4])
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            _ => Err(AuditError::DatabaseError("Too many filter conditions".to_string())),
        }
    }

    /// Get filtered and paginated entries
    async fn get_filtered_entries(&self, params: &AuditQueryParams) -> Result<Vec<AuditEntry>, AuditError> {
        let (query, bindings) = self.build_entries_query(params);

        // Execute with dynamic bindings + pagination bindings
        self.execute_entries_query(&query, &bindings, params.per_page, params.offset()).await
    }

    /// Build entries query with dynamic conditions
    fn build_entries_query(&self, params: &AuditQueryParams) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut bindings = Vec::new();

        if let Some(user_id) = &params.user_id {
            conditions.push("actor_id = ?");
            bindings.push(user_id.clone());
        }

        if let Some(action) = &params.action {
            conditions.push("action = ?");
            bindings.push(action.clone());
        }

        if let Some(target_type) = &params.target_type {
            conditions.push("target_type = ?");
            bindings.push(target_type.clone());
        }

        if let Some(start_date) = &params.start_date {
            conditions.push("created_at >= ?");
            bindings.push(format!("{}T00:00:00Z", start_date));
        }

        if let Some(end_date) = &params.end_date {
            conditions.push("created_at <= ?");
            bindings.push(format!("{}T23:59:59Z", end_date));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let query = format!(
            "SELECT id, action, actor_id, actor_email, target_type, target_id, team_id, details, created_at \
             FROM audit_log {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            where_clause
        );

        (query, bindings)
    }

    /// Execute entries query with dynamic bindings
    async fn execute_entries_query(
        &self,
        query: &str,
        bindings: &[String],
        per_page: i64,
        offset: i64,
    ) -> Result<Vec<AuditEntry>, AuditError> {
        // Handle different numbers of bindings
        match bindings.len() {
            0 => sqlx::query_as(query)
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            1 => sqlx::query_as(query)
                .bind(&bindings[0])
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            2 => sqlx::query_as(query)
                .bind(&bindings[0])
                .bind(&bindings[1])
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            3 => sqlx::query_as(query)
                .bind(&bindings[0])
                .bind(&bindings[1])
                .bind(&bindings[2])
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            4 => sqlx::query_as(query)
                .bind(&bindings[0])
                .bind(&bindings[1])
                .bind(&bindings[2])
                .bind(&bindings[3])
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            5 => sqlx::query_as(query)
                .bind(&bindings[0])
                .bind(&bindings[1])
                .bind(&bindings[2])
                .bind(&bindings[3])
                .bind(&bindings[4])
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AuditError::DatabaseError(e.to_string())),
            _ => Err(AuditError::DatabaseError("Too many filter conditions".to_string())),
        }
    }

    /// Get audit entries for a specific team (team-scoped access)
    pub async fn list_team_entries(
        &self,
        team_id: &str,
        params: &AuditQueryParams,
    ) -> Result<AuditListResponse, AuditError> {
        let params = params.normalize();

        // Build query with team filter as base
        let total = self.get_team_filtered_count(team_id, &params).await?;

        let total_pages = if params.per_page > 0 {
            (total + params.per_page - 1) / params.per_page
        } else {
            1
        };

        let entries = self.get_team_filtered_entries(team_id, &params).await?;

        let entry_responses = entries
            .into_iter()
            .map(crate::models::audit::AuditEntryResponse::from)
            .collect();

        Ok(AuditListResponse {
            entries: entry_responses,
            pagination: PaginationInfo {
                total,
                page: params.page,
                per_page: params.per_page,
                total_pages,
            },
        })
    }

    async fn get_team_filtered_count(&self, team_id: &str, params: &AuditQueryParams) -> Result<i64, AuditError> {
        let (query, bindings) = self.build_team_count_query(team_id, params);
        let total: (i64,) = self.execute_count_query(&query, &bindings).await?;
        Ok(total.0)
    }

    fn build_team_count_query(&self, team_id: &str, params: &AuditQueryParams) -> (String, Vec<String>) {
        let mut conditions = vec!["team_id = ?".to_string()];
        let mut bindings = vec![team_id.to_string()];

        if let Some(user_id) = &params.user_id {
            conditions.push("actor_id = ?".to_string());
            bindings.push(user_id.clone());
        }

        if let Some(action) = &params.action {
            conditions.push("action = ?".to_string());
            bindings.push(action.clone());
        }

        if let Some(target_type) = &params.target_type {
            conditions.push("target_type = ?".to_string());
            bindings.push(target_type.clone());
        }

        if let Some(start_date) = &params.start_date {
            conditions.push("created_at >= ?".to_string());
            bindings.push(format!("{}T00:00:00Z", start_date));
        }

        if let Some(end_date) = &params.end_date {
            conditions.push("created_at <= ?".to_string());
            bindings.push(format!("{}T23:59:59Z", end_date));
        }

        let query = format!("SELECT COUNT(*) FROM audit_log WHERE {}", conditions.join(" AND "));
        (query, bindings)
    }

    async fn get_team_filtered_entries(&self, team_id: &str, params: &AuditQueryParams) -> Result<Vec<AuditEntry>, AuditError> {
        let (query, bindings) = self.build_team_entries_query(team_id, params);
        self.execute_entries_query(&query, &bindings, params.per_page, params.offset()).await
    }

    fn build_team_entries_query(&self, team_id: &str, params: &AuditQueryParams) -> (String, Vec<String>) {
        let mut conditions = vec!["team_id = ?".to_string()];
        let mut bindings = vec![team_id.to_string()];

        if let Some(user_id) = &params.user_id {
            conditions.push("actor_id = ?".to_string());
            bindings.push(user_id.clone());
        }

        if let Some(action) = &params.action {
            conditions.push("action = ?".to_string());
            bindings.push(action.clone());
        }

        if let Some(target_type) = &params.target_type {
            conditions.push("target_type = ?".to_string());
            bindings.push(target_type.clone());
        }

        if let Some(start_date) = &params.start_date {
            conditions.push("created_at >= ?".to_string());
            bindings.push(format!("{}T00:00:00Z", start_date));
        }

        if let Some(end_date) = &params.end_date {
            conditions.push("created_at <= ?".to_string());
            bindings.push(format!("{}T23:59:59Z", end_date));
        }

        let query = format!(
            "SELECT id, action, actor_id, actor_email, target_type, target_id, team_id, details, created_at \
             FROM audit_log WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            conditions.join(" AND ")
        );

        (query, bindings)
    }

    /// Get recent audit entries for a specific user
    pub async fn get_user_recent_activity(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<AuditEntry>, AuditError> {
        let entries: Vec<AuditEntry> = sqlx::query_as(
            r#"
            SELECT id, action, actor_id, actor_email, target_type, target_id, team_id, details, created_at
            FROM audit_log
            WHERE actor_id = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AuditError::DatabaseError(e.to_string()))?;

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_error_display() {
        assert_eq!(
            AuditError::DatabaseError("connection failed".to_string()).to_string(),
            "Database error: connection failed"
        );
        assert_eq!(
            AuditError::InvalidDateRange("start after end".to_string()).to_string(),
            "Invalid date range: start after end"
        );
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
            per_page: 10,
        };

        let normalized = params.normalize();
        assert_eq!(normalized.offset(), 20); // (3-1) * 10
    }

    #[test]
    fn test_audit_query_params_per_page_cap() {
        let params = AuditQueryParams {
            user_id: None,
            action: None,
            target_type: None,
            start_date: None,
            end_date: None,
            page: 1,
            per_page: 500, // Should be capped
        };

        let normalized = params.normalize();
        assert_eq!(normalized.per_page, 100);
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
        assert_eq!(entry.target_type, "session");
        assert_eq!(entry.target_id, "rt_abc");

        let details: serde_json::Value = serde_json::from_str(&entry.details.unwrap()).unwrap();
        assert_eq!(details["ip_address"], "192.168.1.1");
        assert_eq!(details["user_agent"], "Mozilla/5.0");
    }

    #[test]
    fn test_create_audit_entry_user_login_failed() {
        let entry = CreateAuditEntry::user_login_failed(
            "test@example.com",
            "invalid_credentials",
            Some("10.0.0.1"),
        );

        assert_eq!(entry.action, "user.login_failed");
        assert_eq!(entry.actor_id, "anonymous");
        assert_eq!(entry.target_id, "n/a");

        let details: serde_json::Value = serde_json::from_str(&entry.details.unwrap()).unwrap();
        assert_eq!(details["reason"], "invalid_credentials");
        assert_eq!(details["ip_address"], "10.0.0.1");
    }

    #[test]
    fn test_create_audit_entry_user_logout() {
        let entry = CreateAuditEntry::user_logout("user_123", "rt_xyz");

        assert_eq!(entry.action, "user.logout");
        assert_eq!(entry.actor_id, "user_123");
        assert_eq!(entry.target_type, "session");
        assert_eq!(entry.target_id, "rt_xyz");
        assert!(entry.details.is_none());
    }

    #[test]
    fn test_create_audit_entry_session_revoked() {
        let entry = CreateAuditEntry::session_revoked("user_123", "rt_abc");

        assert_eq!(entry.action, "session.revoked");
        assert_eq!(entry.actor_id, "user_123");
        assert_eq!(entry.target_type, "session");
        assert_eq!(entry.target_id, "rt_abc");
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
        assert_eq!(entry.actor_email, Some("admin@example.com".to_string()));
        assert_eq!(entry.target_type, "user");
        assert_eq!(entry.target_id, "user_456");

        let details: serde_json::Value = serde_json::from_str(&entry.details.unwrap()).unwrap();
        assert_eq!(details["old_role"], "viewer");
        assert_eq!(details["new_role"], "agent");
    }
}
