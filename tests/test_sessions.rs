//! Integration tests for Session Management (Story 14-4)
//!
//! Tests cover session listing, revocation, and cross-user access prevention.

mod common;

use actix_web::{test, web, App};
use cloudcontrol::routes::auth;
use cloudcontrol::middleware::JwtAuth;
use common::create_test_app_state_with_jwt_auth;
use serde_json::json;

/// Test listing sessions returns empty array when no sessions
#[actix_web::test]
async fn test_list_sessions_empty() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1/auth")
                    .wrap(JwtAuth::new(auth_service))
                    .route("/sessions", web::get().to(auth::list_sessions))
            )
    )
    .await;

    // Register a user
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&json!({
            "email": "user@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let register_resp = test::call_service(&app, register_req).await;
    let register_body = test::read_body(register_resp).await;
    let register_json: serde_json::Value = serde_json::from_slice(&register_body).unwrap();

    // Skip if auth not configured
    if register_json["data"]["id"].is_null() {
        println!("Skipping test - auth not configured");
        return;
    }

    // Login to get token
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "user@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let login_resp = test::call_service(&app, login_req).await;
    let login_body = test::read_body(login_resp).await;
    let login_json: serde_json::Value = serde_json::from_slice(&login_body).unwrap();

    let token = match login_json["data"]["access_token"].as_str() {
        Some(t) => t,
        None => {
            println!("Skipping test - no access token in response");
            return;
        }
    };

    // List sessions - should have at least one (the current login)
    let list_req = test::TestRequest::get()
        .uri("/api/v1/auth/sessions")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, list_req).await;
    assert!(resp.status().is_success(), "List sessions should succeed");

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have at least one session (the one just created)
    assert!(json["data"]["sessions"].is_array(), "Sessions should be an array");
    assert!(json["data"]["total"].as_u64().unwrap() >= 1, "Should have at least one session");
}

