//! Authentication service (Story 14-1)
//!
//! Handles password hashing, JWT token generation/validation, and user authentication.

use argon2::{
    password_hash::{
        rand_core::OsRng, PasswordHash, PasswordHasher as ArgonPasswordHasher, PasswordVerifier,
        SaltString,
    },
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sqlx::SqlitePool;

use crate::config::AuthConfig;
use crate::models::session::{RevokeAllSessionsResponse, Session, SessionInfo, SessionListResponse, SessionRevokeResponse};
use crate::models::user::{
    generate_refresh_token, generate_refresh_token_id, generate_user_id, validate_password,
    AccessTokenClaims, AuthResponse, RefreshResponse, RefreshToken, RegisterResponse, User, UserInfo,
};

/// Authentication errors
#[derive(Debug, Clone)]
pub enum AuthError {
    InvalidCredentials,
    TokenExpired,
    TokenRevoked,
    TokenInvalid,
    EmailAlreadyExists,
    PasswordTooWeak(Vec<String>),
    UserNotFound,
    SessionNotFound,
    DatabaseError(String),
    ConfigError(String),
    RateLimited,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidCredentials => write!(f, "Invalid credentials"),
            AuthError::TokenExpired => write!(f, "Token expired"),
            AuthError::TokenRevoked => write!(f, "Token revoked"),
            AuthError::TokenInvalid => write!(f, "Invalid token"),
            AuthError::EmailAlreadyExists => write!(f, "Email already registered"),
            AuthError::PasswordTooWeak(errors) => {
                write!(f, "Password requirements not met: {}", errors.join(", "))
            }
            AuthError::UserNotFound => write!(f, "User not found"),
            AuthError::SessionNotFound => write!(f, "Session not found"),
            AuthError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            AuthError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            AuthError::RateLimited => write!(f, "Too many requests"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Password hasher using Argon2id
pub struct PasswordHasher {
    argon2: Argon2<'static>,
}

impl Default for PasswordHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordHasher {
    pub fn new() -> Self {
        Self {
            argon2: Argon2::default(),
        }
    }

    /// Hash a password with a random salt using Argon2id
    pub fn hash(&self, password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AuthError::ConfigError(format!("Password hashing failed: {}", e)))?;
        Ok(hash.to_string())
    }

    /// Verify a password against a stored hash
    pub fn verify(&self, password: &str, hash: &str) -> bool {
        let parsed_hash = match PasswordHash::new(hash) {
            Ok(h) => h,
            Err(_) => return false,
        };
        self.argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    }
}

/// JWT token service
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_token_expiry: Duration,
    refresh_token_expiry: Duration,
    #[allow(dead_code)]
    jwt_secret: String, // Reserved for future use (e.g., token rotation)
}

impl JwtService {
    pub fn new(config: &AuthConfig) -> Result<Self, AuthError> {
        let secret = config
            .jwt_secret
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("JWT_SECRET not configured".to_string()))?;

        if secret.len() < 32 {
            return Err(AuthError::ConfigError(
                "JWT_SECRET must be at least 32 characters".to_string(),
            ));
        }

        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            access_token_expiry: Duration::minutes(config.access_token_expiry_minutes as i64),
            refresh_token_expiry: Duration::days(config.refresh_token_expiry_days as i64),
            jwt_secret: secret.clone(),
        })
    }

    /// Generate an access token (JWT)
    pub fn generate_access_token(
        &self,
        user_id: &str,
        email: &str,
        role: &str,
        team_id: Option<&str>,
    ) -> Result<(String, u64), AuthError> {
        let now = Utc::now();
        let exp = now + self.access_token_expiry;

        let claims = AccessTokenClaims {
            sub: user_id.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            team_id: team_id.map(|s| s.to_string()),
            exp: exp.timestamp(),
            iat: now.timestamp(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AuthError::ConfigError(format!("JWT encoding failed: {}", e)))?;

        let expires_in = self.access_token_expiry.num_seconds() as u64;

        Ok((token, expires_in))
    }

    /// Validate an access token and extract claims
    pub fn validate_access_token(&self, token: &str) -> Result<AccessTokenClaims, AuthError> {
        let token_data = decode::<AccessTokenClaims>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("ExpiredSignature") {
                    AuthError::TokenExpired
                } else {
                    AuthError::TokenInvalid
                }
            })?;

        Ok(token_data.claims)
    }

    /// Get refresh token expiry duration
    pub fn refresh_token_expiry(&self) -> Duration {
        self.refresh_token_expiry
    }
}

