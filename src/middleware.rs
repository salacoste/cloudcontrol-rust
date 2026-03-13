use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, Payload};
use actix_web::http::Method;
use actix_web::{Error, HttpResponse, HttpMessage, HttpRequest, FromRequest};
use actix_web::error::ErrorForbidden;
use crate::config::RateLimitConfig;
use dashmap::DashMap;
use futures::future::{ok, Either, Ready, ready};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

/// API key authentication middleware (Story 12-1).
///
/// When an API key is configured, all non-exempt requests must include
/// the key via `Authorization: Bearer <key>` header or `?api_key=<key>`
/// query parameter. If no key is configured, all requests pass through.
pub struct ApiKeyAuth {
    api_key: Option<String>,
}

impl ApiKeyAuth {
    pub fn new(api_key: Option<String>) -> Self {
        let api_key = api_key.filter(|k| !k.is_empty());
        Self { api_key }
    }
}

impl<S, B> Transform<S, ServiceRequest> for ApiKeyAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = ApiKeyAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ApiKeyAuthMiddleware {
            service,
            api_key: self.api_key.clone(),
        })
    }
}

pub struct ApiKeyAuthMiddleware<S> {
    service: S,
    api_key: Option<String>,
}

impl<S, B> Service<ServiceRequest> for ApiKeyAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Either<
        futures::future::Map<S::Future, fn(Result<ServiceResponse<B>, Error>) -> Result<ServiceResponse<EitherBody<B>>, Error>>,
        Ready<Result<ServiceResponse<EitherBody<B>>, Error>>,
    >;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let expected_key = match &self.api_key {
            Some(key) => key,
            None => {
                // Auth disabled — pass through
                return Either::Left(futures::future::FutureExt::map(
                    self.service.call(req),
                    |res| res.map(|r| r.map_into_left_body()),
                ));
            }
        };

        if is_exempt(req.path(), req.method()) {
            return Either::Left(futures::future::FutureExt::map(
                self.service.call(req),
                |res| res.map(|r| r.map_into_left_body()),
            ));
        }

        // Same-origin browser requests are exempt (Sec-Fetch-Site header)
        if is_same_origin(&req) {
            tracing::debug!("Auth bypassed via same-origin header: {}", req.path());
            return Either::Left(futures::future::FutureExt::map(
                self.service.call(req),
                |res| res.map(|r| r.map_into_left_body()),
            ));
        }

        // Extract API key from request
        let provided_key = extract_bearer_token(&req)
            .or_else(|| extract_query_api_key(&req));

        let authorized = match provided_key {
            Some(key) => constant_time_eq(key.as_bytes(), expected_key.as_bytes()),
            None => false,
        };

        if authorized {
            Either::Left(futures::future::FutureExt::map(
                self.service.call(req),
                |res| res.map(|r| r.map_into_left_body()),
            ))
        } else {
            let response = HttpResponse::Unauthorized()
                .json(serde_json::json!({
                    "status": "error",
                    "error": "ERR_UNAUTHORIZED",
                    "message": "Valid API key required. Provide via 'Authorization: Bearer <key>' header or '?api_key=<key>' query parameter.",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            Either::Right(ok(req.into_response(response).map_into_right_body()))
        }
    }
}

/// Check if a request path is exempt from authentication.
fn is_exempt(path: &str, method: &Method) -> bool {
    // Static files
    if path.starts_with("/static/") {
        return true;
    }

    // Health + OpenAPI (monitoring/discovery)
    if path == "/api/v1/health" || path == "/api/v1/openapi.json" {
        return true;
    }

    // Page routes (GET only — these serve HTML)
    if *method == Method::GET {
        if path == "/"
            || path == "/async"
            || path == "/installfile"
            || path == "/test"
            || path == "/files"
            || path == "/providers"
            || path == "/atxagent"
        {
            return true;
        }
        // Dynamic page routes: /devices/{udid}/remote, /devices/{udid}/edit, /devices/{udid}/property
        if path.starts_with("/devices/")
            && (path.ends_with("/remote")
                || path.ends_with("/edit")
                || path.ends_with("/property")
                || path.ends_with("/reserved"))
        {
            return true;
        }
    }

    false
}

/// Check if request originates from the same origin (browser AJAX/WebSocket).
/// Modern browsers set `Sec-Fetch-Site: same-origin` for same-origin requests.
/// This allows frontend pages to make API calls without needing the API key.
fn is_same_origin(req: &ServiceRequest) -> bool {
    req.headers()
        .get("Sec-Fetch-Site")
        .and_then(|v| v.to_str().ok())
        .map_or(false, |v| v == "same-origin" || v == "same-site")
}

/// Extract Bearer token from Authorization header.
/// RFC 6750: scheme matching is case-insensitive.
fn extract_bearer_token(req: &ServiceRequest) -> Option<String> {
    let auth_header = req.headers().get("Authorization")?;
    let auth_str = auth_header.to_str().ok()?;
    // Case-insensitive "Bearer " prefix per RFC 6750
    if auth_str.len() < 7 {
        return None;
    }
    if auth_str[..7].eq_ignore_ascii_case("bearer ") {
        Some(auth_str[7..].trim().to_string())
    } else {
        None
    }
}

/// Extract api_key from query parameters.
fn extract_query_api_key(req: &ServiceRequest) -> Option<String> {
    let query = req.query_string();
    query
        .split('&')
        .find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            if k == "api_key" {
                Some(urlencoding::decode(v).ok()?.into_owned())
            } else {
                None
            }
        })
}

