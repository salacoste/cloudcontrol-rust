use crate::device::adb::Adb;
use crate::device::atx_client::AtxClient;
use crate::middleware::{OptionalAuth, RequireAnyRole};
use crate::models::api_response::{
    ApiResponse, BatchResponse, BatchResult, DeviceInfo, DeviceInfoResponse,
    ScreenshotResponse, TapRequest, SwipeRequest, InputRequest, KeyEventRequest,
    BatchTapRequest, BatchSwipeRequest, BatchInputRequest, DisplayInfo, ERR_NO_DEVICES_SELECTED,
    DeviceStatusSummary, DeviceStatusEntry, HealthCheckResponse,
};
use crate::services::device_resolver::DeviceResolver;
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

/// Get device client from state using shared DeviceResolver (Story 13-1)
async fn get_device_client(
    state: &AppState,
    udid: &str,
) -> Result<(serde_json::Value, Arc<AtxClient>), HttpResponse> {
    DeviceResolver::new(state)
        .get_device_client(udid)
        .await
        .map_err(|e| e.into())
}

/// Create standardized error response
fn error_response(code: &str, message: &str) -> HttpResponse {
    let status = match code {
        "ERR_DEVICE_NOT_FOUND" | "ERR_RECORDING_NOT_FOUND" | "ERR_FILE_NOT_FOUND" => actix_web::http::StatusCode::NOT_FOUND,
        "ERR_DEVICE_DISCONNECTED" | "ERR_SERVICE_UNAVAILABLE" => actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
        "ERR_INVALID_REQUEST" | "ERR_NO_DEVICES_SELECTED" => actix_web::http::StatusCode::BAD_REQUEST,
        "ERR_RECORDING_ACTIVE" | "ERR_RECORDING_NOT_READY" => actix_web::http::StatusCode::CONFLICT,
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


// ═════════════════════════════════════════════════════════════════════════════
// Device Endpoints
// ═════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/devices - List all connected devices
/// Supports optional authentication for team-scoped filtering (Story 14-3)
pub async fn list_devices(
    auth: OptionalAuth,
    state: web::Data<AppState>,
) -> HttpResponse {
    let phone_service = state.phone_service.clone();

    // Get the user's team_id for filtering (if authenticated)
    let user_team_id = auth.user.as_ref().and_then(|u| u.team_id.clone());

    match phone_service.query_device_list_by_present().await {
        Ok(devices) => {
            let device_infos: Vec<DeviceInfo> = devices.iter()
                .filter_map(|dev| {
                    // Get device team_id from the device data
                    let device_team_id = dev.get("team_id").and_then(|v| v.as_str()).map(|s| s.to_string());

                    // Filter by team if user is authenticated and not an admin
                    if let Some(ref_team_id) = &user_team_id {
                        // Non-admin users only see devices in their team
                        if device_team_id.as_ref() != Some(ref_team_id) {
                            return None; // Skip this device
                        }
                    }
                    // If user is admin (no team_id) or not authenticated, show all devices

                    let display = dev.get("display").cloned().unwrap_or(json!({"width": 1080, "height": 1920}));
                    Some(DeviceInfo {
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
                        team_id: device_team_id,
                    })
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
                team_id: device.get("team_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
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
            let start = std::time::Instant::now();
            let result = client.screenshot_base64_direct().await;
            let elapsed = start.elapsed().as_secs_f64();
            state.metrics.record_screenshot_latency(elapsed);

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
/// Requires: Agent, Admin, or Renter role (Viewer excluded - AC: Role-based device visibility)
pub async fn tap(
    _auth: RequireAnyRole,
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
/// Requires: Agent, Admin, or Renter role (Viewer excluded - AC: Role-based device visibility)
pub async fn swipe(
    _auth: RequireAnyRole,
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
/// Requires: Agent, Admin, or Renter role (Viewer excluded - AC: Role-based device visibility)
pub async fn input(
    _auth: RequireAnyRole,
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
/// Requires: Agent, Admin, or Renter role (Viewer excluded - AC: Role-based device visibility)
pub async fn keyevent(
    _auth: RequireAnyRole,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<KeyEventRequest>,
) -> HttpResponse {
    let udid = path.into_inner();

    if body.key.is_empty() {
        return error_response("ERR_INVALID_REQUEST", "Key cannot be empty");
    }

    // Validate key name to prevent shell injection (alphanumeric, underscore, hyphen only)
    if !body.key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return error_response("ERR_INVALID_REQUEST", "Key contains invalid characters");
    }

    match get_device_client(&state, &udid).await {
        Ok((device, client)) => {
            // Try ATX press_key first, fall back to ADB input keyevent
            match client.press_key(&body.key).await {
                Ok(_) => success_response(json!({"executed": true, "key": body.key})),
                Err(e) => {
                    tracing::warn!("[KEYEVENT] ATX press_key failed for {}: {}, trying ADB fallback", udid, e);
                    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or(&udid);
                    let adb_key = format!("KEYCODE_{}", body.key.to_uppercase());
                    match Adb::shell(serial, &format!("input keyevent {}", adb_key)).await {
                        Ok(_) => success_response(json!({"executed": true, "key": body.key, "fallback": "adb"})),
                        Err(adb_err) => error_response("ERR_OPERATION_FAILED", &format!("ATX: {} | ADB: {}", e, adb_err)),
                    }
                }
            }
        }
        Err(response) => response,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Batch Endpoints
// ═════════════════════════════════════════════════════════════════════════════

/// POST /api/v1/batch/tap - Batch tap
/// Requires: Agent, Admin, or Renter role (Viewer excluded - AC: Role-based device visibility)
pub async fn batch_tap(
    _auth: RequireAnyRole,
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
/// Requires: Agent, Admin, or Renter role (Viewer excluded - AC: Role-based device visibility)
pub async fn batch_swipe(
    _auth: RequireAnyRole,
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
/// Requires: Agent, Admin, or Renter role (Viewer excluded - AC: Role-based device visibility)
pub async fn batch_input(
    _auth: RequireAnyRole,
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
    let phone_service = state.phone_service.clone();

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
    let phone_service = state.phone_service.clone();

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
    let spec = crate::models::openapi::generate_openapi_spec();
    HttpResponse::Ok()
        .content_type("application/json")
        .json(spec)
}

// ═════════════════════════════════════════════════════════════════════════════
// Server Version
// ═════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/version — server version info
pub async fn get_version() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "data": {
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
            "server": "cloudcontrol-rust"
        }
    }))
}

// ═════════════════════════════════════════════════════════════════════════════
// Product Catalog API
// ═════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/products — list all products with optional ?brand=X&model=Y filters
pub async fn list_products(
    state: web::Data<AppState>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let brand_filter = query.get("brand").map(|s| s.as_str()).filter(|s| !s.is_empty());
    let model_filter = query.get("model").map(|s| s.as_str()).filter(|s| !s.is_empty());

    match state.db.list_products(brand_filter, model_filter).await {
        Ok(products) => HttpResponse::Ok().json(json!({
            "status": "success",
            "data": products
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_DATABASE",
            "message": format!("Failed to list products: {}", e)
        })),
    }
}

/// GET /api/v1/products/{id} — get single product by ID
pub async fn get_product(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let id = path.into_inner();
    match state.db.get_product(id).await {
        Ok(Some(product)) => HttpResponse::Ok().json(json!({
            "status": "success",
            "data": product
        })),
        Ok(None) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_PRODUCT_NOT_FOUND",
            "message": format!("Product {} not found", id)
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_DATABASE",
            "message": format!("Failed to get product: {}", e)
        })),
    }
}

/// POST /api/v1/products — create product from JSON body
pub async fn create_product(
    state: web::Data<AppState>,
    body: web::Json<crate::models::product::CreateProductRequest>,
) -> HttpResponse {
    if body.brand.trim().is_empty() || body.model.trim().is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "Brand and model are required"
        }));
    }

    let brand = body.brand.trim();
    let model = body.model.trim();

    match state.db.create_product(
        brand,
        model,
        body.name.as_deref(),
        body.cpu.as_deref(),
        body.gpu.as_deref(),
        body.link.as_deref(),
        body.coverage,
    ).await {
        Ok(product) => HttpResponse::Created().json(json!({
            "status": "success",
            "data": product
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_DATABASE",
            "message": format!("Failed to create product: {}", e)
        })),
    }
}

/// PUT /api/v1/products/{id} — update product from JSON body
pub async fn update_product(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: web::Json<crate::models::product::UpdateProductRequest>,
) -> HttpResponse {
    let id = path.into_inner();

    // Validate brand/model are not empty if provided
    if let Some(ref brand) = body.brand {
        if brand.trim().is_empty() {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": "Brand cannot be empty"
            }));
        }
    }
    if let Some(ref model) = body.model {
        if model.trim().is_empty() {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": "Model cannot be empty"
            }));
        }
    }

    match state.db.update_product(
        id,
        body.brand.as_deref(),
        body.model.as_deref(),
        body.name.as_deref(),
        body.cpu.as_deref(),
        body.gpu.as_deref(),
        body.link.as_deref(),
        body.coverage,
    ).await {
        Ok(Some(product)) => HttpResponse::Ok().json(json!({
            "status": "success",
            "data": product
        })),
        Ok(None) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_PRODUCT_NOT_FOUND",
            "message": format!("Product {} not found", id)
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_DATABASE",
            "message": format!("Failed to update product: {}", e)
        })),
    }
}

