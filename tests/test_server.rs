mod common;

use actix_web::{test, web, App};
use cloudcontrol::pool::connection_pool::ConnectionPool;
use cloudcontrol::routes;
use cloudcontrol::state::AppState;
use common::{create_temp_db, make_device_json, make_test_config};
use serde_json::{json, Value};
use std::time::Duration;

/// Helper macro to create a test app and avoid type inference issues.
/// Returns (TempDir, AppState, app_service) where app_service is used via test::call_service.
macro_rules! setup_test_app {
    () => {{
        let (tmp, db) = create_temp_db().await;
        let config = make_test_config();
        let pool = ConnectionPool::new(100, Duration::from_secs(60));
        let tera = tera::Tera::new("resources/templates/**/*").unwrap();
        let state = AppState::new(db, config, pool, tera, "127.0.0.1".to_string());

        let app_state = state.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(app_state))
                .route("/", web::get().to(routes::control::index))
                .route("/devices/{udid}/remote", web::get().to(routes::control::remote))
                .route("/installfile", web::get().to(routes::control::installfile))
                .route("/list", web::get().to(routes::control::device_list))
                .route("/devices/{udid}/info", web::get().to(routes::control::device_info))
                .route("/inspector/{udid}/screenshot", web::get().to(routes::control::inspector_screenshot))
                .route("/inspector/{udid}/touch", web::post().to(routes::control::inspector_touch))
                .route("/inspector/{udid}/input", web::post().to(routes::control::inspector_input))
                .route("/inspector/{udid}/keyevent", web::post().to(routes::control::inspector_keyevent))
                .route("/heartbeat", web::post().to(routes::control::heartbeat))
                .route("/shell", web::post().to(routes::control::shell))
                .route("/api/wifi-connect", web::post().to(routes::control::wifi_connect))
                .route("/api/devices/add", web::post().to(routes::control::add_device))
                .route("/api/devices/{udid}", web::delete().to(routes::control::disconnect_device))
                .route("/api/devices/{udid}/reconnect", web::post().to(routes::control::reconnect_device))
                .route("/api/screenshot/batch", web::post().to(routes::control::batch_screenshot))
                .route("/files", web::get().to(routes::control::files))
                .route("/file/delete/{group}/{filename}", web::get().to(routes::control::file_delete))
                .route("/nio/stats", web::get().to(routes::nio::nio_stats)),
        )
        .await;

        (tmp, state, app)
    }};
}

/// Helper to insert a mock device directly into the test DB.
async fn insert_device(state: &AppState, udid: &str, present: bool, is_mock: bool) {
    let data = make_device_json(udid, present, is_mock);
    state.db.upsert(udid, &data).await.unwrap();
}

// ═══════════════ PAGE ROUTES ═══════════════

#[actix_web::test]
async fn test_index_page() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    // Index route redirects to /async
    assert_eq!(resp.status(), 302);
    let location = resp.headers().get("Location").unwrap().to_str().unwrap();
    assert_eq!(location, "/async");
}

#[actix_web::test]
async fn test_installfile_page() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::get().uri("/installfile").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_remote_page_not_found() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::get()
        .uri("/devices/nonexistent-udid/remote")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_remote_page_with_device() {
    let (_tmp, state, app) = setup_test_app!();
    insert_device(&state, "remote-test-dev", true, false).await;

    let req = test::TestRequest::get()
        .uri("/devices/remote-test-dev/remote")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

// ═══════════════ DEVICE API ═══════════════

#[actix_web::test]
async fn test_device_list_empty() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::get().uri("/list").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_device_list_with_devices() {
    let (_tmp, state, app) = setup_test_app!();
    insert_device(&state, "list-dev-1", true, false).await;
    insert_device(&state, "list-dev-2", true, false).await;
    insert_device(&state, "list-dev-3", true, false).await;

    let req = test::TestRequest::get().uri("/list").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 3);
}

#[actix_web::test]
async fn test_device_info_found() {
    let (_tmp, state, app) = setup_test_app!();
    insert_device(&state, "info-dev", true, false).await;

    let req = test::TestRequest::get()
        .uri("/devices/info-dev/info")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["udid"], "info-dev");
    assert_eq!(body["model"], "TestPhone");
}

#[actix_web::test]
async fn test_device_info_not_found() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::get()
        .uri("/devices/nonexistent/info")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

// ═══════════════ HEARTBEAT ═══════════════

#[actix_web::test]
async fn test_heartbeat_new_session() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::post()
        .uri("/heartbeat")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("identifier=heartbeat-test-1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body = test::read_body(resp).await;
    assert_eq!(body, "hello kitty");
}

#[actix_web::test]
async fn test_heartbeat_updates_session() {
    let (_tmp, _state, app) = setup_test_app!();

    // First heartbeat
    let req1 = test::TestRequest::post()
        .uri("/heartbeat")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("identifier=hb-update-test")
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), 200);

    // Second heartbeat (same identifier)
    let req2 = test::TestRequest::post()
        .uri("/heartbeat")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("identifier=hb-update-test")
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), 200);

    let body = test::read_body(resp2).await;
    assert_eq!(body, "hello kitty");
}

