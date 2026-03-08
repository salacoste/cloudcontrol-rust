use crate::device::adb::Adb;
use crate::device::atx_client::AtxClient;
use crate::models::api_response::{
    ApiResponse, BatchResponse, BatchResult, DeviceInfo, DeviceInfoResponse,
    ScreenshotResponse, TapRequest, SwipeRequest, InputRequest, KeyEventRequest,
    BatchTapRequest, BatchSwipeRequest, BatchInputRequest, DisplayInfo, ERR_NO_DEVICES_SELECTED,
    DeviceStatusSummary, DeviceStatusEntry, HealthCheckResponse,
};
use crate::services::device_service::DeviceService;
 use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ═════════════════════════════════════════════════════════════════════════════
// Constants
// ═════════════════════════════════════════════════════════════════════════════

/// Maximum batch size for batch operations (NFR5)
const MAX_BATCH_SIZE: usize = 20;
// ═════════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═════════════════════════════════════════════════════════════════════════════

/// Get device client from state
async fn get_device_client(
    state: &AppState,
    udid: &str,
) -> Result<(serde_json::Value, std::sync::Arc<crate::device::atx_client::AtxClient>), HttpResponse> {
    // Try device info cache first
    if let Some(cached) = state.device_info_cache.get(udid).await {
        let ip = cached.get("ip").and_then(|v| v.as_str()).unwrap_or("");
        let port = cached.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
        let (final_ip, final_port) = resolve_device_connection(&cached, ip, port).await;
        let client = state.connection_pool.get_or_create(udid, &final_ip, final_port).await;
        return Ok((cached, client));
    }

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
    let device = phone_service
        .query_info_by_udid(udid)
        .await
        .map_err(|e| {
            if e.contains("not found") {
                HttpResponse::NotFound().json(json!({
                    "status": "error",
                    "error": "ERR_DEVICE_NOT_FOUND",
                    "message": e
                }))
            } else {
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "error": "ERR_OPERATION_FAILED",
                    "message": e
                }))
            }
        })?
        .ok_or_else(|| HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_DEVICE_NOT_FOUND",
            "message": format!("Device '{}' not found", udid)
        })))?;

    state.device_info_cache.insert(udid.to_string(), device.clone()).await;

    let ip = device.get("ip").and_then(|v| v.as_str()).unwrap_or("");
    let port = device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
    let (final_ip, final_port) = resolve_device_connection(&device, ip, port).await;
    let client = state.connection_pool.get_or_create(udid, &final_ip, final_port).await;
    Ok((device, client))
}

/// Resolve device connection (WiFi vs USB)
async fn resolve_device_connection(
    device: &serde_json::Value,
    ip: &str,
    port: i64,
) -> (String, i64) {
    if !ip.is_empty() && ip != "127.0.0.1" {
        return (ip.to_string(), port);
    }
    if ip == "127.0.0.1" && port != 9008 {
        return (ip.to_string(), port);
    }
    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("");
    if !serial.is_empty() {
        if let Ok(local_port) = crate::device::adb::Adb::forward(serial, 9008).await {
            return ("127.0.0.1".to_string(), local_port as i64);
        }
    }
    (ip.to_string(), port)
}

/// Create standardized error response
fn error_response(code: &str, message: &str) -> HttpResponse {
    let status = match code {
        "ERR_DEVICE_NOT_FOUND" => actix_web::http::StatusCode::NOT_FOUND,
        "ERR_DEVICE_DISCONNECTED" => actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
        "ERR_INVALID_REQUEST" | "ERR_NO_DEVICES_SELECTED" => actix_web::http::StatusCode::BAD_REQUEST,
        "ERR_BATCH_PARTIAL_FAILURE" => actix_web::http::StatusCode::MULTI_STATUS,
        _ => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
    };
    HttpResponse::build(status).json(ApiResponse::<()> {
        status: "error".to_string(),
        data: None,
        error: Some(code.to_string()),
        message: Some(message.to_string()),
        timestamp: chrono::Utc::now(),
    })
}

/// Create standardized success response
fn success_response<T: serde::Serialize>(data: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse {
        status: "success".to_string(),
        data: Some(data),
        error: None,
        message: None,
        timestamp: chrono::Utc::now(),
    })
}