/// DELETE /api/v1/products/{id} — delete product by ID
pub async fn delete_product(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let id = path.into_inner();
    match state.db.delete_product(id).await {
        Ok(true) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": format!("Product {} deleted", id)
        })),
        Ok(false) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_PRODUCT_NOT_FOUND",
            "message": format!("Product {} not found", id)
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_DATABASE",
            "message": format!("Failed to delete product: {}", e)
        })),
    }
}

/// GET /products/{brand}/{model} — legacy endpoint for edit.html compatibility
pub async fn list_products_by_brand_model(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (brand, model) = path.into_inner();
    match state.db.list_products_by_brand_model(&brand, &model).await {
        Ok(products) => HttpResponse::Ok().json(products),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_DATABASE",
            "message": format!("Failed to list products: {}", e)
        })),
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Provider Registry API
// ═════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/providers — list all providers with device counts
pub async fn list_providers(state: web::Data<AppState>) -> HttpResponse {
    match state.db.list_providers().await {
        Ok(providers) => {
            let mut providers_with_devices = Vec::new();
            for provider in providers {
                let devices = state
                    .db
                    .list_devices_by_provider(&provider.ip)
                    .await
                    .unwrap_or_default();
                let device_count = devices.len() as i64;
                providers_with_devices.push(
                    crate::models::provider::ProviderWithDevices {
                        provider,
                        device_count,
                        devices,
                    },
                );
            }
            HttpResponse::Ok().json(json!({
                "status": "success",
                "data": providers_with_devices
            }))
        }
        Err(e) => {
            tracing::warn!("Failed to list providers: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_DATABASE",
                "message": format!("Failed to list providers: {}", e)
            }))
        }
    }
}