/// Constant-time byte comparison to prevent timing attacks.
/// Does NOT early-return on length mismatch to avoid leaking key length.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut result = (a.len() ^ b.len()) as u8;
    for i in 0..a.len().min(b.len()) {
        result |= a[i] ^ b[i];
    }
    result == 0
}

// ═══════════════ RATE LIMITING (Story 12-2) ═══════════════

/// Result of a rate limit check.
#[derive(Debug)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset_secs: u64,
}

/// Sliding-window rate limiter using DashMap for per-IP tracking.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    buckets: Arc<DashMap<String, Vec<Instant>>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Check rate limit for an IP and optional endpoint category.
    pub fn check_rate_limit(&self, ip: &str, category: Option<&str>) -> RateLimitResult {
        let limit = category
            .and_then(|c| self.config.category_limits.get(c))
            .copied()
            .unwrap_or(self.config.requests_per_window);
        let window = Duration::from_secs(self.config.window_secs);
        let now = Instant::now();

        let mut entry = self.buckets.entry(ip.to_string()).or_default();
        // Remove expired timestamps (lazy cleanup)
        entry.retain(|t| now.duration_since(*t) < window);

        if entry.len() as u32 >= limit {
            let oldest = entry.first().copied();
            let reset_secs = oldest
                .map(|t| window.saturating_sub(now.duration_since(t)).as_secs())
                .unwrap_or(self.config.window_secs);
            RateLimitResult { allowed: false, limit, remaining: 0, reset_secs }
        } else {
            entry.push(now);
            let remaining = limit.saturating_sub(entry.len() as u32);
            RateLimitResult { allowed: true, limit, remaining, reset_secs: self.config.window_secs }
        }
    }
}

/// Categorize an endpoint path for rate limit category overrides.
/// Order matters: batch is checked before screenshot because
/// `/api/screenshot/batch` is a batch operation, not a single screenshot.
/// Auth endpoints are checked early for stricter rate limiting (Story 14-1).
pub fn categorize_endpoint(path: &str) -> Option<&'static str> {
    // Auth endpoints - stricter rate limiting (Story 14-1)
    if path.starts_with("/api/v1/auth/login") || path.starts_with("/api/v1/auth/register") {
        return Some("auth");
    }
    // Batch must be checked first — /api/screenshot/batch is a batch operation
    if path.starts_with("/api/batch/") || path.starts_with("/api/v1/batch/") || path == "/api/screenshot/batch" {
        return Some("batch");
    }
    if path.contains("/screenshot") {
        return Some("screenshot");
    }
    if path.contains("/touch") || path.contains("/input") || path.contains("/keyevent") || path.contains("/swipe") {
        return Some("control");
    }
    if path.ends_with("/ws") || path == "/video/convert" {
        return Some("websocket");
    }
    None
}

