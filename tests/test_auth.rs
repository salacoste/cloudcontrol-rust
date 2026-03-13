//! Integration tests for authentication endpoints (Story 14-1)
//!
//! Tests the full authentication flow: register, login, refresh, logout.

mod common;

use actix_web::{test, web, App};
use cloudcontrol::routes::auth;
use common::{create_test_app_state_with_jwt_auth};

/// Test user registration with valid credentials
#[actix_web::test]
async fn test_register_valid_user() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/register", web::post().to(auth::register)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "test@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status() == actix_web::http::StatusCode::CREATED);
}

/// Test registration with duplicate email
#[actix_web::test]
async fn test_register_duplicate_email() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/register", web::post().to(auth::register)),
    )
    .await;

    // First registration
    let req1 = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "duplicate@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _resp1 = test::call_service(&app, req1).await;

    // Second registration with same email
    let req2 = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "duplicate@example.com",
            "password": "DifferentP@ss456"
        }))
        .to_request();

    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), actix_web::http::StatusCode::CONFLICT);
}

/// Test registration with weak password
#[actix_web::test]
async fn test_register_weak_password() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/register", web::post().to(auth::register)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "weak@example.com",
            "password": "123"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
}

/// Test login with valid credentials
#[actix_web::test]
async fn test_login_valid_credentials() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login)),
    )
    .await;

    // Register user first
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "login@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _register_resp = test::call_service(&app, register_req).await;

    // Login
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&serde_json::json!({
            "email": "login@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();

    let resp = test::call_service(&app, login_req).await;
    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["data"]["access_token"].is_string());
    assert!(json["data"]["refresh_token"].is_string());
    // Note: token_type is serialized as "type" in the response
    assert_eq!(json["data"]["type"], "Bearer");
}

/// Test login with invalid password
#[actix_web::test]
async fn test_login_invalid_password() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login)),
    )
    .await;

    // Register user first
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "wrongpass@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _register_resp = test::call_service(&app, register_req).await;

    // Login with wrong password
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&serde_json::json!({
            "email": "wrongpass@example.com",
            "password": "WrongPassword"
        }))
        .to_request();

    let resp = test::call_service(&app, login_req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
}

/// Test login with non-existent email
#[actix_web::test]
async fn test_login_nonexistent_email() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/login", web::post().to(auth::login)),
    )
    .await;

    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&serde_json::json!({
            "email": "nonexistent@example.com",
            "password": "AnyPassword123"
        }))
        .to_request();

    let resp = test::call_service(&app, login_req).await;
    // Should return same error as wrong password (no email enumeration)
    assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
}

/// Test auth status endpoint
#[actix_web::test]
async fn test_auth_status() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/status", web::get().to(auth::auth_status)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/auth/status")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["data"]["auth_enabled"].is_boolean());
}

/// Test token refresh flow
#[actix_web::test]
async fn test_token_refresh_flow() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .route("/api/v1/auth/refresh", web::post().to(auth::refresh)),
    )
    .await;

    // Register and login
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "refresh@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _register_resp = test::call_service(&app, register_req).await;

    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&serde_json::json!({
            "email": "refresh@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let login_resp = test::call_service(&app, login_req).await;
    let login_body = test::read_body(login_resp).await;
    let login_json: serde_json::Value = serde_json::from_slice(&login_body).unwrap();
    let refresh_token = login_json["data"]["refresh_token"].as_str().unwrap().to_string();

    // Refresh token
    let refresh_req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .set_json(&serde_json::json!({
            "refresh_token": refresh_token
        }))
        .to_request();

    let resp = test::call_service(&app, refresh_req).await;
    assert!(resp.status().is_success(), "Refresh should succeed");

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["data"]["access_token"].is_string(), "Should return new access token");
    assert!(json["data"]["refresh_token"].is_string(), "Should return new refresh token");
    assert_ne!(
        json["data"]["refresh_token"].as_str().unwrap(),
        refresh_token,
        "New refresh token should be different from old one"
    );
}

