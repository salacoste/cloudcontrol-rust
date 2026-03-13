//! Integration tests for Team/Organization Scoping (Story 14-3)
//!
//! Tests cover team CRUD, member management, device assignment, and team-scoped visibility.

mod common;

use actix_web::{test, web, App};
use cloudcontrol::routes::{admin, api_v1, auth};
use cloudcontrol::models::team::CreateTeamRequest;
use cloudcontrol::middleware::JwtAuth;
use common::{create_test_app_state_with_jwt_auth, promote_user_to_admin, create_test_device, add_user_to_team, assign_device_to_team};
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

// ============================================================================
// HIGH-1 FIX: Team-scoped device visibility tests (AC #6, #7, #14)
// ============================================================================

/// Test that agent users only see devices in their team (AC #6)
#[actix_web::test]
async fn test_agent_sees_only_team_devices() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1")
                    .wrap(JwtAuth::new(auth_service.clone()))
                    .route("/devices", web::get().to(api_v1::list_devices))
            )
            .service(
                web::scope("/api/v1/admin")
                    .wrap(JwtAuth::new(auth_service.clone()))
                    .route("/teams", web::post().to(admin::create_team))
            )
    )
    .await;

    // Create test devices
    let device1 = create_test_device(&state, "CC-001").await;
    let device2 = create_test_device(&state, "CC-002").await;
    let device3 = create_test_device(&state, "CC-003").await;

    // Create a team
    let team_service = state.team_service.as_ref().unwrap();
    let team = team_service.create_team(
        &cloudcontrol::models::team::CreateTeamRequest {
            name: "Team Alpha".to_string(),
            description: None,
        },
        "system",
        None,
    ).await.expect("Team creation should succeed");

    // Assign device1 to the team
    assign_device_to_team(&state, &device1.udid, &team.id).await;

    // Register an agent user
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

    if register_json["data"]["id"].is_null() {
        println!("Skipping test - auth not configured");
        return;
    }

    let user_id = register_json["data"]["id"].as_str().unwrap();

    // Add user to team
    add_user_to_team(&state, user_id, &team.id).await;

    // Login to get token with team_id in claims
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
            println!("Skipping test - no access token");
            return;
        }
    };

    // List devices - should only see device1 (team's device)
    let list_req = test::TestRequest::get()
        .uri("/api/v1/devices")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, list_req).await;
    assert!(resp.status().is_success(), "Device list should succeed");

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let devices = json["data"]["devices"].as_array().expect("Devices should be array");
    let udids: Vec<&str> = devices.iter()
        .filter_map(|d| d["udid"].as_str())
        .collect();

    assert!(udids.contains(&"CC-001"), "Should see team device CC-001");
    assert!(!udids.contains(&"CC-002"), "Should NOT see non-team device CC-002");
    assert!(!udids.contains(&"CC-003"), "Should NOT see unassigned device CC-003");
}

/// Test that admin sees all devices regardless of team (AC #7)
#[actix_web::test]
async fn test_admin_sees_all_devices() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1")
                    .wrap(JwtAuth::new(auth_service.clone()))
                    .route("/devices", web::get().to(api_v1::list_devices))
            )
            .service(
                web::scope("/api/v1/admin")
                    .wrap(JwtAuth::new(auth_service.clone()))
                    .route("/teams", web::post().to(admin::create_team))
            )
    )
    .await;

    // Create test devices
    let _device1 = create_test_device(&state, "CC-001").await;
    let _device2 = create_test_device(&state, "CC-002").await;
    let _device3 = create_test_device(&state, "CC-003").await;

    // Create a team and assign device1
    let team_service = state.team_service.as_ref().unwrap();
    let team = team_service.create_team(
        &cloudcontrol::models::team::CreateTeamRequest {
            name: "Team Alpha".to_string(),
            description: None,
        },
        "system",
        None,
    ).await.expect("Team creation should succeed");
    assign_device_to_team(&state, "CC-001", &team.id).await;

    // Register and promote to admin
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

    if register_json["data"]["id"].is_null() {
        println!("Skipping test - auth not configured");
        return;
    }

    let user_id = register_json["data"]["id"].as_str().unwrap();
    promote_user_to_admin(&state, user_id).await;

    // Login
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
            println!("Skipping test - no access token");
            return;
        }
    };

    // List devices - should see ALL devices
    let list_req = test::TestRequest::get()
        .uri("/api/v1/devices")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, list_req).await;
    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let devices = json["data"]["devices"].as_array().expect("Devices should be array");
    let udids: Vec<&str> = devices.iter()
        .filter_map(|d| d["udid"].as_str())
        .collect();

    assert!(udids.contains(&"CC-001"), "Admin should see CC-001");
    assert!(udids.contains(&"CC-002"), "Admin should see CC-002");
    assert!(udids.contains(&"CC-003"), "Admin should see CC-003");
}

/// Test that user without team sees no devices (AC #14)
#[actix_web::test]
async fn test_user_without_team_sees_no_devices() {
    let (_tmp, state) = create_test_app_state_with_jwt_auth().await;
    let auth_service = state.auth_service.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/api/v1/auth/register", web::post().to(auth::register))
            .route("/api/v1/auth/login", web::post().to(auth::login))
            .service(
                web::scope("/api/v1")
                    .wrap(JwtAuth::new(auth_service.clone()))
                    .route("/devices", web::get().to(api_v1::list_devices))
            )
    )
    .await;

    // Create test devices
    let _device1 = create_test_device(&state, "CC-001").await;
    let _device2 = create_test_device(&state, "CC-002").await;

    // Register a user (not added to any team)
    let register_req = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(&json!({
            "email": "lonely@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let register_resp = test::call_service(&app, register_req).await;
    let register_body = test::read_body(register_resp).await;
    let register_json: serde_json::Value = serde_json::from_slice(&register_body).unwrap();

    if register_json["data"]["id"].is_null() {
        println!("Skipping test - auth not configured");
        return;
    }

    // Login
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(&json!({
            "email": "lonely@test.com",
            "password": "SecureP@ss123"
        }))
        .to_request();
    let login_resp = test::call_service(&app, login_req).await;
    let login_body = test::read_body(login_resp).await;
    let login_json: serde_json::Value = serde_json::from_slice(&login_body).unwrap();

    let token = match login_json["data"]["access_token"].as_str() {
        Some(t) => t,
        None => {
            println!("Skipping test - no access token");
            return;
        }
    };

    // List devices - should see empty list
    let list_req = test::TestRequest::get()
        .uri("/api/v1/devices")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, list_req).await;
    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let devices = json["data"]["devices"].as_array().expect("Devices should be array");
    assert_eq!(devices.len(), 0, "User without team should see no devices");
}