/// Extract client IP from request, checking X-Forwarded-For first.
///
/// # Security Note
/// `X-Forwarded-For` is trivially spoofable by clients. This function is only
/// safe when the server runs behind a trusted reverse proxy that overwrites
/// the header. Without a proxy, an attacker can rotate the header value to
/// get a fresh rate limit bucket per request, bypassing rate limiting entirely.
fn extract_client_ip(req: &ServiceRequest) -> String {
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

/// Rate limiting middleware (Story 12-2).
pub struct RateLimit {
    limiter: Option<Arc<RateLimiter>>,
}

impl RateLimit {
    pub fn new(limiter: Option<Arc<RateLimiter>>) -> Self {
        Self { limiter }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RateLimitMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimitMiddleware {
            service,
            limiter: self.limiter.clone(),
        })
    }
}

pub struct RateLimitMiddleware<S> {
    service: S,
    limiter: Option<Arc<RateLimiter>>,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Either<
        futures::future::Map<S::Future, fn(Result<ServiceResponse<B>, Error>) -> Result<ServiceResponse<EitherBody<B>>, Error>>,
        Ready<Result<ServiceResponse<EitherBody<B>>, Error>>,
    >;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let limiter = match &self.limiter {
            Some(l) => l,
            None => {
                // Rate limiting disabled — pass through
                return Either::Left(futures::future::FutureExt::map(
                    self.service.call(req),
                    |res| res.map(|r| r.map_into_left_body()),
                ));
            }
        };

        // Exempt same paths as auth middleware
        if is_exempt(req.path(), req.method()) {
            return Either::Left(futures::future::FutureExt::map(
                self.service.call(req),
                |res| res.map(|r| r.map_into_left_body()),
            ));
        }

        let ip = extract_client_ip(&req);
        let category = categorize_endpoint(req.path());
        let result = limiter.check_rate_limit(&ip, category);

        if result.allowed {
            // Pass through — rate limit headers added would require async response wrapping,
            // but actix-web's Map type doesn't let us modify headers on the inner response
            // without Pin/Box. For simplicity, we only add headers on 429 responses.
            Either::Left(futures::future::FutureExt::map(
                self.service.call(req),
                |res| res.map(|r| r.map_into_left_body()),
            ))
        } else {
            tracing::debug!("Rate limited IP {} on {}", ip, req.path());
            // Use CC-SYS-902 for auth endpoints (Story 14-1), ERR_RATE_LIMITED for others
            let (error_code, message): (&str, String) = match category {
                Some("auth") => ("CC-SYS-902", "Too many authentication attempts. Please try again later.".to_string()),
                _ => ("ERR_RATE_LIMITED", format!("Rate limit exceeded. Try again in {} seconds.", result.reset_secs)),
            };
            let response = HttpResponse::TooManyRequests()
                .insert_header(("Retry-After", result.reset_secs.to_string()))
                .insert_header(("X-RateLimit-Limit", result.limit.to_string()))
                .insert_header(("X-RateLimit-Remaining", "0"))
                .insert_header(("X-RateLimit-Reset", result.reset_secs.to_string()))
                .json(serde_json::json!({
                    "status": "error",
                    "error": error_code,
                    "message": message,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            Either::Right(ok(req.into_response(response).map_into_right_body()))
        }
    }
}

// ═══════════════ RBAC EXTRACTORS (Story 14-2) ═══════════════

use std::str::FromStr;
use crate::models::user::{UserRole, Permission};

/// Extractor that requires the user to have the Admin role.
pub struct RequireAdmin {
    pub user: UserInfo,
}

impl FromRequest for RequireAdmin {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let user = req.extensions()
            .get::<UserInfo>()
            .cloned();

        match user {
            Some(user) if user.role == "admin" => {
                ready(Ok(RequireAdmin { user }))
            }
            Some(_) => {
                ready(Err(ErrorForbidden(serde_json::json!({
                    "status": "error",
                    "error": "CC-AUTH-104",
                    "message": "Insufficient permissions",
                    "details": "This action requires admin role"
                }))))
            }
            None => {
                ready(Err(ErrorForbidden(serde_json::json!({
                    "status": "error",
                    "error": "CC-AUTH-104",
                    "message": "Insufficient permissions",
                    "details": "Authentication required"
                }))))
            }
        }
    }
}

/// Extractor that requires any non-viewer role (Admin, Agent, or Renter).
pub struct RequireAnyRole {
    pub user: UserInfo,
}

impl FromRequest for RequireAnyRole {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let user = req.extensions()
            .get::<UserInfo>()
            .cloned();