/// Helper to parse tags from JSON value
fn parse_tags(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

/// Validate batch request size
fn validate_batch_size(udids: &[String]) -> Result<(), HttpResponse> {
    if udids.is_empty() {
        return Err(error_response(ERR_NO_DEVICES_SELECTED, "At least one device must be selected"));
    }
    if udids.len() > MAX_BATCH_SIZE {
        return Err(error_response(
            "ERR_INVALID_REQUEST",
            &format!("Maximum {} devices exceeded (max: {})", udids.len(), MAX_BATCH_SIZE)
        ));
    }
    Ok(())
}

// ═════════════════════════════════════════════════════════════════════════════
// Device Endpoints
// ═════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/devices - List all connected devices
pub async fn list_devices(state: web::Data<AppState>) -> HttpResponse {
    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());

    match phone_service.query_device_list_by_present().await {
        Ok(devices) => {
            let device_infos: Vec<DeviceInfo> = devices.iter().map(|dev| {
                let display = dev.get("display").cloned().unwrap_or(json!({"width": 1080, "height": 1920}));
                DeviceInfo {
                    udid: dev.get("udid").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    model: dev.get("model").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    android_version: dev.get("version").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    battery: dev.get("battery").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                    display: DisplayInfo {
                        width: display.get("width").and_then(|v| v.as_u64()).unwrap_or(1080) as u32,
                        height: display.get("height").and_then(|v| v.as_u64()).unwrap_or(1920) as u32,
                    },
                    serial: dev.get("serial").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    ip: dev.get("ip").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    port: dev.get("port").and_then(|v| v.as_i64()).unwrap_or(9008) as i64,
                    status: dev.get("status").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    last_seen: dev.get("last_seen").and_then(|v| {
                        v.as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    }).map(|dt| chrono::DateTime::<chrono::Utc>::from(dt)),
                    tags: parse_tags(dev.get("tags").unwrap_or(&json!([]))),
                }
            }).collect();

            success_response(json!({
                "devices": device_infos,
                "total": device_infos.len()
            }))
        }
        Err(e) => error_response("ERR_OPERATION_FAILED", &e),
    }
}

/// GET /api/v1/devices/{udid} - Get device info
pub async fn get_device(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    match get_device_client(&state, &udid).await {
        Ok((device, _client)) => {
            let display = device.get("display").cloned().unwrap_or(json!({"width": 1080, "height": 1920}));
            let info = DeviceInfoResponse {
                udid: device.get("udid").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                model: device.get("model").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                android_version: device.get("version").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                battery: device.get("battery").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                display: DisplayInfo {
                    width: display.get("width").and_then(|v| v.as_u64()).unwrap_or(1080) as u32,
                    height: display.get("height").and_then(|v| v.as_u64()).unwrap_or(1920) as u32,
                },
                serial: device.get("serial").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                ip: device.get("ip").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                port: device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008) as i64,
                status: device.get("status").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                last_seen: device.get("last_seen").and_then(|v| {
                    v.as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                }).map(|dt| chrono::DateTime::<chrono::Utc>::from(dt)),
                tags: parse_tags(device.get("tags").unwrap_or(&json!([]))),
            };
            success_response(info)
        }
        Err(response) => response,
    }
}

