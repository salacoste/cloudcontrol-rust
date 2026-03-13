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
use crate::models::user::{
    generate_refresh_token, generate_refresh_token_id, generate_user_id, validate_password,
    AccessTokenClaims, AuthResponse, RefreshResponse, RefreshToken, RegisterResponse, UserInfo,
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
    jwt_secret: String,
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
    ) -> Result<(String, u64), AuthError> {
        let now = Utc::now();
        let exp = now + self.access_token_expiry;

        let claims = AccessTokenClaims {
            sub: user_id.to_string(),
            email: email.to_string(),
            role: role.to_string(),
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
    ) -> Result<AuthResponse, AuthError> {
        // Find user
        let user: Option<(String, String, String)> = sqlx::query_as(
            "SELECT id, password_hash, role FROM users WHERE email = ?",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        let (user_id, password_hash, role) = match user {
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
                .generate_access_token(&user_id, email, &role)?;

        let refresh_token = generate_refresh_token();
        let refresh_token_hash = self.hash_refresh_token(&refresh_token);
        let refresh_token_id = generate_refresh_token_id();
        let now = Utc::now();
        let expires_at = now + self.jwt_service.refresh_token_expiry();

        // Store refresh token
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&refresh_token_id)
        .bind(&user_id)
        .bind(&refresh_token_hash)
        .bind(expires_at.to_rfc3339())
        .bind(now.to_rfc3339())
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

        // Get user info
        let user: Option<(String, String)> = sqlx::query_as(
            "SELECT email, role FROM users WHERE id = ?",
        )
        .bind(&token.user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        let (email, role) = match user {
            Some(u) => u,
            None => return Err(AuthError::UserNotFound),
        };

        // Revoke old token
        sqlx::query("UPDATE refresh_tokens SET revoked = 1 WHERE id = ?")
            .bind(&token.id)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        // Generate new tokens
        let (access_token, expires_in) = self.jwt_service.generate_access_token(
            &token.user_id,
            &email,
            &role,
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

    /// Get user by ID
    pub async fn get_user(&self, user_id: &str) -> Result<Option<UserInfo>, AuthError> {
        let user: Option<(String, String, String)> = sqlx::query_as(
            "SELECT id, email, role FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        Ok(user.map(|(id, email, role)| UserInfo { id, email, role }))
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
            .generate_access_token("user_123", "test@example.com", "agent")
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
}