/// POST /api/v1/providers — register a new provider
pub async fn create_provider(
    state: web::Data<AppState>,
    body: web::Json<crate::models::provider::CreateProviderRequest>,
) -> HttpResponse {
    let ip = body.ip.trim();
    if ip.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "IP address is required"
        }));
    }

    let notes = body.notes.as_deref().map(|n| n.trim());
    match state
        .db
        .create_provider(ip, notes)
        .await
    {
        Ok(provider) => HttpResponse::Created().json(json!({
            "status": "success",
            "data": provider
        })),
        Err(e) => {
            let err_msg = e.to_string();
            if err_msg.contains("UNIQUE constraint failed") {
                HttpResponse::Conflict().json(json!({
                    "status": "error",
                    "error": "ERR_DUPLICATE_IP",
                    "message": format!("Provider with IP '{}' already exists", ip)
                }))
            } else {
                tracing::warn!("Failed to create provider: {}", e);
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "error": "ERR_DATABASE",
                    "message": format!("Failed to create provider: {}", e)
                }))
            }
        }
    }
}

/// GET /api/v1/providers/{id} — get a single provider
pub async fn get_provider(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let id = path.into_inner();
    match state.db.get_provider(id).await {
        Ok(Some(provider)) => {
            let devices = state
                .db
                .list_devices_by_provider(&provider.ip)
                .await
                .unwrap_or_default();
            let device_count = devices.len() as i64;
            HttpResponse::Ok().json(json!({
                "status": "success",
                "data": crate::models::provider::ProviderWithDevices {
                    provider,
                    device_count,
                    devices,
                }
            }))
        }
        Ok(None) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_PROVIDER_NOT_FOUND",
            "message": "Provider not found"
        })),
        Err(e) => {
            tracing::warn!("Failed to get provider {}: {}", id, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_DATABASE",
                "message": format!("Failed to get provider: {}", e)
            }))
        }
    }
}

/// PUT /api/v1/providers/{id} — update provider notes
pub async fn update_provider(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: String,
) -> HttpResponse {
    let id = path.into_inner();

    // Parse JSON body manually — jQuery $.ajax sends without Content-Type header
    let body: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": "Invalid JSON body"
            }));
        }
    };

    let notes = body
        .get("notes")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    match state.db.update_provider_notes(id, notes).await {
        Ok(Some(provider)) => {
            let devices = state
                .db
                .list_devices_by_provider(&provider.ip)
                .await
                .unwrap_or_default();
            let device_count = devices.len() as i64;
            HttpResponse::Ok().json(json!({
                "status": "success",
                "data": crate::models::provider::ProviderWithDevices {
                    provider,
                    device_count,
                    devices,
                }
            }))
        }
        Ok(None) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_PROVIDER_NOT_FOUND",
            "message": "Provider not found"
        })),
        Err(e) => {
            tracing::warn!("Failed to update provider {}: {}", id, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_DATABASE",
                "message": format!("Failed to update provider: {}", e)
            }))
        }
    }
}