        match user {
            Some(user) => {
                let role = UserRole::from_str(&user.role);
                match role {
                    Ok(UserRole::Viewer) => {
                        ready(Err(ErrorForbidden(serde_json::json!({
                            "status": "error",
                            "error": "CC-AUTH-104",
                            "message": "Insufficient permissions",
                            "details": "Viewer role has read-only access"
                        }))))
                    }
                    Ok(_) => ready(Ok(RequireAnyRole { user })),
                    Err(_) => {
                        ready(Err(ErrorForbidden(serde_json::json!({
                            "status": "error",
                            "error": "CC-AUTH-104",
                            "message": "Insufficient permissions",
                            "details": "Invalid user role"
                        }))))
                    }
                }
            }
            None => {
                ready(Err(ErrorForbidden(serde_json::json!({
                    "status": "error",
                    "error": "CC-AUTH-104",
                    "message": "Insufficient permissions",
                    "details": "Authentication required"
                }))))
            }
        }
    }
}

/// Optional authentication extractor (Story 14-3).
/// Returns Some(user) if authenticated, None if not.
/// Allows endpoints to work with or without authentication.
pub struct OptionalAuth {
    pub user: Option<UserInfo>,
}

impl FromRequest for OptionalAuth {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let user = req.extensions().get::<UserInfo>().cloned();
        ready(Ok(OptionalAuth { user }))
    }
}

/// Check if the authenticated user has a specific permission.
/// Returns the user info if authorized, or an error if not.
pub fn check_permission(req: &HttpRequest, permission: Permission) -> Result<UserInfo, Error> {
    let user = req.extensions()
        .get::<UserInfo>()
        .cloned()
        .ok_or_else(|| ErrorForbidden(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-104",
            "message": "Insufficient permissions",
            "details": "Authentication required"
        })))?;

    let role = UserRole::from_str(&user.role)
        .map_err(|_| ErrorForbidden(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-104",
            "message": "Insufficient permissions",
            "details": "Invalid user role"
        })))?;

    if role.has_permission(permission) {
        Ok(user)
    } else {
        Err(ErrorForbidden(serde_json::json!({
            "status": "error",
            "error": "CC-AUTH-104",
            "message": "Insufficient permissions",
            "details": format!("This action requires {:?} permission", permission)
        })))
    }
}