#[actix_web::test]
async fn test_heartbeat_missing_identifier() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::post()
        .uri("/heartbeat")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

// ═══════════════ FILES MANAGEMENT ═══════════════

#[actix_web::test]
async fn test_files_empty() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::get().uri("/files").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 0);
    assert_eq!(body["data"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn test_files_pagination() {
    let (_tmp, state, app) = setup_test_app!();

    // Insert 8 files directly via DB
    for i in 0..8i64 {
        state
            .db
            .save_install_file("0", &format!("file_{}.apk", i), Some(1000 + i), "2024-01-01", "admin", None)
            .await
            .unwrap();
    }

    let req = test::TestRequest::get().uri("/files?page=1").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 8);
    assert_eq!(body["per_page"], 5);
    assert_eq!(body["current_page"], 1);
    assert_eq!(body["data"].as_array().unwrap().len(), 5);
}

#[actix_web::test]
async fn test_file_delete_redirect() {
    let (_tmp, state, app) = setup_test_app!();

    state
        .db
        .save_install_file("0", "test.apk", Some(512), "2024-01-01", "admin", None)
        .await
        .unwrap();

    let req = test::TestRequest::get()
        .uri("/file/delete/0/test.apk")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 302);
    let location = resp.headers().get("Location").unwrap().to_str().unwrap();
    assert_eq!(location, "/installfile");
}

// ═══════════════ MOCK DEVICE SCREENSHOT ═══════════════

#[actix_web::test]
async fn test_mock_screenshot_base64() {
    let (_tmp, state, app) = setup_test_app!();
    insert_device(&state, "mock-screenshot-dev", true, true).await;

    let req = test::TestRequest::get()
        .uri("/inspector/mock-screenshot-dev/screenshot")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["type"], "jpeg");
    assert_eq!(body["encoding"], "base64");
    assert!(body["data"].as_str().unwrap().len() > 100, "base64 data should be non-trivial");
}