/// POST /api/v1/providers/{id}/heartbeat — provider heartbeat keep-alive
pub async fn provider_heartbeat(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let id = path.into_inner();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let heartbeats = state.provider_heartbeats.clone();
    let timeout = now + 60.0;

    if heartbeats.get(&id).is_some() {
        // Existing session — just reset timer, no DB write needed
        heartbeats.insert(id, timeout);

        // Fetch current provider data without updating presence_changed_at
        match state.db.get_provider(id).await {
            Ok(Some(provider)) => {
                let devices = state
                    .db
                    .list_devices_by_provider(&provider.ip)
                    .await
                    .unwrap_or_default();
                let device_count = devices.len() as i64;

                HttpResponse::Ok().json(json!({
                    "status": "success",
                    "data": crate::models::provider::ProviderWithDevices {
                        provider,
                        device_count,
                        devices,
                    }
                }))
            }
            Ok(None) => {
                heartbeats.remove(&id);
                HttpResponse::NotFound().json(json!({
                    "status": "error",
                    "error": "ERR_PROVIDER_NOT_FOUND",
                    "message": "Provider not found"
                }))
            }
            Err(e) => {
                tracing::warn!("Failed to fetch provider {}: {}", id, e);
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "error": "ERR_DATABASE",
                    "message": format!("Failed to process heartbeat: {}", e)
                }))
            }
        }
    } else {
        // New session — update presence to online and spawn timeout checker
        match state.db.update_provider_presence(id, true).await {
            Ok(Some(provider)) => {
                heartbeats.insert(id, timeout);

                let db = state.db.clone();
                let hb = heartbeats.clone();
                let provider_id = id;
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs_f64();

                        let expired = hb
                            .get(&provider_id)
                            .map(|t| *t < now)
                            .unwrap_or(true);

                        if expired {
                            hb.remove(&provider_id);
                            if let Err(e) =
                                db.update_provider_presence(provider_id, false).await
                            {
                                tracing::warn!(
                                    "Failed to mark provider {} offline: {}",
                                    provider_id,
                                    e
                                );
                            }
                            return;
                        }
                    }
                });

                let devices = state
                    .db
                    .list_devices_by_provider(&provider.ip)
                    .await
                    .unwrap_or_default();
                let device_count = devices.len() as i64;

                HttpResponse::Ok().json(json!({
                    "status": "success",
                    "data": crate::models::provider::ProviderWithDevices {
                        provider,
                        device_count,
                        devices,
                    }
                }))
            }
            Ok(None) => HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_PROVIDER_NOT_FOUND",
                "message": "Provider not found"
            })),
            Err(e) => {
                tracing::warn!("Failed to process provider heartbeat {}: {}", id, e);
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "error": "ERR_DATABASE",
                    "message": format!("Failed to process heartbeat: {}", e)
                }))
            }
        }
    }
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
    result: Option<serde_json::Value>,
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

// ═════════════════════════════════════════════════════════════════════════════
// Hierarchy, Upload, Rotation (Story 10-4)
// ═════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/devices/{udid}/hierarchy - Get UI hierarchy tree
pub async fn hierarchy(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Mock device handling
    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return success_response(json!({
            "id": "mock-root",
            "className": "android.widget.FrameLayout",
            "text": "",
            "resourceId": "",
            "description": "",
            "rect": {"x": 0, "y": 0, "width": 1080, "height": 2400},
            "clickable": false,
            "enabled": true,
            "children": [
                {
                    "id": "mock-button",
                    "className": "android.widget.Button",
                    "text": "Mock Button",
                    "resourceId": "com.app:id/button",
                    "description": "",
                    "rect": {"x": 100, "y": 200, "width": 200, "height": 50},
                    "clickable": true,
                    "enabled": true,
                    "children": []
                }
            ]
        }));
    }

    match DeviceService::dump_hierarchy(&client).await {
        Ok(h) => success_response(h),
        Err(e) => error_response("ERR_OPERATION_FAILED", &format!("Hierarchy dump failed: {}", e)),
    }
}

/// POST /api/v1/devices/{udid}/upload - Upload file to device
pub async fn upload(
    state: web::Data<AppState>,
    path: web::Path<String>,
    mut payload: actix_multipart::Multipart,
) -> HttpResponse {
    let udid = path.into_inner();

    let (_device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    const MAX_UPLOAD_SIZE: usize = 100 * 1024 * 1024; // 100 MB

    while let Some(Ok(mut field)) = payload.next().await {
        let raw_filename = field
            .content_disposition()
            .and_then(|cd| cd.get_filename().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        // Sanitize: strip path components to prevent directory traversal
        let filename = std::path::Path::new(&raw_filename)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Read file data with size limit
        let mut data = Vec::new();
        while let Some(Ok(chunk)) = field.next().await {
            data.extend_from_slice(&chunk);
            if data.len() > MAX_UPLOAD_SIZE {
                return error_response(
                    "ERR_INVALID_REQUEST",
                    &format!("File too large (max {} MB)", MAX_UPLOAD_SIZE / 1024 / 1024),
                );
            }
        }

        // Determine device path by extension
        let ext = filename
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase();
        let device_path = match ext.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "webp" => format!("/sdcard/DCIM/{}", filename),
            "mp4" | "avi" | "mov" | "mkv" => format!("/sdcard/Movies/{}", filename),
            _ => format!("/sdcard/Download/{}", filename),
        };

        // Push to device
        if let Err(e) = client.push_file(&device_path, data, &filename).await {
            return error_response("ERR_OPERATION_FAILED", &format!("Upload failed: {}", e));
        }

        // If image, trigger media scan
        if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp") {
            let _ = client
                .shell_cmd(&format!(
                    "am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file://{}",
                    device_path
                ))
                .await;
        }

        return success_response(json!({
            "message": format!("File uploaded to: {}", device_path),
            "path": device_path,
        }));
    }

    error_response("ERR_INVALID_REQUEST", "No file uploaded")
}