/// Get the authenticated user's role from request extensions, if present.
pub fn get_user_role(req: &HttpRequest) -> Option<UserRole> {
    req.extensions()
        .get::<UserInfo>()
        .and_then(|u| UserRole::from_str(&u.role).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exempt_page_routes() {
        assert!(is_exempt("/", &Method::GET));
        assert!(is_exempt("/async", &Method::GET));
        assert!(is_exempt("/installfile", &Method::GET));
        assert!(is_exempt("/test", &Method::GET));
        assert!(is_exempt("/files", &Method::GET));
        assert!(is_exempt("/providers", &Method::GET));
    }

    #[test]
    fn test_exempt_dynamic_page_routes() {
        assert!(is_exempt("/devices/abc123/remote", &Method::GET));
        assert!(is_exempt("/devices/abc123/edit", &Method::GET));
        assert!(is_exempt("/devices/abc123/property", &Method::GET));
    }

    #[test]
    fn test_exempt_static_files() {
        assert!(is_exempt("/static/js/remote.js", &Method::GET));
        assert!(is_exempt("/static/css/style.css", &Method::GET));
    }

    #[test]
    fn test_exempt_monitoring() {
        assert!(is_exempt("/api/v1/health", &Method::GET));
        assert!(is_exempt("/api/v1/openapi.json", &Method::GET));
    }

    #[test]
    fn test_non_exempt_api_routes() {
        assert!(!is_exempt("/api/v1/devices", &Method::GET));
        assert!(!is_exempt("/api/v1/devices/abc/tap", &Method::POST));
        assert!(!is_exempt("/inspector/abc/screenshot", &Method::GET));
        assert!(!is_exempt("/scrcpy/abc/ws", &Method::GET));
        assert!(!is_exempt("/list", &Method::GET));
        assert!(!is_exempt("/heartbeat", &Method::POST));
        assert!(!is_exempt("/nio/abc/ws", &Method::GET));
        assert!(!is_exempt("/video/convert", &Method::GET));
    }

    #[test]
    fn test_page_routes_not_exempt_for_post() {
        assert!(!is_exempt("/", &Method::POST));
        assert!(!is_exempt("/async", &Method::POST));
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"wrong"));
        assert!(!constant_time_eq(b"short", b"longer"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn test_async_page_post_exempt() {
        // POST /async is the batch control page submission — needs auth
        assert!(!is_exempt("/async", &Method::POST));
    }

    #[test]
    fn test_api_key_auth_disabled_when_no_key() {
        let auth = ApiKeyAuth::new(None);
        assert!(auth.api_key.is_none());
    }

    #[test]
    fn test_api_key_auth_disabled_when_empty_key() {
        let auth = ApiKeyAuth::new(Some(String::new()));
        assert!(auth.api_key.is_none());
    }

    #[test]
    fn test_api_key_auth_enabled_with_key() {
        let auth = ApiKeyAuth::new(Some("my-secret".to_string()));
        assert_eq!(auth.api_key, Some("my-secret".to_string()));
    }

    #[test]
    fn test_exempt_reserved_page() {
        assert!(is_exempt("/devices/abc123/reserved", &Method::GET));
        assert!(!is_exempt("/devices/abc123/reserved", &Method::POST));
    }

    #[test]
    fn test_exempt_atxagent_page() {
        assert!(is_exempt("/atxagent", &Method::GET));
        assert!(!is_exempt("/atxagent", &Method::POST));
    }

    #[test]
    fn test_non_exempt_device_api_paths() {
        // /devices/{udid}/info is an API route, not a page route
        assert!(!is_exempt("/devices/abc/info", &Method::GET));
        // /devices without subpath is not a recognized page
        assert!(!is_exempt("/devices", &Method::GET));
    }

    #[test]
    fn test_constant_time_eq_same_length_different() {
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"xbc"));
    }

    #[test]
    fn test_api_key_auth_whitespace_only_key() {
        let auth = ApiKeyAuth::new(Some("  ".to_string()));
        // Whitespace-only key is treated as a valid key (not empty)
        assert!(auth.api_key.is_some());
    }

    // ═══════════════ RATE LIMITER TESTS (Story 12-2) ═══════════════

    fn make_test_rate_config(limit: u32, window_secs: u64) -> RateLimitConfig {
        RateLimitConfig {
            requests_per_window: limit,
            window_secs,
            category_limits: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_rate_limiter_allows_under_limit() {
        let limiter = RateLimiter::new(make_test_rate_config(5, 60));
        let result = limiter.check_rate_limit("1.2.3.4", None);
        assert!(result.allowed);
        assert_eq!(result.limit, 5);
        assert_eq!(result.remaining, 4);
    }

    #[test]
    fn test_rate_limiter_denies_over_limit() {
        let limiter = RateLimiter::new(make_test_rate_config(3, 60));
        for _ in 0..3 {
            let r = limiter.check_rate_limit("1.2.3.4", None);
            assert!(r.allowed);
        }
        let result = limiter.check_rate_limit("1.2.3.4", None);
        assert!(!result.allowed);
        assert_eq!(result.remaining, 0);
        assert!(result.reset_secs > 0);
    }

    #[test]
    fn test_rate_limiter_per_ip_isolation() {
        let limiter = RateLimiter::new(make_test_rate_config(2, 60));
        // IP A uses both slots
        limiter.check_rate_limit("1.1.1.1", None);
        limiter.check_rate_limit("1.1.1.1", None);
        let a = limiter.check_rate_limit("1.1.1.1", None);
        assert!(!a.allowed, "IP A should be rate limited");

        // IP B still has its own quota
        let b = limiter.check_rate_limit("2.2.2.2", None);
        assert!(b.allowed, "IP B should not be rate limited");
    }

    #[test]
    fn test_rate_limiter_category_override() {
        let mut config = make_test_rate_config(100, 60);
        config.category_limits.insert("screenshot".to_string(), 2);
        let limiter = RateLimiter::new(config);

        // Category-specific limit = 2
        limiter.check_rate_limit("1.2.3.4", Some("screenshot"));
        limiter.check_rate_limit("1.2.3.4", Some("screenshot"));
        let result = limiter.check_rate_limit("1.2.3.4", Some("screenshot"));
        assert!(!result.allowed);
        assert_eq!(result.limit, 2);
    }

    #[test]
    fn test_rate_limiter_window_expiry() {
        // Use a 0-second window so everything expires immediately
        let limiter = RateLimiter::new(make_test_rate_config(1, 0));
        limiter.check_rate_limit("1.2.3.4", None);
        // With 0-second window, the timestamp is immediately expired on next check
        let result = limiter.check_rate_limit("1.2.3.4", None);
        assert!(result.allowed, "Expired entries should be cleaned up");
    }

    #[test]
    fn test_categorize_endpoint_screenshot() {
        assert_eq!(categorize_endpoint("/inspector/abc/screenshot"), Some("screenshot"));
        assert_eq!(categorize_endpoint("/api/v1/devices/abc/screenshot"), Some("screenshot"));
        // Note: /api/screenshot/batch is categorized as "batch" (see test_categorize_endpoint_batch)
    }

    #[test]
    fn test_categorize_endpoint_auth() {
        // Auth endpoints should be rate limited more strictly (Story 14-1)
        assert_eq!(categorize_endpoint("/api/v1/auth/login"), Some("auth"));
        assert_eq!(categorize_endpoint("/api/v1/auth/register"), Some("auth"));
        // Other auth endpoints use default rate limit
        assert_eq!(categorize_endpoint("/api/v1/auth/refresh"), None);
        assert_eq!(categorize_endpoint("/api/v1/auth/logout"), None);
    }

    #[test]
    fn test_categorize_endpoint_control() {
        assert_eq!(categorize_endpoint("/inspector/abc/touch"), Some("control"));
        assert_eq!(categorize_endpoint("/inspector/abc/input"), Some("control"));
        assert_eq!(categorize_endpoint("/inspector/abc/keyevent"), Some("control"));
    }

    #[test]
    fn test_categorize_endpoint_batch() {
        assert_eq!(categorize_endpoint("/api/batch/tap"), Some("batch"));
        assert_eq!(categorize_endpoint("/api/v1/batch/swipe"), Some("batch"));
        // /api/screenshot/batch is a batch operation, not a screenshot endpoint
        assert_eq!(categorize_endpoint("/api/screenshot/batch"), Some("batch"));
    }

    #[test]
    fn test_categorize_endpoint_websocket() {
        assert_eq!(categorize_endpoint("/nio/abc/ws"), Some("websocket"));
        assert_eq!(categorize_endpoint("/scrcpy/abc/ws"), Some("websocket"));
        assert_eq!(categorize_endpoint("/video/convert"), Some("websocket"));
    }

    #[test]
    fn test_categorize_endpoint_default() {
        assert_eq!(categorize_endpoint("/list"), None);
        assert_eq!(categorize_endpoint("/api/v1/devices"), None);
        assert_eq!(categorize_endpoint("/heartbeat"), None);
    }

    #[test]
    fn test_rate_limit_disabled_when_no_limiter() {
        let rl = RateLimit::new(None);
        assert!(rl.limiter.is_none());
    }

    #[test]
    fn test_rate_limit_enabled_with_limiter() {
        let limiter = Arc::new(RateLimiter::new(make_test_rate_config(100, 60)));
        let rl = RateLimit::new(Some(limiter));
        assert!(rl.limiter.is_some());
    }
}

// ═══════════════ JWT AUTHENTICATION (Story 14-1) ═══════════════

use crate::models::user::UserInfo;
use crate::services::auth_service::AuthService;

/// JWT authentication middleware (Story 14-1).
///
/// Validates JWT access tokens and injects user info into request.
/// When no auth service is configured, all requests pass through.
pub struct JwtAuth {
    auth_service: Option<Arc<AuthService>>,
}

impl JwtAuth {
    pub fn new(auth_service: Option<Arc<AuthService>>) -> Self {
        Self { auth_service }
    }
}

impl<S, B> Transform<S, ServiceRequest> for JwtAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = JwtAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(JwtAuthMiddleware {
            service,
            auth_service: self.auth_service.clone(),
        })
    }
}