/// Authentication service
pub struct AuthService {
    pool: SqlitePool,
    password_hasher: PasswordHasher,
    jwt_service: JwtService,
}

impl AuthService {
    pub fn new(pool: SqlitePool, config: &AuthConfig) -> Result<Self, AuthError> {
        Ok(Self {
            pool,
            password_hasher: PasswordHasher::new(),
            jwt_service: JwtService::new(config)?,
        })
    }

    /// Register a new user
    pub async fn register(
        &self,
        email: &str,
        password: &str,
    ) -> Result<RegisterResponse, AuthError> {
        // Validate password strength
        let validation = validate_password(password);
        if !validation.is_valid {
            return Err(AuthError::PasswordTooWeak(validation.errors));
        }

        // Check if email already exists
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM users WHERE email = ?")
                .bind(email)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        if existing.is_some() {
            return Err(AuthError::EmailAlreadyExists);
        }

        // Hash password
        let password_hash = self.password_hasher.hash(password)?;

        // Create user
        let user_id = generate_user_id();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, role, created_at)
            VALUES (?, ?, ?, 'agent', ?)
            "#,
        )
        .bind(&user_id)
        .bind(email)
        .bind(&password_hash)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        tracing::info!("User registered: {} ({})", email, user_id);

        Ok(RegisterResponse {
            id: user_id,
            email: email.to_string(),
            role: "agent".to_string(),
            created_at: now,
        })
    }

    /// Login a user
    pub async fn login(
        &self,
        email: &str,
        password: &str,
        user_agent: Option<&str>,
        ip_address: Option<&str>,
    ) -> Result<AuthResponse, AuthError> {
        // Find user (includes team_id for Story 14-3)
        let user: Option<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, password_hash, role, team_id FROM users WHERE email = ?",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        let (user_id, password_hash, role, team_id) = match user {
            Some(u) => u,
            None => {
                tracing::warn!("Login failed: user not found for email {}", email);
                return Err(AuthError::InvalidCredentials);
            }
        };

        // Verify password
        if !self.password_hasher.verify(password, &password_hash) {
            tracing::warn!("Login failed: invalid password for user {}", user_id);
            return Err(AuthError::InvalidCredentials);
        }

        // Generate tokens
        let (access_token, expires_in) =
            self.jwt_service
                .generate_access_token(&user_id, email, &role, team_id.as_deref())?;

        let refresh_token = generate_refresh_token();
        let refresh_token_hash = self.hash_refresh_token(&refresh_token);
        let refresh_token_id = generate_refresh_token_id();
        let now = Utc::now();
        let expires_at = now + self.jwt_service.refresh_token_expiry();

        // Store refresh token (with optional metadata for session management - Story 14-4)
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, created_at, user_agent, ip_address)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&refresh_token_id)
        .bind(&user_id)
        .bind(&refresh_token_hash)
        .bind(expires_at.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(user_agent)
        .bind(ip_address)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        // Update last_login_at
        sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
            .bind(now.to_rfc3339())
            .bind(&user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        tracing::info!("User logged in: {} ({})", email, user_id);

        Ok(AuthResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in,
            user: UserInfo {
                id: user_id,
                email: email.to_string(),
                role,
                team_id,
            },
        })
    }

    /// Refresh access token
    pub async fn refresh(&self, refresh_token: &str) -> Result<RefreshResponse, AuthError> {
        let token_hash = self.hash_refresh_token(refresh_token);

        // Find and validate refresh token
        let token_record: Option<RefreshToken> = sqlx::query_as(
            r#"
            SELECT id, user_id, token_hash, expires_at, revoked, created_at
            FROM refresh_tokens
            WHERE token_hash = ?
            "#,
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        let token = match token_record {
            Some(t) => t,
            None => return Err(AuthError::TokenRevoked),
        };

        // Check if revoked
        if token.revoked != 0 {
            tracing::warn!("Attempted to use revoked refresh token: {}", token.id);
            return Err(AuthError::TokenRevoked);
        }

        // Check if expired
        let expires_at = chrono::DateTime::parse_from_rfc3339(&token.expires_at)
            .map_err(|_| AuthError::TokenInvalid)?;
        if expires_at < Utc::now() {
            tracing::warn!("Attempted to use expired refresh token: {}", token.id);
            return Err(AuthError::TokenExpired);
        }

        // Get user info (includes team_id for Story 14-3)
        let user: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT email, role, team_id FROM users WHERE id = ?",
        )
        .bind(&token.user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        let (email, role, team_id) = match user {
            Some(u) => u,
            None => return Err(AuthError::UserNotFound),
        };

        // Update last_used_at and revoke old token (Story 14-4)
        let now = Utc::now();
        sqlx::query("UPDATE refresh_tokens SET revoked = 1, last_used_at = ? WHERE id = ?")
            .bind(now.to_rfc3339())
            .bind(&token.id)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        // Generate new tokens
        let (access_token, expires_in) = self.jwt_service.generate_access_token(
            &token.user_id,
            &email,
            &role,
            team_id.as_deref(),
        )?;

        let new_refresh_token = generate_refresh_token();
        let new_token_hash = self.hash_refresh_token(&new_refresh_token);
        let new_token_id = generate_refresh_token_id();
        let now = Utc::now();
        let new_expires_at = now + self.jwt_service.refresh_token_expiry();

        // Store new refresh token
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&new_token_id)
        .bind(&token.user_id)
        .bind(&new_token_hash)
        .bind(new_expires_at.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        tracing::info!("Token refreshed for user: {}", token.user_id);

        Ok(RefreshResponse {
            access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in,
        })
    }

    /// Logout user (revoke refresh token)
    pub async fn logout(&self, refresh_token: &str) -> Result<(), AuthError> {
        let token_hash = self.hash_refresh_token(refresh_token);

        let result = sqlx::query("UPDATE refresh_tokens SET revoked = 1 WHERE token_hash = ?")
            .bind(&token_hash)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        if result.rows_affected() > 0 {
            tracing::info!("User logged out (refresh token revoked)");
        }

        Ok(())
    }

    /// Logout all sessions for a user
    pub async fn logout_all(&self, user_id: &str) -> Result<u64, AuthError> {
        let result = sqlx::query("UPDATE refresh_tokens SET revoked = 1 WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        let count = result.rows_affected();
        tracing::info!("Logged out all sessions for user {}: {} tokens revoked", user_id, count);

        Ok(count)
    }

    // ========================================================================
    // Session Management Methods (Story 14-4)
    // ========================================================================

    /// List active sessions for a user
    /// Returns sessions that are not revoked and not expired
    pub async fn list_sessions(&self, user_id: &str) -> Result<SessionListResponse, AuthError> {
        let now = Utc::now().to_rfc3339();

        let sessions: Vec<Session> = sqlx::query_as(
            r#"
            SELECT id, user_id, token_hash, expires_at, revoked, created_at,
                   last_used_at, user_agent, ip_address
            FROM refresh_tokens
            WHERE user_id = ? AND revoked = 0 AND expires_at > ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .bind(&now)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        let session_infos: Vec<SessionInfo> = sessions.into_iter().map(SessionInfo::from).collect();
        let total = session_infos.len();

        Ok(SessionListResponse {
            sessions: session_infos,
            total,
        })
    }

    /// Revoke a specific session (must belong to the user)
    /// Returns SessionRevokeResponse on success, AuthError::SessionNotFound if not found or not owned
    pub async fn revoke_session(&self, user_id: &str, session_id: &str) -> Result<SessionRevokeResponse, AuthError> {
        let result = sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked = 1
            WHERE id = ? AND user_id = ? AND revoked = 0
            "#,
        )
        .bind(session_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        if result.rows_affected() == 0 {
            // Session not found, not owned by user, or already revoked
            return Err(AuthError::SessionNotFound);
        }

        tracing::info!("Session revoked: {} for user {}", session_id, user_id);

        Ok(SessionRevokeResponse {
            message: "Session revoked successfully".to_string(),
            session_id: session_id.to_string(),
        })
    }

    /// Revoke all other sessions for a user, preserving the current session
    /// Returns the count of revoked sessions
    pub async fn revoke_all_other_sessions(
        &self,
        user_id: &str,
        current_session_id: &str,
    ) -> Result<RevokeAllSessionsResponse, AuthError> {
        let result = sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked = 1
            WHERE user_id = ? AND id != ? AND revoked = 0
            "#,
        )
        .bind(user_id)
        .bind(current_session_id)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        let count = result.rows_affected();
        tracing::info!(
            "Revoked {} other sessions for user {} (preserved: {})",
            count, user_id, current_session_id
        );

        Ok(RevokeAllSessionsResponse {
            message: "All other sessions revoked".to_string(),
            revoked_count: count,
        })
    }

    /// Get user by ID
    pub async fn get_user(&self, user_id: &str) -> Result<Option<UserInfo>, AuthError> {
        let user: Option<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, email, role, team_id FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        Ok(user.map(|(id, email, role, team_id)| UserInfo { id, email, role, team_id }))
    }

    /// Validate JWT access token
    pub fn validate_token(&self, token: &str) -> Result<AccessTokenClaims, AuthError> {
        self.jwt_service.validate_access_token(token)
    }

    /// Hash a refresh token using SHA-256
    fn hash_refresh_token(&self, token: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Update a user's role (admin only - Story 14-2)
    pub async fn update_user_role(&self, user_id: &str, new_role: &str) -> Result<User, AuthError> {
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE users SET role = ?, last_login_at = ? WHERE id = ?
            "#,
        )
        .bind(new_role)
        .bind(&now)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(AuthError::UserNotFound);
        }

        // Fetch and return updated user
        let user: Option<(String, String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
            "SELECT id, email, role, team_id, created_at, last_login_at FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        match user {
            Some((id, email, role, team_id, created_at, last_login_at)) => {
                Ok(User {
                    id,
                    email,
                    password_hash: String::new(), // Don't expose password hash
                    role,
                    team_id,
                    created_at,
                    last_login_at,
                })
            }
            None => Err(AuthError::UserNotFound),
        }
    }

    /// List all users (admin only - Story 14-2)
    pub async fn list_users(&self) -> Result<Vec<User>, AuthError> {
        let users: Vec<(String, String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
            "SELECT id, email, role, team_id, created_at, last_login_at FROM users ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        Ok(users
            .into_iter()
            .map(|(id, email, role, team_id, created_at, last_login_at)| User {
                id,
                email,
                password_hash: String::new(),
                role,
                team_id,
                created_at,
                last_login_at,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hasher() {
        let hasher = PasswordHasher::new();
        let password = "SecureP@ss123";
        let hash = hasher.hash(password).unwrap();

        // Hash should be non-empty and different from password
        assert!(!hash.is_empty());
        assert_ne!(hash, password);

        // Should verify correctly
        assert!(hasher.verify(password, &hash));

        // Wrong password should fail
        assert!(!hasher.verify("WrongPassword", &hash));
    }

    #[test]
    fn test_password_hasher_different_salts() {
        let hasher = PasswordHasher::new();
        let password = "SecureP@ss123";
        let hash1 = hasher.hash(password).unwrap();
        let hash2 = hasher.hash(password).unwrap();

        // Same password should produce different hashes (different salts)
        assert_ne!(hash1, hash2);

        // Both should verify
        assert!(hasher.verify(password, &hash1));
        assert!(hasher.verify(password, &hash2));
    }

    #[test]
    fn test_jwt_service() {
        let config = AuthConfig {
            jwt_secret: Some("test-secret-key-at-least-32-characters-long".to_string()),
            access_token_expiry_minutes: 15,
            refresh_token_expiry_days: 7,
        };

        let service = JwtService::new(&config).unwrap();

        let (token, expires_in) = service
            .generate_access_token("user_123", "test@example.com", "agent", Some("team_abc"))
            .unwrap();

        assert!(!token.is_empty());
        assert_eq!(expires_in, 900); // 15 minutes in seconds

        // Validate token
        let claims = service.validate_access_token(&token).unwrap();
        assert_eq!(claims.sub, "user_123");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.role, "agent");
    }

    #[test]
    fn test_jwt_service_invalid_token() {
        let config = AuthConfig {
            jwt_secret: Some("test-secret-key-at-least-32-characters-long".to_string()),
            access_token_expiry_minutes: 15,
            refresh_token_expiry_days: 7,
        };

        let service = JwtService::new(&config).unwrap();

        // Invalid token should fail
        let result = service.validate_access_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_jwt_service_config_validation() {
        // Missing secret
        let config = AuthConfig {
            jwt_secret: None,
            access_token_expiry_minutes: 15,
            refresh_token_expiry_days: 7,
        };
        assert!(JwtService::new(&config).is_err());

        // Short secret
        let config = AuthConfig {
            jwt_secret: Some("too-short".to_string()),
            access_token_expiry_minutes: 15,
            refresh_token_expiry_days: 7,
        };
        assert!(JwtService::new(&config).is_err());
    }

    // ========================================
    // Session Management Unit Tests (Story 14-4)
    // ========================================

    #[test]
    fn test_session_info_excludes_token_hash() {
        use crate::models::session::{Session, SessionInfo};

        let session = Session {
            id: "rt_test123".to_string(),
            user_id: "user_abc".to_string(),
            token_hash: "super_secret_hash".to_string(),
            expires_at: "2026-03-20T10:30:00Z".to_string(),
            revoked: 0,
            created_at: "2026-03-13T10:30:00Z".to_string(),
            last_used_at: None,
            user_agent: None,
            ip_address: None,
        };

        let info: SessionInfo = session.into();

        // Verify token_hash is not exposed
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("token_hash"));
        assert!(!json.contains("super_secret"));
        assert!(json.contains("rt_test123"));
    }

    #[test]
    fn test_session_list_response_structure() {
        use crate::models::session::{SessionInfo, SessionListResponse};

        let response = SessionListResponse {
            sessions: vec![
                SessionInfo {
                    id: "rt_1".to_string(),
                    created_at: "2026-03-13T10:00:00Z".to_string(),
                    expires_at: "2026-03-20T10:00:00Z".to_string(),
                    last_used_at: Some("2026-03-14T12:00:00Z".to_string()),
                    user_agent: Some("Mozilla/5.0".to_string()),
                    ip_address: Some("192.168.1.1".to_string()),
                },
                SessionInfo {
                    id: "rt_2".to_string(),
                    created_at: "2026-03-12T10:00:00Z".to_string(),
                    expires_at: "2026-03-19T10:00:00Z".to_string(),
                    last_used_at: None,
                    user_agent: None,
                    ip_address: None,
                },
            ],
            total: 2,
        };

        assert_eq!(response.sessions.len(), 2);
        assert_eq!(response.total, 2);
    }

    #[test]
    fn test_revoke_session_response() {
        use crate::models::session::SessionRevokeResponse;

        let response = SessionRevokeResponse {
            message: "Session revoked successfully".to_string(),
            session_id: "rt_abc123".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("Session revoked successfully"));
        assert!(json.contains("rt_abc123"));
    }

    #[test]
    fn test_revoke_all_sessions_response() {
        use crate::models::session::RevokeAllSessionsResponse;

        let response = RevokeAllSessionsResponse {
            message: "All other sessions revoked".to_string(),
            revoked_count: 5,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("All other sessions revoked"));
        assert!(json.contains("5"));
    }
}