/// POST /api/v1/devices/{udid}/rotation - Fix device rotation via ATX agent
pub async fn rotation(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    let (_device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let url = format!("{}/info/rotation", client.base_url());
    match client.http_client().post(&url).send().await {
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            let data: serde_json::Value = serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
            success_response(data)
        }
        Err(e) => {
            tracing::warn!("[ROTATION] v1 failed for {}: {}", udid, e);
            error_response("ERR_OPERATION_FAILED", &format!("Rotation fix failed: {}", e))
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Video Recording Management (Story 11-1)
// ═════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/videos - List all video recordings (with optional ?udid= and ?status= filters)
pub async fn list_videos(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> HttpResponse {
    let query = web::Query::<std::collections::HashMap<String, String>>::from_query(
        req.query_string(),
    )
    .unwrap_or_else(|_| web::Query(std::collections::HashMap::new()));
    let udid = query.get("udid").map(|s| s.as_str());
    let status = query.get("status").map(|s| s.as_str());
    let recordings = state.video_service.list_recordings(udid, status).await;
    success_response(json!(recordings))
}

/// GET /api/v1/videos/{id} - Get video recording metadata
pub async fn get_video(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();
    match state.video_service.get_recording(&id).await {
        Some(info) => success_response(json!(info)),
        None => error_response("ERR_RECORDING_NOT_FOUND", "Video recording not found"),
    }
}

/// GET /api/v1/videos/{id}/download - Download video file
pub async fn download_video(
    state: web::Data<AppState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let id = path.into_inner();
    match state.video_service.get_recording(&id).await {
        Some(info) => {
            if info.status != "completed" && info.status != "recovered" {
                return error_response(
                    "ERR_RECORDING_NOT_READY",
                    &format!("Recording is not ready for download (status: {})", info.status),
                );
            }
            let file_path = std::path::PathBuf::from(&info.file_path);
            match actix_files::NamedFile::open(&file_path) {
                Ok(file) => {
                    let filename = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("recording.mp4")
                        .to_string();
                    file.set_content_disposition(actix_web::http::header::ContentDisposition {
                        disposition: actix_web::http::header::DispositionType::Attachment,
                        parameters: vec![actix_web::http::header::DispositionParam::Filename(filename)],
                    })
                    .into_response(&req)
                }
                Err(_) => error_response("ERR_FILE_NOT_FOUND", "Recording file not found on disk"),
            }
        }
        None => error_response("ERR_RECORDING_NOT_FOUND", "Video recording not found"),
    }
}

/// DELETE /api/v1/videos/{id} - Delete video recording
pub async fn delete_video(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();
    match state.video_service.delete_recording(&id).await {
        Ok(()) => success_response(json!({"message": "Recording deleted"})),
        Err(e) => {
            let status = if e == "ERR_RECORDING_NOT_FOUND" { 404 } else { 409 };
            if status == 404 {
                error_response("ERR_RECORDING_NOT_FOUND", "Video recording not found")
            } else {
                error_response("ERR_RECORDING_ACTIVE", "Cannot delete an active recording")
            }
        }
    }
}

/// POST /api/v1/videos/{id}/stop - Force-stop an in-progress recording
pub async fn stop_video(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();
    match state.video_service.stop_recording(&id).await {
        Ok(info) => success_response(json!(info)),
        Err(e) => error_response("ERR_RECORDING_NOT_FOUND", &format!("Stop failed: {}", e)),
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

    // Clone state for the spawned task (Story 12-6: WS counting)
    let state = state.into_inner().clone();
    state.metrics.increment_ws_count();

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
        let state_clone = state.clone(); // Story 12-6: for latency recording

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

                        // Record screenshot latency (Story 12-6)
                        state_clone.metrics.record_screenshot_latency(total.as_secs_f64());

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
                                let _ = session.text(serde_json::to_string(&error_resp).unwrap_or_default()).await;
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
                                        result: Some(json!("ok")),
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
                                        result: Some(json!("ok")),
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
                                            result: Some(json!("ok")),
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
                                            result: Some(json!("ok")),
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
                                            result: Some(json!("ok")),
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
                                        result: Some(json!("ok")),
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
                            let _ = session.text(serde_json::to_string(&response).unwrap_or_default()).await;
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
        state.metrics.decrement_ws_count(); // Story 12-6

        tracing::info!("[API-V1-WS] WebSocket session closed: {}", udid);
    });

    resp
}

/// Helper to get device client for WebSocket using shared DeviceResolver (Story 13-1)
async fn get_device_client_for_ws(
    state: &AppState,
    udid: &str,
) -> Result<(serde_json::Value, Arc<AtxClient>), HttpResponse> {
    DeviceResolver::new(state)
        .get_device_client(udid)
        .await
        .map_err(|e| e.into())
}

// ═════════════════════════════════════════════════════════════════════════════
// JSON-RPC NIO WebSocket Handler (Story 5-5)
// ═════════════════════════════════════════════════════════════════════════════

/// JSON-RPC NIO WebSocket endpoint — device-agnostic automation interface.
/// Unlike `/ws/screenshot/{udid}`, this endpoint accepts any device UDID via params.
pub async fn ws_nio(
    state: web::Data<AppState>,
    req: HttpRequest,
    stream: web::Payload,
) -> HttpResponse {
    let (resp, mut session, mut msg_stream) = match actix_ws::handle(&req, stream) {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_WEBSOCKET_UPGRADE_FAILED",
                "message": format!("WebSocket upgrade failed: {}", e)
            }));
        }
    };

    state.metrics.websocket_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    tracing::info!("[WS-NIO] Client connected");

    let state_inner = state.into_inner();
    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                actix_ws::Message::Text(text) => {
                    let response = handle_nio_rpc(&state_inner, &text).await;
                    if session.text(response).await.is_err() {
                        break;
                    }
                }
                actix_ws::Message::Ping(bytes) => {
                    if session.pong(&bytes).await.is_err() {
                        break;
                    }
                }
                actix_ws::Message::Close(_) => break,
                _ => {}
            }
        }
        state_inner.metrics.websocket_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        tracing::info!("[WS-NIO] Client disconnected");
    });

    resp
}