pub struct JwtAuthMiddleware<S> {
    service: S,
    auth_service: Option<Arc<AuthService>>,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Either<
        futures::future::Map<S::Future, fn(Result<ServiceResponse<B>, Error>) -> Result<ServiceResponse<EitherBody<B>>, Error>>,
        Ready<Result<ServiceResponse<EitherBody<B>>, Error>>,
    >;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let auth_service = match &self.auth_service {
            Some(service) => service,
            None => {
                // Auth disabled — pass through
                return Either::Left(futures::future::FutureExt::map(
                    self.service.call(req),
                    |res| res.map(|r| r.map_into_left_body()),
                ));
            }
        };

        // Check if path is exempt from JWT auth
        if is_jwt_exempt(req.path()) {
            return Either::Left(futures::future::FutureExt::map(
                self.service.call(req),
                |res| res.map(|r| r.map_into_left_body()),
            ));
        }

        // Extract Bearer token
        let token = match extract_bearer_token(&req) {
            Some(t) => t,
            None => {
                let response = HttpResponse::Unauthorized()
                    .json(serde_json::json!({
                        "status": "error",
                        "error": "CC-AUTH-103",
                        "message": "Missing or invalid Authorization header"
                    }));
                return Either::Right(ok(req.into_response(response).map_into_right_body()));
            }
        };

