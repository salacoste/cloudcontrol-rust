use actix_web::middleware::ErrorHandlerResponse;
use actix_web::{dev, web, HttpResponse, Result};
use serde_json::json;
use thiserror::Error;

// ═════════════════════════════════════════════════════════════════════════════
// Error Page Handlers (for middleware)
// ═════════════════════════════════════════════════════════════════════════════

/// Render the 404 page using Tera templates.
pub fn handle_404<B>(
    res: dev::ServiceResponse<B>,
) -> Result<ErrorHandlerResponse<B>> {
    let request = res.request().clone();
    let tera = request
        .app_data::<web::Data<tera::Tera>>()
        .cloned();

    let body = if let Some(tera) = tera {
        let ctx = tera::Context::new();
        tera.render("404.html", &ctx).unwrap_or_else(|_| {
            "<h1>404 Not Found</h1>".to_string()
        })
    } else {
        "<h1>404 Not Found</h1>".to_string()
    };

    let new_response = HttpResponse::NotFound()
        .content_type("text/html; charset=utf-8")
        .body(body);

    Ok(ErrorHandlerResponse::Response(
        res.into_response(new_response).map_into_right_body(),
    ))
}

/// Render the 500 page using Tera templates.
pub fn handle_500<B>(
    res: dev::ServiceResponse<B>,
) -> Result<ErrorHandlerResponse<B>> {
    let request = res.request().clone();
    let tera = request
        .app_data::<web::Data<tera::Tera>>()
        .cloned();

    let body = if let Some(tera) = tera {
        let ctx = tera::Context::new();
        tera.render("500.html", &ctx).unwrap_or_else(|_| {
            "<h1>500 Internal Server Error</h1>".to_string()
        })
    } else {
        "<h1>500 Internal Server Error</h1>".to_string()
    };

    let new_response = HttpResponse::InternalServerError()
        .content_type("text/html; charset=utf-8")
        .body(body);

    Ok(ErrorHandlerResponse::Response(
        res.into_response(new_response).map_into_right_body(),
    ))
}

// ═════════════════════════════════════════════════════════════════════════════
// Application Error Types (Story 13.2)
// ═════════════════════════════════════════════════════════════════════════════

/// Application-wide error enum with thiserror derive.
///
/// Provides type-safe error handling with automatic HTTP response conversion.
/// Created as part of Story 13.2: Error Handling Modernization.
#[derive(Debug, Error)]
pub enum AppError {
    /// Device not found in database or cache
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Device is disconnected or unreachable
    #[error("Device disconnected: {0}")]
    DeviceDisconnected(String),

    /// Database operation failed
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// JSON serialization/deserialization failed
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Recording operation failed
    #[error("Recording error: {0}")]
    RecordingError(String),

    /// Regex pattern compilation failed
    #[error("Invalid regex pattern: {0}")]
    RegexError(String),
}

impl AppError {
    /// Get the error code for API responses
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::DeviceNotFound(_) => "ERR_DEVICE_NOT_FOUND",
            AppError::DeviceDisconnected(_) => "ERR_DEVICE_DISCONNECTED",
            AppError::DatabaseError(_) => "ERR_DATABASE_ERROR",
            AppError::SerializationError(_) => "ERR_SERIALIZATION_ERROR",
            AppError::InvalidRequest(_) => "ERR_INVALID_REQUEST",
            AppError::RecordingError(_) => "ERR_RECORDING_ERROR",
            AppError::RegexError(_) => "ERR_REGEX_ERROR",
        }
    }

    /// Check if error indicates device not found
    pub fn is_not_found(&self) -> bool {
        matches!(self, AppError::DeviceNotFound(_))
    }

    /// Check if error indicates device disconnected
    pub fn is_disconnected(&self) -> bool {
        matches!(self, AppError::DeviceDisconnected(_))
    }
}

impl From<AppError> for HttpResponse {
    fn from(err: AppError) -> HttpResponse {
        match &err {
            AppError::DeviceNotFound(msg) => HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_DEVICE_NOT_FOUND",
                "message": msg
            })),
            AppError::DeviceDisconnected(msg) => HttpResponse::ServiceUnavailable().json(json!({
                "status": "error",
                "error": "ERR_DEVICE_DISCONNECTED",
                "message": msg
            })),
            AppError::DatabaseError(msg) => HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_DATABASE_ERROR",
                "message": msg
            })),
            AppError::SerializationError(e) => HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_SERIALIZATION_ERROR",
                "message": e.to_string()
            })),
            AppError::InvalidRequest(msg) => HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": msg
            })),
            AppError::RecordingError(msg) => HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_RECORDING_ERROR",
                "message": msg
            })),
            AppError::RegexError(msg) => HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_REGEX_ERROR",
                "message": msg
            })),
        }
    }
}

/// Helper trait for converting string errors to AppError
pub trait IntoAppError {
    /// Convert to AppError with context
    fn into_app_error(self, context: &str) -> AppError;
}

impl IntoAppError for String {
    fn into_app_error(self, context: &str) -> AppError {
        let msg = format!("{}: {}", context, self);
        if self.contains("not found") {
            AppError::DeviceNotFound(msg)
        } else if self.contains("disconnected") || self.contains("unreachable") {
            AppError::DeviceDisconnected(msg)
        } else {
            AppError::DatabaseError(msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(
            AppError::DeviceNotFound("test".into()).error_code(),
            "ERR_DEVICE_NOT_FOUND"
        );
        assert_eq!(
            AppError::DeviceDisconnected("test".into()).error_code(),
            "ERR_DEVICE_DISCONNECTED"
        );
        assert_eq!(
            AppError::DatabaseError("test".into()).error_code(),
            "ERR_DATABASE_ERROR"
        );
        assert_eq!(
            AppError::InvalidRequest("test".into()).error_code(),
            "ERR_INVALID_REQUEST"
        );
    }

    #[test]
    fn test_is_not_found() {
        assert!(AppError::DeviceNotFound("test".into()).is_not_found());
        assert!(!AppError::DeviceDisconnected("test".into()).is_not_found());
    }

    #[test]
    fn test_is_disconnected() {
        assert!(AppError::DeviceDisconnected("test".into()).is_disconnected());
        assert!(!AppError::DeviceNotFound("test".into()).is_disconnected());
    }

    #[test]
    fn test_into_app_error() {
        let err = "Device not found: abc".to_string().into_app_error("query");
        assert!(err.is_not_found());

        let err = "Device disconnected: xyz".to_string().into_app_error("query");
        assert!(err.is_disconnected());

        let err = "Some other error".to_string().into_app_error("query");
        assert!(matches!(err, AppError::DatabaseError(_)));
    }
}