/// Parse and dispatch a JSON-RPC 2.0 request.
async fn handle_nio_rpc(state: &AppState, text: &str) -> String {
    let request: JsonRpcRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            return serde_json::to_string(&JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                }),
                id: 0,
            })
            .unwrap_or_default();
        }
    };

    if request.jsonrpc != "2.0" {
        return serde_json::to_string(&JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "Invalid Request: jsonrpc must be \"2.0\"".to_string(),
            }),
            id: request.id,
        })
        .unwrap_or_default();
    }

    let result = match request.method.as_str() {
        // Single-device operations
        "tap" => nio_tap(state, &request.params, request.id).await,
        "swipe" => nio_swipe(state, &request.params, request.id).await,
        "input" => nio_input(state, &request.params, request.id).await,
        "keyevent" => nio_keyevent(state, &request.params, request.id).await,
        // Batch operations
        "batchTap" => nio_batch_tap(state, &request.params, request.id).await,
        "batchSwipe" => nio_batch_swipe(state, &request.params, request.id).await,
        "batchInput" => nio_batch_input(state, &request.params, request.id).await,
        // Utility methods
        "listDevices" => nio_list_devices(state, request.id).await,
        "getDevice" => nio_get_device(state, &request.params, request.id).await,
        "screenshot" => nio_screenshot(state, &request.params, request.id).await,
        "getStatus" => nio_get_status(state, request.id).await,
        _ => {
            return serde_json::to_string(&JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                }),
                id: request.id,
            })
            .unwrap_or_default();
        }
    };

    serde_json::to_string(&result).unwrap_or_default()
}

// ─── NIO Helpers ─────────────────────────────────────────────────────────────

/// Resolve a device client from UDID for NIO operations using shared DeviceResolver (Story 13-1)
async fn nio_get_client(
    state: &AppState,
    udid: &str,
) -> Result<(serde_json::Value, Arc<AtxClient>), JsonRpcError> {
    DeviceResolver::new(state)
        .get_device_client(udid)
        .await
        .map_err(|e| JsonRpcError {
            code: -1,
            message: e.message().to_string(),
        })
}

fn missing_param(name: &str) -> JsonRpcError {
    JsonRpcError {
        code: -32602,
        message: format!("Invalid params: missing '{}'", name),
    }
}

fn invalid_param(name: &str, detail: &str) -> JsonRpcError {
    JsonRpcError {
        code: -32602,
        message: format!("Invalid params: '{}' {}", name, detail),
    }
}

fn operation_failed(msg: String) -> JsonRpcError {
    JsonRpcError {
        code: -32603,
        message: msg,
    }
}

fn rpc_ok(id: u64) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(json!("ok")),
        error: None,
        id,
    }
}

fn rpc_result(id: u64, value: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(value),
        error: None,
        id,
    }
}

fn rpc_error(id: u64, err: JsonRpcError) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(err),
        id,
    }
}

// ─── Single-Device JSON-RPC Methods ──────────────────────────────────────────

async fn nio_tap(state: &AppState, params: &serde_json::Value, id: u64) -> JsonRpcResponse {
    let udid = match params.get("udid").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return rpc_error(id, missing_param("udid")),
    };
    let x = match params.get("x").and_then(|v| v.as_f64()) {
        Some(v) => v as i32,
        None => return rpc_error(id, missing_param("x")),
    };
    let y = match params.get("y").and_then(|v| v.as_f64()) {
        Some(v) => v as i32,
        None => return rpc_error(id, missing_param("y")),
    };

    let (_device, client) = match nio_get_client(state, udid).await {
        Ok(c) => c,
        Err(e) => return rpc_error(id, e),
    };

    // click() takes i32 directly; batch uses u32→i32 cast via execute_single_tap
    match client.click(x, y).await {
        Ok(_) => rpc_ok(id),
        Err(e) => rpc_error(id, operation_failed(format!("Tap failed: {}", e))),
    }
}