        // Validate token
        match auth_service.validate_token(&token) {
            Ok(claims) => {
                // Inject user info into request (includes team_id for Story 14-3)
                let user_info = UserInfo {
                    id: claims.sub.clone(),
                    email: claims.email.clone(),
                    role: claims.role.clone(),
                    team_id: claims.team_id.clone(),
                };
                req.extensions_mut().insert(user_info);
                Either::Left(futures::future::FutureExt::map(
                    self.service.call(req),
                    |res| res.map(|r| r.map_into_left_body()),
                ))
            }
            Err(e) => {
                let (error_code, message) = match e {
                    crate::services::auth_service::AuthError::TokenExpired => ("CC-AUTH-102", "Token expired"),
                    _ => ("CC-AUTH-103", "Invalid token"),
                };
                let response = HttpResponse::Unauthorized()
                    .json(serde_json::json!({
                        "status": "error",
                        "error": error_code,
                        "message": message
                    }));
                Either::Right(ok(req.into_response(response).map_into_right_body()))
            }
        }
    }
}

/// Check if a path is exempt from JWT authentication.
fn is_jwt_exempt(path: &str) -> bool {
    // Auth endpoints
    if path == "/api/v1/auth/register"
        || path == "/api/v1/auth/login"
        || path == "/api/v1/auth/refresh"
        || path == "/api/v1/auth/status"
    {
        return true;
    }

    // Health and OpenAPI
    if path == "/api/v1/health" || path == "/api/v1/openapi.json" {
        return true;
    }

    // Static files
    if path.starts_with("/static/") {
        return true;
    }

    // Page routes (HTML pages)
    if path == "/"
        || path == "/async"
        || path == "/installfile"
        || path == "/test"
        || path == "/files"
        || path == "/providers"
        || path == "/atxagent"
    {
        return true;
    }

    // Dynamic page routes
    if path.starts_with("/devices/")
        && (path.ends_with("/remote")
            || path.ends_with("/edit")
            || path.ends_with("/property")
            || path.ends_with("/reserved"))
    {
        return true;
    }

    false
}