/// GET /api/v1/devices/{udid}/screenshot - Get screenshot
pub async fn get_screenshot(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let udid = path.into_inner();
    let format = query.get("format").map(|s| s.as_str()).unwrap_or("jpeg");

    match get_device_client(&state, &udid).await {
        Ok((_device, client)) => {
            // Use screenshot_base64_direct for fastest response
            let result = client.screenshot_base64_direct().await;

            match result {
                Ok(base64_data) => {
                    success_response(ScreenshotResponse {
                        image_type: format.to_string(),
                        encoding: "base64".to_string(),
                        data: base64_data,
                    })
                }
                Err(e) => error_response("ERR_OPERATION_FAILED", &e),
            }
        }
        Err(response) => response,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Control Endpoints
// ═════════════════════════════════════════════════════════════════════════════

/// POST /api/v1/devices/{udid}/tap - Execute tap
pub async fn tap(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<TapRequest>,
) -> HttpResponse {
    let udid = path.into_inner();

    match get_device_client(&state, &udid).await {
        Ok((_device, client)) => {
            // Use click method from AtxClient
            match client.click(body.x as i32, body.y as i32).await {
                Ok(_) => success_response(json!({"executed": true, "x": body.x, "y": body.y})),
                Err(e) => error_response("ERR_OPERATION_FAILED", &e),
            }
        }
        Err(response) => response,
    }
}

/// POST /api/v1/devices/{udid}/swipe - Execute swipe
pub async fn swipe(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<SwipeRequest>,
) -> HttpResponse {
    let udid = path.into_inner();

    match get_device_client(&state, &udid).await {
        Ok((_device, client)) => {
            // Use swipe method from AtxClient with duration as f64
            match client.swipe(
                body.x1 as i32, body.y1 as i32,
                body.x2 as i32, body.y2 as i32,
                body.duration as f64
            ).await {
                Ok(_) => success_response(json!({"executed": true, "x1": body.x1, "y1": body.y1, "x2": body.x2, "y2": body.y2})),
                Err(e) => error_response("ERR_OPERATION_FAILED", &e),
            }
        }
        Err(response) => response,
    }
}

/// POST /api/v1/devices/{udid}/input - Text input
pub async fn input(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<InputRequest>,
) -> HttpResponse {
    let udid = path.into_inner();

    if body.text.is_empty() {
        return error_response("ERR_INVALID_REQUEST", "Text cannot be empty");
    }

    match get_device_client(&state, &udid).await {
        Ok((_device, client)) => {
            // Use input_text method from AtxClient
            match client.input_text(&body.text).await {
                Ok(_) => success_response(json!({"executed": true, "text_length": body.text.len()})),
                Err(e) => error_response("ERR_OPERATION_FAILED", &e),
            }
        }
        Err(response) => response,
    }
}

/// POST /api/v1/devices/{udid}/keyevent - Key event
pub async fn keyevent(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<KeyEventRequest>,
) -> HttpResponse {
    let udid = path.into_inner();

    if body.key.is_empty() {
        return error_response("ERR_INVALID_REQUEST", "Key cannot be empty");
    }

    match get_device_client(&state, &udid).await {
        Ok((_device, client)) => {
            // Use press_key method from AtxClient
            match client.press_key(&body.key).await {
                Ok(_) => success_response(json!({"executed": true, "key": body.key})),
                Err(e) => error_response("ERR_OPERATION_FAILED", &e),
            }
        }
        Err(response) => response,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Batch Endpoints
// ═════════════════════════════════════════════════════════════════════════════

/// POST /api/v1/batch/tap - Batch tap
pub async fn batch_tap(
    state: web::Data<AppState>,
    body: web::Json<BatchTapRequest>,
) -> HttpResponse {
    if body.udids.is_empty() {
        return error_response(ERR_NO_DEVICES_SELECTED, "At least one device must be selected");
    }

    let futures: Vec<_> = body.udids.iter()
        .map(|udid| execute_single_tap(&state, udid, body.x, body.y))
        .collect();

    let results = futures::future::join_all(futures).await;

    let batch_results: Vec<BatchResult> = body.udids.iter()
        .zip(results.into_iter())
        .map(|(udid, result)| BatchResult {
            udid: udid.clone(),
            status: if result.is_ok() { "success".to_string() } else { "error".to_string() },
            error: result.as_ref().err().map(|e| e.0.clone()),
            message: result.as_ref().err().map(|e| e.1.clone()),
        })
        .collect();

    let successful = batch_results.iter().filter(|r| r.status == "success").count();
    let failed = batch_results.len() - successful;

    HttpResponse::Ok().json(BatchResponse {
        status: if failed == 0 { "success".to_string() } else { "partial".to_string() },
        total: body.udids.len(),
        successful,
        failed,
        results: batch_results,
    })
}

/// POST /api/v1/batch/swipe - Batch swipe
pub async fn batch_swipe(
    state: web::Data<AppState>,
    body: web::Json<BatchSwipeRequest>,
) -> HttpResponse {
    if body.udids.is_empty() {
        return error_response(ERR_NO_DEVICES_SELECTED, "At least one device must be selected");
    }

    let futures: Vec<_> = body.udids.iter()
        .map(|udid| execute_single_swipe(&state, udid, body.x1, body.y1, body.x2, body.y2, body.duration))
        .collect();

    let results = futures::future::join_all(futures).await;

    let batch_results: Vec<BatchResult> = body.udids.iter()
        .zip(results.into_iter())
        .map(|(udid, result)| BatchResult {
            udid: udid.clone(),
            status: if result.is_ok() { "success".to_string() } else { "error".to_string() },
            error: result.as_ref().err().map(|e| e.0.clone()),
            message: result.as_ref().err().map(|e| e.1.clone()),
        })
        .collect();

    let successful = batch_results.iter().filter(|r| r.status == "success").count();
    let failed = batch_results.len() - successful;

    HttpResponse::Ok().json(BatchResponse {
        status: if failed == 0 { "success".to_string() } else { "partial".to_string() },
        total: body.udids.len(),
        successful,
        failed,
        results: batch_results,
    })
}

/// POST /api/v1/batch/input - Batch input
pub async fn batch_input(
    state: web::Data<AppState>,
    body: web::Json<BatchInputRequest>,
) -> HttpResponse {
    if body.udids.is_empty() {
        return error_response(ERR_NO_DEVICES_SELECTED, "At least one device must be selected");
    }

    if body.text.is_empty() {
        return error_response("ERR_INVALID_REQUEST", "Text cannot be empty");
    }

    let text = body.text.clone();
    let udids = body.udids.clone();

    let futures: Vec<_> = udids.iter()
        .map(|udid| execute_single_input(&state, udid, &text))
        .collect();

    let results = futures::future::join_all(futures).await;

    let batch_results: Vec<BatchResult> = udids.iter()
        .zip(results.into_iter())
        .map(|(udid, result)| BatchResult {
            udid: udid.clone(),
            status: if result.is_ok() { "success".to_string() } else { "error".to_string() },
            error: result.as_ref().err().map(|e| e.0.clone()),
            message: result.as_ref().err().map(|e| e.1.clone()),
        })
        .collect();

    let successful = batch_results.iter().filter(|r| r.status == "success").count();
    let failed = batch_results.len() - successful;

    HttpResponse::Ok().json(BatchResponse {
        status: if failed == 0 { "success".to_string() } else { "partial".to_string() },
        total: udids.len(),
        successful,
        failed,
        results: batch_results,
    })
}

// ═════════════════════════════════════════════════════════════════════════════
// Batch Helpers
// ═════════════════════════════════════════════════════════════════════════════

async fn execute_single_tap(
    state: &AppState,
    udid: &str,
    x: u32,
    y: u32,
) -> Result<(), (String, String)> {
    match get_device_client(state, udid).await {
        Ok((_device, client)) => {
            client.click(x as i32, y as i32).await.map_err(|e| ("ERR_OPERATION_FAILED".to_string(), e))
        }
        Err(_) => Err(("ERR_DEVICE_NOT_FOUND".to_string(), format!("Device {} not found", udid))),
    }
}

async fn execute_single_swipe(
    state: &AppState,
    udid: &str,
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
    duration: f64,
) -> Result<(), (String, String)> {
    match get_device_client(state, udid).await {
        Ok((_device, client)) => {
            client.swipe(x1 as i32, y1 as i32, x2 as i32, y2 as i32, duration)
                .await
                .map_err(|e| ("ERR_OPERATION_FAILED".to_string(), e))
        }
        Err(_) => Err(("ERR_DEVICE_NOT_FOUND".to_string(), format!("Device {} not found", udid))),
    }
}

async fn execute_single_input(
    state: &AppState,
    udid: &str,
    text: &str,
) -> Result<(), (String, String)> {
    match get_device_client(state, udid).await {
        Ok((_device, client)) => {
            client.input_text(text).await.map_err(|e| ("ERR_OPERATION_FAILED".to_string(), e))
        }
        Err(_) => Err(("ERR_DEVICE_NOT_FOUND".to_string(), format!("Device {} not found", udid))),
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Status & Health Endpoints (Story 5-3)
// ═════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/status - Get all device statuses summary
///
/// Returns summary of all devices with:
/// - Total count
/// - Count by status (connected, disconnected, error)
/// - Average battery level
/// - List of all devices with status
pub async fn get_device_status(state: web::Data<AppState>) -> HttpResponse {
    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());

    match phone_service.query_device_list_by_present().await {
        Ok(devices) => {
            let mut by_status: HashMap<String, usize> = HashMap::new();
            let mut total_battery: i64 = 0;
            let mut battery_count: usize = 0;
            let mut device_entries: Vec<DeviceStatusEntry> = Vec::new();

            for dev in &devices {
                let status = dev.get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                *by_status.entry(status.clone()).or_insert(0) += 1;

                let battery = dev.get("battery")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(-1);

                if battery >= 0 {
                    total_battery += battery;
                    battery_count += 1;
                }

                device_entries.push(DeviceStatusEntry {
                    udid: dev.get("udid").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    model: dev.get("model").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    status,
                    battery: battery as i32,
                    last_seen: dev.get("last_seen").and_then(|v| {
                        v.as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    }).map(|dt| chrono::DateTime::<chrono::Utc>::from(dt)),
                });
            }

            let average_battery = if battery_count > 0 {
                Some(total_battery as f32 / battery_count as f32)
            } else {
                None
            };

            let summary = DeviceStatusSummary {
                total: devices.len(),
                by_status,
                average_battery,
                devices: device_entries,
            };

            success_response(summary)
        }
        Err(e) => error_response("ERR_OPERATION_FAILED", &e),
    }
}

/// GET /api/v1/health - Health check for load balancers
///
/// Returns HTTP 200 if healthy, HTTP 503 if unhealthy.
/// Checks: database connectivity, connection pool status.
/// Target response time: < 50ms
pub async fn health_check(state: web::Data<AppState>) -> HttpResponse {
    let mut checks: HashMap<String, String> = HashMap::new();
    let mut is_healthy = true;
    let mut error_msg: Option<String> = None;

    // Check database connectivity
    let db_status = match state.db.query_device_list_by_present().await {
        Ok(_) => "ok".to_string(),
        Err(e) => {
            is_healthy = false;
            error_msg = Some(format!("Database error: {}", e));
            "error".to_string()
        }
    };
    checks.insert("database".to_string(), db_status);

    // Check connection pool status
    let pool_stats = state.connection_pool.stats();
    let pool_size = pool_stats.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let max_pool_size = pool_stats.get("max_size").and_then(|v| v.as_u64()).unwrap_or(1200) as usize;

    // Pool is unhealthy if at 95%+ capacity
    let pool_status = if max_pool_size > 0 && pool_size >= (max_pool_size * 95 / 100) {
        is_healthy = false;
        error_msg = Some("Connection pool near capacity".to_string());
        "warning".to_string()
    } else {
        "ok".to_string()
    };
    checks.insert("connectionPool".to_string(), pool_status);

    let response = HealthCheckResponse {
        status: if is_healthy { "healthy".to_string() } else { "unhealthy".to_string() },
        checks,
        pool_size: Some(pool_size),
        max_pool_size: Some(max_pool_size),
        error: error_msg,
        timestamp: chrono::Utc::now(),
    };

    if is_healthy {
        HttpResponse::Ok().json(response)
    } else {
        HttpResponse::ServiceUnavailable().json(response)
    }
}

/// GET /api/v1/metrics - Prometheus-compatible metrics
///
/// Returns metrics in Prometheus text format:
/// - connected_devices: Number of connected devices
/// - disconnected_devices: Number of disconnected devices
/// - error_devices: Number of devices in error state
/// - websocket_connections: Active WebSocket connections
/// - pool_size: Current connection pool size
/// - screenshot_latency_seconds: Screenshot capture latency percentiles
pub async fn get_metrics(state: web::Data<AppState>) -> HttpResponse {
    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());

    let devices = phone_service.query_device_list_by_present().await.unwrap_or_default();

    // Count by status
    let mut connected = 0usize;
    let mut disconnected = 0usize;
    let mut error_count = 0usize;

    for dev in &devices {
        let status = dev.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
        match status {
            "connected" | "online" => connected += 1,
            "disconnected" | "offline" => disconnected += 1,
            _ => error_count += 1,
        }
    }

    // Get pool stats
    let pool_stats = state.connection_pool.stats();
    let pool_size = pool_stats.get("total").and_then(|v| v.as_u64()).unwrap_or(0);

    // Get WebSocket connections from metrics tracker
    let ws_connections = state.metrics.get_ws_count() as u64;

    // Get screenshot latency percentiles
    let p50 = state.metrics.get_latency_percentile(0.5).unwrap_or(0.0);
    let p90 = state.metrics.get_latency_percentile(0.9).unwrap_or(0.0);
    let p95 = state.metrics.get_latency_percentile(0.95).unwrap_or(0.0);
    let p99 = state.metrics.get_latency_percentile(0.99).unwrap_or(0.0);

    // Build Prometheus text format
    let metrics = format!(
        r#"# HELP cloudcontrol_connected_devices Number of currently connected devices
# TYPE cloudcontrol_connected_devices gauge
cloudcontrol_connected_devices {}

# HELP cloudcontrol_disconnected_devices Number of disconnected devices
# TYPE cloudcontrol_disconnected_devices gauge
cloudcontrol_disconnected_devices {}

# HELP cloudcontrol_error_devices Number of devices in error state
# TYPE cloudcontrol_error_devices gauge
cloudcontrol_error_devices {}

# HELP cloudcontrol_websocket_connections Active WebSocket connections
# TYPE cloudcontrol_websocket_connections gauge
cloudcontrol_websocket_connections {}

# HELP cloudcontrol_pool_size Current connection pool size
# TYPE cloudcontrol_pool_size gauge
cloudcontrol_pool_size {}

# HELP cloudcontrol_screenshot_latency_seconds Screenshot capture latency
# TYPE cloudcontrol_screenshot_latency_seconds summary
cloudcontrol_screenshot_latency_seconds{{quantile="0.5"}} {}
cloudcontrol_screenshot_latency_seconds{{quantile="0.9"}} {}
cloudcontrol_screenshot_latency_seconds{{quantile="0.95"}} {}
cloudcontrol_screenshot_latency_seconds{{quantile="0.99"}} {}
"#,
        connected, disconnected, error_count, ws_connections, pool_size, p50, p90, p95, p99
    );

    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(metrics)
}

// ═════════════════════════════════════════════════════════════════════════════
// OpenAPI Specification
// ═════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/openapi.json - OpenAPI 3.0 specification
pub async fn openapi_spec() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .json(json!({
            "openapi": "3.0.0",
            "info": {
                "title": "CloudControl Rust API",
                "version": "1.0.0",
                "description": "REST API for Android device control and automation"
            },
            "servers": [{"url": "http://localhost:8000", "description": "Development server"}],
            "paths": {
                "/api/v1/devices": {
                    "get": {"summary": "List all connected devices", "operationId": "listDevices"}
                },
                "/api/v1/devices/{udid}": {
                    "get": {"summary": "Get device information", "operationId": "getDevice"}
                },
                "/api/v1/devices/{udid}/screenshot": {
                    "get": {"summary": "Get device screenshot", "operationId": "getScreenshot"}
                },
                "/api/v1/devices/{udid}/tap": {
                    "post": {"summary": "Execute tap command", "operationId": "tap"}
                },
                "/api/v1/devices/{udid}/swipe": {
                    "post": {"summary": "Execute swipe gesture", "operationId": "swipe"}
                },
                "/api/v1/devices/{udid}/input": {
                    "post": {"summary": "Input text to device", "operationId": "input"}
                },
                "/api/v1/devices/{udid}/keyevent": {
                    "post": {"summary": "Send key event", "operationId": "keyevent"}
                },
                "/api/v1/batch/tap": {
                    "post": {"summary": "Batch tap operation", "operationId": "batchTap"}
                },
                "/api/v1/batch/swipe": {
                    "post": {"summary": "Batch swipe operation", "operationId": "batchSwipe"}
                },
                "/api/v1/batch/input": {
                    "post": {"summary": "Batch input operation", "operationId": "batchInput"}
                },
                "/api/v1/ws/screenshot/{udid}": {
                    "get": {
                        "summary": "WebSocket screenshot streaming",
                        "description": "Binary JPEG screenshot stream with JSON-RPC 2.0 control",
                        "operationId": "wsScreenshot"
                    }
                }
            }
        }))
}

// ═════════════════════════════════════════════════════════════════════════════
// WebSocket Screenshot Streaming API
// ═════════════════════════════════════════════════════════════════════════════

/// JSON-RPC 2.0 request structure
#[derive(Debug, serde::Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
    id: u64,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, serde::Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: u64,
}

#[derive(Debug, serde::Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// Stream settings for WebSocket screenshot streaming
struct StreamSettings {
    quality: u8,
    scale: f64,
    interval_ms: u64,
}

impl Default for StreamSettings {
    fn default() -> Self {
        Self {
            quality: 40,
            scale: 0.5,
            interval_ms: 50,
        }
    }
}

/// GET /api/v1/ws/screenshot/{udid} - WebSocket screenshot streaming
///
/// Protocol:
/// - Binary frames: JPEG images (raw bytes, not base64)
/// - Text frames: JSON-RPC 2.0 commands and responses
///
/// JSON-RPC Methods:
/// - start: Start streaming screenshots
/// - stop: Stop streaming (connection remains open)
/// - setQuality(quality: 1-100): Set JPEG compression quality
/// - setScale(scale: 0.1-1.0): Set screenshot scale
/// - setInterval(interval: 30-5000ms): Set frame interval
/// - getStatus: Get current stream settings
pub async fn ws_screenshot(
    state: web::Data<AppState>,
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    // Validate device exists before WebSocket upgrade
    let (device, client) = match get_device_client_for_ws(&state, &udid).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };

    // Upgrade to WebSocket
    let (resp, mut session, mut msg_stream) = match actix_ws::handle(&req, stream) {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("WebSocket upgrade failed: {}", e)
            }));
        }
    };

    // Spawn streaming task
    actix_web::rt::spawn(async move {
        let serial = device
            .get("serial")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let is_usb = Adb::is_usb_serial(&serial);

        // Stream settings with Arc for thread-safe sharing
        let settings = Arc::new(tokio::sync::RwLock::new(StreamSettings::default()));
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let streaming = Arc::new(std::sync::atomic::AtomicBool::new(true));

        // Clone for streaming task
        let settings_clone = settings.clone();
        let running_clone = running.clone();
        let streaming_clone = streaming.clone();
        let client_clone = client.clone();
        let mut session_clone = session.clone();
        let udid_clone = udid.clone();

        // Spawn screenshot streaming task
        let stream_handle = tokio::spawn(async move {
            let mut frame_count: u64 = 0;
            let stream_start = std::time::Instant::now();

            while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                // Check if streaming is enabled
                if !streaming_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }

                let start = std::time::Instant::now();

                // Get current settings
                let (quality, scale, interval_ms) = {
                    let s = settings_clone.read().await;
                    (s.quality, s.scale, s.interval_ms)
                };

                // Capture screenshot using AtxClient
                let result = client_clone
                    .screenshot_scaled(scale, quality)
                    .await
                    .or_else(|_| -> Result<Vec<u8>, String> {
                        Err("u2 screenshot failed".into())
                    });

                // Fallback to ADB for USB devices
                let result = match result {
                    Ok(bytes) => Ok(bytes),
                    Err(_) if is_usb && !serial.is_empty() => {
                        tracing::debug!("[API-V1-WS] u2 failed, falling back to ADB screencap");
                        DeviceService::screenshot_usb_jpeg(&serial, quality, scale).await
                    }
                    Err(e) => Err(e),
                };

                match result {
                    Ok(jpeg_bytes) => {
                        // Send binary JPEG frame
                        if session_clone.binary(jpeg_bytes).await.is_err() {
                            break;
                        }

                        frame_count += 1;
                        let total = start.elapsed();

                        // Log every 20 frames
                        if frame_count % 20 == 0 {
                            let avg_fps = frame_count as f64 / stream_start.elapsed().as_secs_f64();
                            tracing::debug!(
                                "[API-V1-WS] {} frame#{} | {:.0}ms | avg {:.1}fps",
                                udid_clone,
                                frame_count,
                                total.as_secs_f64() * 1000.0,
                                avg_fps,
                            );
                        }
                    }
                    Err(e) => {
                        // Device disconnected - send event and close
                        tracing::error!("[API-V1-WS] Screenshot error: {}", e);

                        // Send device_disconnected event
                        let event = json!({
                            "event": "device_disconnected",
                            "udid": udid_clone
                        });
                        let _ = session_clone.text(event.to_string()).await;

                        // Close WebSocket gracefully
                        let _ = session_clone.close(Some(actix_ws::CloseCode::Away.into())).await;
                        running_clone.store(false, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                }

                // Smart interval: only sleep remaining time
                let elapsed = start.elapsed();
                let min_interval = Duration::from_millis(interval_ms.max(30));
                if elapsed < min_interval {
                    tokio::time::sleep(min_interval - elapsed).await;
                }
            }
        });

        // Handle incoming messages (JSON-RPC commands)
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                actix_ws::Message::Text(text) => {
                    // Parse JSON-RPC request
                    let rpc_req: Result<JsonRpcRequest, _> = serde_json::from_str(&text);

                    match rpc_req {
                        Ok(req) => {
                            // Validate JSON-RPC version
                            if req.jsonrpc != "2.0" {
                                let error_resp = JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: -32600,
                                        message: "Invalid Request: jsonrpc must be '2.0'".to_string(),
                                    }),
                                    id: req.id,
                                };
                                let _ = session.text(serde_json::to_string(&error_resp).unwrap()).await;
                                continue;
                            }

                            // Handle JSON-RPC methods
                            let response = match req.method.as_str() {
                                "start" => {
                                    streaming.store(true, std::sync::atomic::Ordering::Relaxed);

                                    // Send stream_started event
                                    let _ = session.text(json!({"event": "stream_started"}).to_string()).await;

                                    JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        result: Some("ok".to_string()),
                                        error: None,
                                        id: req.id,
                                    }
                                }

                                "stop" => {
                                    streaming.store(false, std::sync::atomic::Ordering::Relaxed);

                                    // Send stream_stopped event
                                    let _ = session.text(json!({"event": "stream_stopped"}).to_string()).await;

                                    JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        result: Some("ok".to_string()),
                                        error: None,
                                        id: req.id,
                                    }
                                }

                                "setQuality" => {
                                    let quality = req.params.get("quality")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(40) as u8;

                                    // Validate range
                                    if quality < 1 || quality > 100 {
                                        JsonRpcResponse {
                                            jsonrpc: "2.0".to_string(),
                                            result: None,
                                            error: Some(JsonRpcError {
                                                code: -32602,
                                                message: "Invalid params: quality must be 1-100".to_string(),
                                            }),
                                            id: req.id,
                                        }
                                    } else {
                                        let mut s = settings.write().await;
                                        s.quality = quality;
                                        drop(s);

                                        JsonRpcResponse {
                                            jsonrpc: "2.0".to_string(),
                                            result: Some("ok".to_string()),
                                            error: None,
                                            id: req.id,
                                        }
                                    }
                                }

                                "setScale" => {
                                    let scale = req.params.get("scale")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(0.5);

                                    // Validate range
                                    if scale < 0.1 || scale > 1.0 {
                                        JsonRpcResponse {
                                            jsonrpc: "2.0".to_string(),
                                            result: None,
                                            error: Some(JsonRpcError {
                                                code: -32602,
                                                message: "Invalid params: scale must be 0.1-1.0".to_string(),
                                            }),
                                            id: req.id,
                                        }
                                    } else {
                                        let mut s = settings.write().await;
                                        s.scale = scale;
                                        drop(s);

                                        JsonRpcResponse {
                                            jsonrpc: "2.0".to_string(),
                                            result: Some("ok".to_string()),
                                            error: None,
                                            id: req.id,
                                        }
                                    }
                                }

                                "setInterval" => {
                                    let interval = req.params.get("interval")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(50);

                                    // Validate range
                                    if interval < 30 || interval > 5000 {
                                        JsonRpcResponse {
                                            jsonrpc: "2.0".to_string(),
                                            result: None,
                                            error: Some(JsonRpcError {
                                                code: -32602,
                                                message: "Invalid params: interval must be 30-5000ms".to_string(),
                                            }),
                                            id: req.id,
                                        }
                                    } else {
                                        let mut s = settings.write().await;
                                        s.interval_ms = interval;
                                        drop(s);

                                        JsonRpcResponse {
                                            jsonrpc: "2.0".to_string(),
                                            result: Some("ok".to_string()),
                                            error: None,
                                            id: req.id,
                                        }
                                    }
                                }

                                "getStatus" => {
                                    let s = settings.read().await;
                                    let status = json!({
                                        "quality": s.quality,
                                        "scale": s.scale,
                                        "interval": s.interval_ms,
                                        "streaming": streaming.load(std::sync::atomic::Ordering::Relaxed)
                                    });

                                    // Send status as text message
                                    let _ = session.text(json!({
                                        "event": "status",
                                        "data": status
                                    }).to_string()).await;

                                    JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        result: Some("ok".to_string()),
                                        error: None,
                                        id: req.id,
                                    }
                                }

                                _ => {
                                    JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        result: None,
                                        error: Some(JsonRpcError {
                                            code: -32601,
                                            message: format!("Method not found: {}", req.method),
                                        }),
                                        id: req.id,
                                    }
                                }
                            };

                            // Send JSON-RPC response
                            let _ = session.text(serde_json::to_string(&response).unwrap()).await;
                        }
                        Err(_) => {
                            // Invalid JSON
                            let _ = session.text(json!({
                                "event": "error",
                                "code": "ERR_INVALID_JSON",
                                "message": "Invalid JSON-RPC request"
                            }).to_string()).await;
                        }
                    }
                }

                actix_ws::Message::Close(_) => {
                    running.store(false, std::sync::atomic::Ordering::Relaxed);
                    break;
                }

                actix_ws::Message::Ping(data) => {
                    let _ = session.pong(&data).await;
                }

                _ => {}
            }
        }

        // Cleanup
        running.store(false, std::sync::atomic::Ordering::Relaxed);
        stream_handle.abort();

        tracing::info!("[API-V1-WS] WebSocket session closed: {}", udid);
    });

    resp
}

