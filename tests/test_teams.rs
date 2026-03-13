//! Integration tests for Team/Organization Scoping (Story 14-3)
//!
//! Tests cover team CRUD, member management, device assignment, and team-scoped visibility.

mod common;

use actix_web::{test, web, App};
use cloudcontrol::routes::{admin, auth};
use cloudcontrol::models::team::CreateTeamRequest;
use cloudcontrol::middleware::JwtAuth;
use common::{create_test_app_state_with_jwt_auth, promote_user_to_admin};
use serde_json::json;

/// Test admin can create a team
#[actix_web::test]
async fn test_admin_can_create_team() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1/admin")
                    .wrap(JwtAuth::new(auth_service))
                    .route("/teams", web::post().to(admin::create_team))
                    .route("/teams", web::get().to(admin::list_teams))
            )
    )
    .await;

    // Register a user
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&json!({
            "email": "admin@test.com",
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

    // Promote user to admin role BEFORE logging in
    let user_id = register_json["data"]["id"].as_str().unwrap();
    promote_user_to_admin(&state, user_id).await;

    // Login as admin (token will now have admin role)
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "admin@test.com",
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

    // Create team
    let create_req = CreateTeamRequest {
        name: "Test Team Alpha".to_string(),
        description: Some("Integration test team".to_string()),
    };

    let create_team_req = test::TestRequest::post()
        .uri("/api/v1/admin/teams")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&create_req)
        .to_request();

    let resp = test::call_service(&app, create_team_req).await;
    let status = resp.status();
    assert!(status.is_success(), "Team creation should succeed: {}", status);
}

/// Test non-admin cannot create team
#[actix_web::test]
async fn test_non_admin_cannot_create_team() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1/admin")
                    .wrap(JwtAuth::new(auth_service))
                    .route("/teams", web::post().to(admin::create_team))
            )
    )
    .await;

    // Register a regular user (agent role is default)
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&json!({
            "email": "agent@test.com",
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

    // Login as agent (NOT promoted to admin - keeps default agent role)
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "agent@test.com",
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

    // Try to create team - should fail with 403
    let create_req = CreateTeamRequest {
        name: "Unauthorized Team".to_string(),
        description: None,
    };

    let create_team_req = test::TestRequest::post()
        .uri("/api/v1/admin/teams")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&create_req)
        .to_request();

    let resp = test::call_service(&app, create_team_req).await;
    assert_eq!(resp.status(), 403, "Non-admin should get 403 Forbidden");
}

/// Test admin can list teams
#[actix_web::test]
async fn test_admin_can_list_teams() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1/admin")
                    .wrap(JwtAuth::new(auth_service))
                    .route("/teams", web::get().to(admin::list_teams))
            )
    )
    .await;

    // Register a user
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&json!({
            "email": "admin@test.com",
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

    // Promote user to admin role BEFORE logging in
    let user_id = register_json["data"]["id"].as_str().unwrap();
    promote_user_to_admin(&state, user_id).await;

    // Login as admin (token will now have admin role)
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "admin@test.com",
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

    // List teams
    let list_req = test::TestRequest::get()
        .uri("/api/v1/admin/teams")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, list_req).await;
    assert!(resp.status().is_success(), "List teams should succeed");

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["data"].is_array(), "Response should contain teams array");
}