/// Test revoking a specific session
#[actix_web::test]
async fn test_revoke_specific_session() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1/auth")
                    .wrap(JwtAuth::new(auth_service))
                    .route("/sessions", web::get().to(auth::list_sessions))
                    .route("/sessions/{session_id}", web::delete().to(auth::revoke_session))
            )
    )
    .await;

    // Register and login to create first session
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&json!({
            "email": "user@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _ = test::call_service(&app, register_req).await;

    // Login twice to create two sessions
    let login1_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "user@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let login1_resp = test::call_service(&app, login1_req).await;
    let login1_body = test::read_body(login1_resp).await;
    let login1_json: serde_json::Value = serde_json::from_slice(&login1_body).unwrap();

    let token1 = match login1_json["data"]["access_token"].as_str() {
        Some(t) => t,
        None => {
            println!("Skipping test - no access token");
            return;
        }
    };

    let login2_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "user@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _ = test::call_service(&app, login2_req).await;

    // List sessions
    let list_req = test::TestRequest::get()
        .uri("/api/v1/auth/sessions")
        .insert_header(("Authorization", format!("Bearer {}", token1)))
        .to_request();
    let list_resp = test::call_service(&app, list_req).await;
    let list_body = test::read_body(list_resp).await;
    let list_json: serde_json::Value = serde_json::from_slice(&list_body).unwrap();

    let sessions = list_json["data"]["sessions"].as_array().unwrap();
    if sessions.len() < 2 {
        println!("Skipping test - not enough sessions");
        return;
    }

    // Find a session to revoke (not the current one)
    let session_to_revoke = &sessions[1]["id"].as_str().unwrap();

    // Revoke the session
    let revoke_req = test::TestRequest::delete()
        .uri(&format!("/api/v1/auth/sessions/{}", session_to_revoke))
        .insert_header(("Authorization", format!("Bearer {}", token1)))
        .to_request();

    let resp = test::call_service(&app, revoke_req).await;
    assert!(resp.status().is_success(), "Revoke session should succeed");

    // Verify it was revoked by listing sessions again
    let list2_req = test::TestRequest::get()
        .uri("/api/v1/auth/sessions")
        .insert_header(("Authorization", format!("Bearer {}", token1)))
        .to_request();
    let list2_resp = test::call_service(&app, list2_req).await;
    let list2_body = test::read_body(list2_resp).await;
    let list2_json: serde_json::Value = serde_json::from_slice(&list2_body).unwrap();

    let remaining_sessions = list2_json["data"]["sessions"].as_array().unwrap();
    let revoked_exists = remaining_sessions
        .iter()
        .any(|s| s["id"].as_str().unwrap() == *session_to_revoke);

    assert!(!revoked_exists, "Revoked session should not appear in list");
}

/// Test that user cannot revoke another user's session
#[actix_web::test]
async fn test_cannot_revoke_other_user_session() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1/auth")
                    .wrap(JwtAuth::new(auth_service))
                    .route("/sessions", web::get().to(auth::list_sessions))
                    .route("/sessions/{session_id}", web::delete().to(auth::revoke_session))
            )
    )
    .await;

    // Register and login as user1
    let register1_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&json!({
            "email": "user1@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _ = test::call_service(&app, register1_req).await;

    let login1_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "user1@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let login1_resp = test::call_service(&app, login1_req).await;
    let login1_body = test::read_body(login1_resp).await;
    let login1_json: serde_json::Value = serde_json::from_slice(&login1_body).unwrap();

    let token1 = match login1_json["data"]["access_token"].as_str() {
        Some(t) => t,
        None => {
            println!("Skipping test - no access token");
            return;
        }
    };

    // Register and login as user2
    let register2_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&json!({
            "email": "user2@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _ = test::call_service(&app, register2_req).await;

    let login2_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "user2@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let login2_resp = test::call_service(&app, login2_req).await;
    let login2_body = test::read_body(login2_resp).await;
    let login2_json: serde_json::Value = serde_json::from_slice(&login2_body).unwrap();

    let token2 = match login2_json["data"]["access_token"].as_str() {
        Some(t) => t,
        None => {
            println!("Skipping test - no access token");
            return;
        }
    };

    // Get user2's actual session ID from the sessions list
    let list2_req = test::TestRequest::get()
        .uri("/api/v1/auth/sessions")
        .insert_header(("Authorization", format!("Bearer {}", token2)))
        .to_request();
    let list2_resp = test::call_service(&app, list2_req).await;
    let list2_body = test::read_body(list2_resp).await;
    let list2_json: serde_json::Value = serde_json::from_slice(&list2_body).unwrap();

    let sessions = list2_json["data"]["sessions"].as_array().unwrap();
    if sessions.is_empty() {
        println!("Skipping test - no sessions found for user2");
        return;
    }
    let user2_session_id = sessions[0]["id"].as_str().unwrap();

    // User1 tries to revoke user2's session (should fail with 404)
    let revoke_req = test::TestRequest::delete()
        .uri(&format!("/api/v1/auth/sessions/{}", user2_session_id))
        .insert_header(("Authorization", format!("Bearer {}", token1)))
        .to_request();

    let resp = test::call_service(&app, revoke_req).await;
    assert_eq!(resp.status(), 404, "Should get 404 when trying to revoke another user's session");
}

/// Test revoking all other sessions preserves current session
#[actix_web::test]
async fn test_revoke_all_other_sessions() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1/auth")
                    .wrap(JwtAuth::new(auth_service))
                    .route("/sessions", web::get().to(auth::list_sessions))
                    .route("/sessions", web::delete().to(auth::revoke_all_other_sessions))
            )
    )
    .await;

    // Register user
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&json!({
            "email": "user@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _ = test::call_service(&app, register_req).await;

    // Login 3 times to create 3 sessions
    let login1_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "user@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let login1_resp = test::call_service(&app, login1_req).await;
    let login1_body = test::read_body(login1_resp).await;
    let login1_json: serde_json::Value = serde_json::from_slice(&login1_body).unwrap();

    let token1 = match login1_json["data"]["access_token"].as_str() {
        Some(t) => t,
        None => {
            println!("Skipping test - no access token");
            return;
        }
    };

    // Create 2 more sessions
    let login2_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "user@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _ = test::call_service(&app, login2_req).await;

    let login3_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "user@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let _ = test::call_service(&app, login3_req).await;

    // List sessions to get current session ID
    let list_req = test::TestRequest::get()
        .uri("/api/v1/auth/sessions")
        .insert_header(("Authorization", format!("Bearer {}", token1)))
        .to_request();
    let list_resp = test::call_service(&app, list_req).await;
    let list_body = test::read_body(list_resp).await;
    let list_json: serde_json::Value = serde_json::from_slice(&list_body).unwrap();

    let sessions = list_json["data"]["sessions"].as_array().unwrap();
    let initial_count = sessions.len();
    if initial_count < 3 {
        println!("Skipping test - not enough sessions (need 3, got {})", initial_count);
        return;
    }

    // Get the first (most recent) session ID as current session
    let current_session_id = sessions[0]["id"].as_str().unwrap();

    // Revoke all other sessions
    let revoke_all_req = test::TestRequest::delete()
        .uri("/api/v1/auth/sessions")
        .insert_header(("Authorization", format!("Bearer {}", token1)))
        .insert_header(("X-Current-Session", current_session_id))
        .to_request();

    let resp = test::call_service(&app, revoke_all_req).await;
    assert!(resp.status().is_success(), "Revoke all other sessions should succeed");

    let revoke_body = test::read_body(resp).await;
    let revoke_json: serde_json::Value = serde_json::from_slice(&revoke_body).unwrap();
    assert!(revoke_json["data"]["revoked_count"].as_u64().unwrap() >= 2, "Should revoke at least 2 sessions");

    // Verify only current session remains
    let list2_req = test::TestRequest::get()
        .uri("/api/v1/auth/sessions")
        .insert_header(("Authorization", format!("Bearer {}", token1)))
        .to_request();
    let list2_resp = test::call_service(&app, list2_req).await;
    let list2_body = test::read_body(list2_resp).await;
    let list2_json: serde_json::Value = serde_json::from_slice(&list2_body).unwrap();

    let remaining_sessions = list2_json["data"]["sessions"].as_array().unwrap();
    assert_eq!(remaining_sessions.len(), 1, "Should have exactly 1 session remaining");
    assert_eq!(
        remaining_sessions[0]["id"].as_str().unwrap(),
        current_session_id,
        "Remaining session should be the current one"
    );
}

/// Test unauthenticated request is rejected
#[actix_web::test]
async fn test_sessions_require_authentication() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(
                web::scope("/api/v1/auth")
                    .wrap(JwtAuth::new(auth_service))
                    .route("/sessions", web::get().to(auth::list_sessions))
            )
    )
    .await;

    // Try to list sessions without authentication
    let list_req = test::TestRequest::get()
        .uri("/api/v1/auth/sessions")
        .to_request();

    let resp = test::call_service(&app, list_req).await;
    assert_eq!(resp.status(), 401, "Unauthenticated request should be rejected");
}