async fn nio_swipe(state: &AppState, params: &serde_json::Value, id: u64) -> JsonRpcResponse {
    let udid = match params.get("udid").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return rpc_error(id, missing_param("udid")),
    };
    let x1 = match params.get("x1").and_then(|v| v.as_f64()) {
        Some(v) => v as i32,
        None => return rpc_error(id, missing_param("x1")),
    };
    let y1 = match params.get("y1").and_then(|v| v.as_f64()) {
        Some(v) => v as i32,
        None => return rpc_error(id, missing_param("y1")),
    };
    let x2 = match params.get("x2").and_then(|v| v.as_f64()) {
        Some(v) => v as i32,
        None => return rpc_error(id, missing_param("x2")),
    };
    let y2 = match params.get("y2").and_then(|v| v.as_f64()) {
        Some(v) => v as i32,
        None => return rpc_error(id, missing_param("y2")),
    };
    let duration = params
        .get("duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(200.0);

    let (_device, client) = match nio_get_client(state, udid).await {
        Ok(c) => c,
        Err(e) => return rpc_error(id, e),
    };

    match client.swipe(x1, y1, x2, y2, duration).await {
        Ok(_) => rpc_ok(id),
        Err(e) => rpc_error(id, operation_failed(format!("Swipe failed: {}", e))),
    }
}

async fn nio_input(state: &AppState, params: &serde_json::Value, id: u64) -> JsonRpcResponse {
    let udid = match params.get("udid").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return rpc_error(id, missing_param("udid")),
    };
    let text = match params.get("text").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return rpc_error(id, missing_param("text")),
    };

    let (_device, client) = match nio_get_client(state, udid).await {
        Ok(c) => c,
        Err(e) => return rpc_error(id, e),
    };

    match client.input_text(text).await {
        Ok(_) => rpc_ok(id),
        Err(e) => rpc_error(id, operation_failed(format!("Input failed: {}", e))),
    }
}

async fn nio_keyevent(state: &AppState, params: &serde_json::Value, id: u64) -> JsonRpcResponse {
    let udid = match params.get("udid").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return rpc_error(id, missing_param("udid")),
    };
    let key = match params.get("key").and_then(|v| v.as_str()) {
        Some(k) => k,
        None => return rpc_error(id, missing_param("key")),
    };

    let (_device, client) = match nio_get_client(state, udid).await {
        Ok(c) => c,
        Err(e) => return rpc_error(id, e),
    };

    match client.press_key(key).await {
        Ok(_) => rpc_ok(id),
        Err(e) => rpc_error(id, operation_failed(format!("Keyevent failed: {}", e))),
    }
}

// ─── Batch JSON-RPC Methods ─────────────────────────────────────────────────

fn extract_udids(params: &serde_json::Value) -> Result<Vec<String>, JsonRpcError> {
    let udids = params
        .get("udids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| missing_param("udids"))?;

    if udids.is_empty() {
        return Err(invalid_param("udids", "must not be empty"));
    }
    if udids.len() > MAX_BATCH_SIZE {
        return Err(invalid_param(
            "udids",
            &format!("exceeds max batch size of {}", MAX_BATCH_SIZE),
        ));
    }

    udids
        .iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| invalid_param("udids", "all elements must be strings"))
        })
        .collect()
}

fn build_batch_result(results: Vec<(String, Result<(), (String, String)>)>) -> serde_json::Value {
    let total = results.len();
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut items = Vec::new();

    for (udid, result) in results {
        match result {
            Ok(_) => {
                succeeded += 1;
                items.push(json!({"udid": udid, "status": "success"}));
            }
            Err((_code, msg)) => {
                failed += 1;
                items.push(json!({"udid": udid, "status": "error", "error": msg}));
            }
        }
    }

    json!({
        "total": total,
        "succeeded": succeeded,
        "failed": failed,
        "results": items
    })
}

async fn nio_batch_tap(state: &AppState, params: &serde_json::Value, id: u64) -> JsonRpcResponse {
    let udids = match extract_udids(params) {
        Ok(u) => u,
        Err(e) => return rpc_error(id, e),
    };
    // Parse as i32 (consistent with nio_tap), cast to u32 for execute_single_tap
    let x = match params.get("x").and_then(|v| v.as_f64()) {
        Some(v) => v as i32 as u32,
        None => return rpc_error(id, missing_param("x")),
    };
    let y = match params.get("y").and_then(|v| v.as_f64()) {
        Some(v) => v as i32 as u32,
        None => return rpc_error(id, missing_param("y")),
    };

    let futures: Vec<_> = udids.iter()
        .map(|udid| execute_single_tap(state, udid, x, y))
        .collect();
    let results = futures::future::join_all(futures).await;

    let batch = udids.into_iter()
        .zip(results.into_iter())
        .collect::<Vec<_>>();

    rpc_result(id, build_batch_result(batch))
}