/// Test refresh with revoked token (token already used)
#[actix_web::test]
async fn test_token_refresh_revoked_token() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .route("/api/v1/auth/refresh", web::post().to(auth::refresh)),
    )
    .await;

    // Register and login
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "revoked@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _register_resp = test::call_service(&app, register_req).await;

    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&serde_json::json!({
            "email": "revoked@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let login_resp = test::call_service(&app, login_req).await;
    let login_body = test::read_body(login_resp).await;
    let login_json: serde_json::Value = serde_json::from_slice(&login_body).unwrap();
    let refresh_token = login_json["data"]["refresh_token"].as_str().unwrap().to_string();

    // First refresh - should succeed
    let refresh_req1 = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .set_json(&serde_json::json!({
            "refresh_token": refresh_token
        }))
        .to_request();
    let resp1 = test::call_service(&app, refresh_req1).await;
    assert!(resp1.status().is_success(), "First refresh should succeed");

    // Second refresh with same token - should fail (token revoked)
    let refresh_req2 = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .set_json(&serde_json::json!({
            "refresh_token": refresh_token
        }))
        .to_request();
    let resp2 = test::call_service(&app, refresh_req2).await;
    assert_eq!(
        resp2.status(),
        actix_web::http::StatusCode::UNAUTHORIZED,
        "Second refresh with same token should fail"
    );

    let body2 = test::read_body(resp2).await;
    let json2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["error"], "CC-AUTH-103", "Should return token revoked error code");
}

/// Test logout flow
#[actix_web::test]
async fn test_logout_flow() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .route("/api/v1/auth/logout", web::post().to(auth::logout))
            .route("/api/v1/auth/refresh", web::post().to(auth::refresh)),
    )
    .await;

    // Register and login
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "logout@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _register_resp = test::call_service(&app, register_req).await;

    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&serde_json::json!({
            "email": "logout@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let login_resp = test::call_service(&app, login_req).await;
    let login_body = test::read_body(login_resp).await;
    let login_json: serde_json::Value = serde_json::from_slice(&login_body).unwrap();
    let refresh_token = login_json["data"]["refresh_token"].as_str().unwrap().to_string();

    // Logout
    let logout_req = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .set_json(&serde_json::json!({
            "refresh_token": refresh_token
        }))
        .to_request();
    let logout_resp = test::call_service(&app, logout_req).await;
    assert!(logout_resp.status().is_success(), "Logout should succeed");

    // Try to use the revoked refresh token - should fail
    let refresh_req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .set_json(&serde_json::json!({
            "refresh_token": refresh_token
        }))
        .to_request();
    let refresh_resp = test::call_service(&app, refresh_req).await;
    assert_eq!(
        refresh_resp.status(),
        actix_web::http::StatusCode::UNAUTHORIZED,
        "Refresh after logout should fail"
    );
}

/// Test error response format contains correct error codes
#[actix_web::test]
async fn test_error_codes_in_responses() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login)),
    )
    .await;

    // Test duplicate email returns CC-AUTH-105
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "errorcode@example.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _ = test::call_service(&app, register_req).await;

    let duplicate_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&serde_json::json!({
            "email": "errorcode@example.com",
            "password": "SecureP@ss456"
        }))
        .to_request();
    let dup_resp = test::call_service(&app, duplicate_req).await;
    let dup_body = test::read_body(dup_resp).await;
    let dup_json: serde_json::Value = serde_json::from_slice(&dup_body).unwrap();
    assert_eq!(dup_json["error"], "CC-AUTH-105", "Duplicate email should return CC-AUTH-105");

    // Test invalid credentials returns CC-AUTH-101
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&serde_json::json!({
            "email": "errorcode@example.com",
            "password": "WrongPassword123"
        }))
        .to_request();
    let login_resp = test::call_service(&app, login_req).await;
    let login_body = test::read_body(login_resp).await;
    let login_json: serde_json::Value = serde_json::from_slice(&login_body).unwrap();
    assert_eq!(login_json["error"], "CC-AUTH-101", "Invalid credentials should return CC-AUTH-101");
}