#[actix_web::test]
async fn test_mock_touch_ok() {
    let (_tmp, state, app) = setup_test_app!();
    insert_device(&state, "mock-touch-dev", true, true).await;

    let req = test::TestRequest::post()
        .uri("/inspector/mock-touch-dev/touch")
        .set_json(json!({"action": "click", "x": 540, "y": 960}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
}

// ═══════════════ INPUT VALIDATION ═══════════════

#[actix_web::test]
async fn test_touch_missing_coordinates() {
    let (_tmp, state, app) = setup_test_app!();
    insert_device(&state, "touch-val-dev", true, false).await;

    let req = test::TestRequest::post()
        .uri("/inspector/touch-val-dev/touch")
        .set_json(json!({"action": "click"})) // Missing x, y
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_wifi_connect_missing_address() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::post()
        .uri("/api/wifi-connect")
        .set_json(json!({"address": ""}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_wifi_connect_invalid_format() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::post()
        .uri("/api/wifi-connect")
        .set_json(json!({"address": "192.168.1.100"})) // No port
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

// ═══════════════ NIO STATS ═══════════════

#[actix_web::test]
async fn test_nio_stats() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::get().uri("/nio/stats").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert!(body.is_object());
}

// ═══════════════ MANUAL DEVICE ADDITION ═══════════════

#[actix_web::test]
async fn test_add_device_missing_ip() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::post()
        .uri("/api/devices/add")
        .set_json(json!({"ip": "", "port": 9008}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_add_device_invalid_ip_format() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::post()
        .uri("/api/devices/add")
        .set_json(json!({"ip": "invalid-ip", "port": 9008}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_add_device_unreachable() {
    let (_tmp, _state, app) = setup_test_app!();
    // Use a non-routable IP that will fail to connect
    let req = test::TestRequest::post()
        .uri("/api/devices/add")
        .set_json(json!({"ip": "198.51.100.1", "port": 9008}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Should return 503 Service Unavailable for unreachable device
    assert_eq!(resp.status(), 503);
}

#[actix_web::test]
async fn test_add_device_default_port() {
    let (_tmp, _state, app) = setup_test_app!();
    // Test that port defaults to 9008 when not specified
    let req = test::TestRequest::post()
        .uri("/api/devices/add")
        .set_json(json!({"ip": "198.51.100.1"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Should attempt connection (will fail since IP is unreachable)
    assert_eq!(resp.status(), 503);
}

// ═══════════════ DEVICE DISCONNECT/RECONNECT TESTS (Story 1B-6) ═══════════════

#[actix_web::test]
async fn test_disconnect_device_success() {
    let (_tmp, state, app) = setup_test_app!();
    // Insert a connected device
    insert_device(&state, "disconnect-test-1", true, false).await;

    // Verify device is initially present
    let device_before = state.db.find_by_udid("disconnect-test-1").await.unwrap();
    assert!(device_before.is_some());
    assert_eq!(device_before.unwrap()["present"], true);

    // Disconnect the device
    let req = test::TestRequest::delete()
        .uri("/api/devices/disconnect-test-1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Verify API response
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "success");

    // Verify device is now marked offline in database
    let device_after = state.db.find_by_udid("disconnect-test-1").await.unwrap();
    assert!(device_after.is_some());
    assert_eq!(device_after.unwrap()["present"], false);
}

#[actix_web::test]
async fn test_disconnect_device_not_found() {
    let (_tmp, _state, app) = setup_test_app!();
    // Try to disconnect a non-existent device
    let req = test::TestRequest::delete()
        .uri("/api/devices/nonexistent-device")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_reconnect_device_not_found() {
    let (_tmp, _state, app) = setup_test_app!();
    // Try to reconnect a non-existent device
    let req = test::TestRequest::post()
        .uri("/api/devices/nonexistent-device/reconnect")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_reconnect_device_unreachable() {
    let (_tmp, state, app) = setup_test_app!();
    // Insert an offline device with an unreachable IP
    let data = json!({
        "udid": "reconnect-test-unreachable",
        "ip": "198.51.100.1",  // Non-routable IP
        "port": 9008,
        "present": false,
        "model": "TestPhone",
    });
    state.db.upsert("reconnect-test-unreachable", &data).await.unwrap();

    // Try to reconnect
    let req = test::TestRequest::post()
        .uri("/api/devices/reconnect-test-unreachable/reconnect")
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Should return 503 since device is unreachable
    assert_eq!(resp.status(), 503);
}

#[actix_web::test]
async fn test_reconnect_device_no_ip() {
    let (_tmp, state, app) = setup_test_app!();
    // Insert an offline device without an IP address
    let data = json!({
        "udid": "reconnect-test-no-ip",
        "present": false,
        "model": "TestPhone",
    });
    state.db.upsert("reconnect-test-no-ip", &data).await.unwrap();

    // Try to reconnect - should return 400 since no IP
    let req = test::TestRequest::post()
        .uri("/api/devices/reconnect-test-no-ip/reconnect")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "error");
    assert!(body["message"].as_str().unwrap().contains("no IP address"));
}

// ═══════════════ BATCH SCREENSHOT TESTS (Story 2-4) ═══════════════

#[actix_web::test]
async fn test_batch_screenshot_empty_devices() {
    let (_tmp, _state, app) = setup_test_app!();
    let req = test::TestRequest::post()
        .uri("/api/screenshot/batch")
        .set_json(json!({"devices": []}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_batch_screenshot_single_mock_device() {
    let (_tmp, state, app) = setup_test_app!();
    insert_device(&state, "batch-mock-1", true, true).await;

    let req = test::TestRequest::post()
        .uri("/api/screenshot/batch")
        .set_json(json!({"devices": ["batch-mock-1"]}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "success");
    assert_eq!(body["total"], 1);
    assert_eq!(body["success"], 1);
    assert_eq!(body["failed"], 0);
    assert!(body["results"].is_object());
    assert!(body["results"]["batch-mock-1"]["status"] == "success");
}

#[actix_web::test]
async fn test_batch_screenshot_multiple_devices() {
    let (_tmp, state, app) = setup_test_app!();
    // Insert 5 mock devices
    for i in 1..=5 {
        insert_device(&state, &format!("batch-mock-{}", i), true, true).await;
    }

    let req = test::TestRequest::post()
        .uri("/api/screenshot/batch")
        .set_json(json!({"devices": ["batch-mock-1", "batch-mock-2", "batch-mock-3", "batch-mock-4", "batch-mock-5"]}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "success");
    assert_eq!(body["total"], 5);
    assert_eq!(body["success"], 5);
    assert_eq!(body["failed"], 0);
}

#[actix_web::test]
async fn test_batch_screenshot_partial_failure() {
    let (_tmp, state, app) = setup_test_app!();
    // Insert 4 mock devices and 1 non-mock (will fail to get screenshot)
    insert_device(&state, "batch-partial-1", true, true).await;
    insert_device(&state, "batch-partial-2", true, true).await;
    insert_device(&state, "batch-partial-3", true, false).await; // Non-mock, will fail
    insert_device(&state, "batch-partial-4", true, true).await;

    let req = test::TestRequest::post()
        .uri("/api/screenshot/batch")
        .set_json(json!({"devices": ["batch-partial-1", "batch-partial-2", "batch-partial-3", "batch-partial-4"]}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Should return 207 Multi-Status for partial success
    assert_eq!(resp.status(), 207);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "partial");
    assert_eq!(body["total"], 4);
    assert!(body["success"].as_u64().unwrap() >= 3);
    assert!(body["failed"].as_u64().unwrap() >= 1);
}

#[actix_web::test]
async fn test_batch_screenshot_with_quality_and_scale() {
    let (_tmp, state, app) = setup_test_app!();
    insert_device(&state, "batch-quality-1", true, true).await;

    let req = test::TestRequest::post()
        .uri("/api/screenshot/batch")
        .set_json(json!({"devices": ["batch-quality-1"], "quality": 50, "scale": 0.5}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "success");
}