async fn nio_batch_swipe(
    state: &AppState,
    params: &serde_json::Value,
    id: u64,
) -> JsonRpcResponse {
    let udids = match extract_udids(params) {
        Ok(u) => u,
        Err(e) => return rpc_error(id, e),
    };
    // Parse as i32 (consistent with nio_swipe), cast to u32 for execute_single_swipe
    let x1 = match params.get("x1").and_then(|v| v.as_f64()) {
        Some(v) => v as i32 as u32,
        None => return rpc_error(id, missing_param("x1")),
    };
    let y1 = match params.get("y1").and_then(|v| v.as_f64()) {
        Some(v) => v as i32 as u32,
        None => return rpc_error(id, missing_param("y1")),
    };
    let x2 = match params.get("x2").and_then(|v| v.as_f64()) {
        Some(v) => v as i32 as u32,
        None => return rpc_error(id, missing_param("x2")),
    };
    let y2 = match params.get("y2").and_then(|v| v.as_f64()) {
        Some(v) => v as i32 as u32,
        None => return rpc_error(id, missing_param("y2")),
    };
    let duration = params
        .get("duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(200.0);

    let futures: Vec<_> = udids.iter()
        .map(|udid| execute_single_swipe(state, udid, x1, y1, x2, y2, duration))
        .collect();
    let results = futures::future::join_all(futures).await;

    let batch = udids.into_iter()
        .zip(results.into_iter())
        .collect::<Vec<_>>();

    rpc_result(id, build_batch_result(batch))
}

async fn nio_batch_input(
    state: &AppState,
    params: &serde_json::Value,
    id: u64,
) -> JsonRpcResponse {
    let udids = match extract_udids(params) {
        Ok(u) => u,
        Err(e) => return rpc_error(id, e),
    };
    let text = match params.get("text").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => return rpc_error(id, missing_param("text")),
    };

    let futures: Vec<_> = udids.iter()
        .map(|udid| execute_single_input(state, udid, &text))
        .collect();
    let results = futures::future::join_all(futures).await;

    let batch = udids.into_iter()
        .zip(results.into_iter())
        .collect::<Vec<_>>();

    rpc_result(id, build_batch_result(batch))
}

// ─── Utility JSON-RPC Methods ───────────────────────────────────────────────

async fn nio_list_devices(state: &AppState, id: u64) -> JsonRpcResponse {
    let phone_service = state.phone_service.clone();
    match phone_service.query_device_list_by_present().await {
        Ok(devices) => rpc_result(id, json!(devices)),
        Err(e) => rpc_error(id, operation_failed(format!("Failed to list devices: {}", e))),
    }
}

async fn nio_get_device(
    state: &AppState,
    params: &serde_json::Value,
    id: u64,
) -> JsonRpcResponse {
    let udid = match params.get("udid").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return rpc_error(id, missing_param("udid")),
    };

    let phone_service = state.phone_service.clone();
    match phone_service.query_info_by_udid(udid).await {
        Ok(Some(device)) => rpc_result(id, device),
        Ok(None) => rpc_error(
            id,
            JsonRpcError {
                code: -1,
                message: format!("Device '{}' not found", udid),
            },
        ),
        Err(e) => rpc_error(id, operation_failed(format!("Failed to get device: {}", e))),
    }
}

async fn nio_screenshot(
    state: &AppState,
    params: &serde_json::Value,
    id: u64,
) -> JsonRpcResponse {
    let udid = match params.get("udid").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return rpc_error(id, missing_param("udid")),
    };
    let quality = params
        .get("quality")
        .and_then(|v| v.as_u64())
        .map(|q| q as u8)
        .unwrap_or(80);
    let scale = params
        .get("scale")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    let (_device, client) = match nio_get_client(state, udid).await {
        Ok(c) => c,
        Err(e) => return rpc_error(id, e),
    };

    match client.screenshot_scaled(scale, quality).await {
        Ok(data) => {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
            rpc_result(
                id,
                json!({
                    "format": "jpeg",
                    "quality": quality,
                    "scale": scale,
                    "data": b64
                }),
            )
        }
        Err(e) => rpc_error(
            id,
            operation_failed(format!("Screenshot failed: {}", e)),
        ),
    }
}

async fn nio_get_status(state: &AppState, id: u64) -> JsonRpcResponse {
    let phone_service = state.phone_service.clone();
    // query_device_list_by_present returns only devices with present=true (connected)
    match phone_service.query_device_list_by_present().await {
        Ok(devices) => {
            rpc_result(
                id,
                json!({
                    "total_devices": devices.len(),
                    "connected": devices.len(),
                    "websocket_connections": state.metrics.websocket_count.load(std::sync::atomic::Ordering::Relaxed)
                }),
            )
        }
        Err(e) => rpc_error(id, operation_failed(format!("Failed to get status: {}", e))),
    }
}