/// Helper to get device client for WebSocket (returns HTTP error response if not found)
async fn get_device_client_for_ws(
    state: &AppState,
    udid: &str,
) -> Result<(serde_json::Value, Arc<AtxClient>), HttpResponse> {
    // Try device info cache first
    if let Some(cached) = state.device_info_cache.get(udid).await {
        let ip = cached.get("ip").and_then(|v| v.as_str()).unwrap_or("");
        let port = cached.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
        let (final_ip, final_port) = resolve_device_connection(&cached, ip, port).await;
        let client = state.connection_pool.get_or_create(udid, &final_ip, final_port).await;
        return Ok((cached, client));
    }

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
    let device = phone_service
        .query_info_by_udid(udid)
        .await
        .map_err(|e| {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_DEVICE_NOT_FOUND",
                "message": e
            }))
        })?
        .ok_or_else(|| {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_DEVICE_NOT_FOUND",
                "message": format!("Device '{}' not found", udid)
            }))
        })?;

    state.device_info_cache.insert(udid.to_string(), device.clone()).await;

    let ip = device.get("ip").and_then(|v| v.as_str()).unwrap_or("");
    let port = device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
    let (final_ip, final_port) = resolve_device_connection(&device, ip, port).await;
    let client = state.connection_pool.get_or_create(udid, &final_ip, final_port).await;
    Ok((device, client))
}
