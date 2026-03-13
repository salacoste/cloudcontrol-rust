//! Authentication routes (Story 14-1)
//!
//! Handles user registration, login, token refresh, and logout.

use actix_web::{web, HttpRequest, HttpResponse};
use validator::Validate;

use crate::models::user::{LoginRequest, LogoutRequest, RefreshRequest, RegisterRequest, UserInfo};
use crate::services::auth_service::AuthError;
use crate::state::AppState;

/// Extract client IP from request for audit logging.
fn extract_client_ip(req: &HttpRequest) -> String {
    if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
        if let Ok(val) = forwarded.to_str() {
            if let Some(first_ip) = val.split(',').next() {
                let ip = first_ip.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }
    req.peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Map AuthError to HTTP response with appropriate error codes.
fn auth_error_to_response(error: AuthError) -> HttpResponse {
    match error {
        AuthError::InvalidCredentials => HttpResponse::Unauthorized().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-101",
            "message": "Invalid credentials"
        })),
        AuthError::TokenExpired => HttpResponse::Unauthorized().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-102",
            "message": "Token expired"
        })),
        AuthError::TokenRevoked => HttpResponse::Unauthorized().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-103",
            "message": "Token revoked"
        })),
        AuthError::TokenInvalid => HttpResponse::Unauthorized().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-103",
            "message": "Invalid token"
        })),
        AuthError::EmailAlreadyExists => HttpResponse::Conflict().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-105",
            "message": "Email already registered"
        })),
        AuthError::PasswordTooWeak(errors) => HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-106",
            "message": "Password requirements not met",
            "details": errors
        })),
        AuthError::UserNotFound => HttpResponse::NotFound().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-107",
            "message": "User not found"
        })),
        AuthError::DatabaseError(msg) => {
            tracing::error!("Database error: {}", msg);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "error": "CC-AUTH-500",
                "message": "Internal server error"
            }))
        }
        AuthError::ConfigError(msg) => {
            tracing::error!("Configuration error: {}", msg);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "error": "CC-AUTH-500",
                "message": "Internal server error"
            }))
        }
        AuthError::RateLimited => HttpResponse::TooManyRequests().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-108",
            "message": "Too many requests"
        })),
    }
}

/// Register a new user.
///
/// POST /api/v1/auth/register
pub async fn register(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<RegisterRequest>,
) -> HttpResponse {
    // Validate input
    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-106",
            "message": "Validation failed",
            "details": e.to_string()
        }));
    }

    // Get auth service or return error if auth not configured
    let auth_service = match &state.auth_service {
        Some(service) => service,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "error": "CC-AUTH-500",
                "message": "Authentication not configured"
            }));
        }
    };

    let ip = extract_client_ip(&req);
    tracing::info!("Registration attempt from {} for email: {}", ip, body.email);

    match auth_service.register(&body.email, &body.password).await {
        Ok(response) => {
            tracing::info!("User registered successfully: {}", response.email);
            HttpResponse::Created().json(serde_json::json!({
                "status": "success",
                "data": response
            }))
        }
        Err(e) => {
            tracing::warn!("Registration failed for {}: {}", body.email, e);
            auth_error_to_response(e)
        }
    }
}

/// Login with email and password.
///
/// POST /api/v1/auth/login
pub async fn login(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<LoginRequest>,
) -> HttpResponse {
    // Validate input
    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-106",
            "message": "Validation failed",
            "details": e.to_string()
        }));
    }

    // Get auth service or return error if auth not configured
    let auth_service = match &state.auth_service {
        Some(service) => service,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "error": "CC-AUTH-500",
                "message": "Authentication not configured"
            }));
        }
    };

    let ip = extract_client_ip(&req);
    tracing::info!("Login attempt from {} for email: {}", ip, body.email);

    match auth_service.login(&body.email, &body.password).await {
        Ok(response) => {
            tracing::info!("User logged in successfully: {}", body.email);
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "data": response
            }))
        }
        Err(e) => {
            tracing::warn!("Login failed for {}: {}", body.email, e);
            auth_error_to_response(e)
        }
    }
}

/// Refresh access token.
///
/// POST /api/v1/auth/refresh
pub async fn refresh(
    state: web::Data<AppState>,
    body: web::Json<RefreshRequest>,
) -> HttpResponse {
    // Get auth service or return error if auth not configured
    let auth_service = match &state.auth_service {
        Some(service) => service,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "error": "CC-AUTH-500",
                "message": "Authentication not configured"
            }));
        }
    };

    match auth_service.refresh(&body.refresh_token).await {
        Ok(response) => {
            tracing::info!("Token refreshed successfully");
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "data": response
            }))
        }
        Err(e) => {
            tracing::warn!("Token refresh failed: {}", e);
            auth_error_to_response(e)
        }
    }
}

/// Logout (revoke refresh token).
///
/// POST /api/v1/auth/logout
pub async fn logout(
    state: web::Data<AppState>,
    body: web::Json<LogoutRequest>,
) -> HttpResponse {
    // Get auth service or return error if auth not configured
    let auth_service = match &state.auth_service {
        Some(service) => service,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "error": "CC-AUTH-500",
                "message": "Authentication not configured"
            }));
        }
    };

    match auth_service.logout(&body.refresh_token).await {
        Ok(()) => {
            tracing::info!("User logged out successfully");
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": "Logged out successfully"
            }))
        }
        Err(e) => {
            tracing::warn!("Logout failed: {}", e);
            auth_error_to_response(e)
        }
    }
}

/// Logout all sessions for current user.
///
/// POST /api/v1/auth/logout-all
pub async fn logout_all(
    state: web::Data<AppState>,
    user: web::ReqData<UserInfo>,
) -> HttpResponse {
    // Get auth service or return error if auth not configured
    let auth_service = match &state.auth_service {
        Some(service) => service,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "error": "CC-AUTH-500",
                "message": "Authentication not configured"
            }));
        }
    };

    match auth_service.logout_all(&user.id).await {
        Ok(count) => {
            tracing::info!("Logged out all sessions for user {}: {} tokens revoked", user.id, count);
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": format!("Logged out {} sessions", count)
            }))
        }
        Err(e) => {
            tracing::warn!("Logout all failed: {}", e);
            auth_error_to_response(e)
        }
    }
}

/// Get current user info.
///
/// GET /api/v1/auth/me
pub async fn get_me(user: web::ReqData<UserInfo>) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "data": user.into_inner()
    }))
}

/// Check if authentication is enabled.
///
/// GET /api/v1/auth/status
pub async fn auth_status(state: web::Data<AppState>) -> HttpResponse {
    let auth_enabled = state.auth_service.is_some();

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "data": {
            "auth_enabled": auth_enabled
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_error_invalid_credentials() {
        let resp = auth_error_to_response(AuthError::InvalidCredentials);
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_token_expired() {
        let resp = auth_error_to_response(AuthError::TokenExpired);
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_email_exists() {
        let resp = auth_error_to_response(AuthError::EmailAlreadyExists);
        assert_eq!(resp.status(), actix_web::http::StatusCode::CONFLICT);
    }

    #[test]
    fn test_auth_error_password_weak() {
        let resp = auth_error_to_response(AuthError::PasswordTooWeak(vec![
            "Too short".to_string(),
        ]));
        assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
    }
}
