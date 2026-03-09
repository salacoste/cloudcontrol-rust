use crate::services::scrcpy_manager::ScrcpyManager;
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use serde_json::json;

// ─── Control request types ───

#[derive(Deserialize)]
pub struct TapRequest {
    pub x: u32,
    pub y: u32,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

#[derive(Deserialize)]
pub struct KeyRequest {
    pub keycode: u32,
    /// "press" (default, sends down+up), "down", or "up"
    pub action: Option<String>,
}

#[derive(Deserialize)]
pub struct SwipeRequest {
    pub start_x: u32,
    pub start_y: u32,
    pub end_x: u32,
    pub end_y: u32,
    /// Duration in milliseconds (default: 300)
    pub duration_ms: Option<u64>,
    /// Number of intermediate move steps (default: 20)
    pub steps: Option<u32>,
}

/// POST /scrcpy/{udid}/start — Start a scrcpy session for the given device.
pub async fn start_scrcpy_session(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    // Check if scrcpy JAR is available
    if !ScrcpyManager::jar_available() {
        return HttpResponse::ServiceUnavailable().json(json!({
            "status": "error",
            "error": "ERR_SCRCPY_NOT_AVAILABLE",
            "message": "scrcpy-server.jar not found. Place it at resources/scrcpy/scrcpy-server.jar"
        }));
    }

    // Look up device serial
    let serial = match resolve_device_serial(&state, &udid).await {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    // Start session
    match state.scrcpy_manager.start_session(&udid, &serial).await {
        Ok(info) => HttpResponse::Ok().json(json!({
            "status": "success",
            "session_id": info.session_id,
            "udid": info.udid,
            "serial": info.serial,
            "width": info.width,
            "height": info.height,
            "device_name": info.device_name,
            "started_at": info.started_at,
        })),
        Err(e) if e == "ERR_SESSION_ALREADY_ACTIVE" => {
            HttpResponse::Conflict().json(json!({
                "status": "error",
                "error": "ERR_SESSION_ALREADY_ACTIVE",
                "message": format!("Device '{}' already has an active scrcpy session", udid)
            }))
        }
        Err(e) => {
            tracing::error!("[Scrcpy] Failed to start session for {}: {}", udid, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("Failed to start scrcpy session: {}", e)
            }))
        }
    }
}

/// POST /scrcpy/{udid}/stop — Stop the scrcpy session for the given device.
pub async fn stop_scrcpy_session(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    match state.scrcpy_manager.stop_session(&udid).await {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Scrcpy session stopped"
        })),
        Err(e) if e == "ERR_SESSION_NOT_FOUND" => {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_SESSION_NOT_FOUND",
                "message": format!("No active scrcpy session for device '{}'", udid)
            }))
        }
        Err(e) => {
            tracing::error!("[Scrcpy] Failed to stop session for {}: {}", udid, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("Failed to stop scrcpy session: {}", e)
            }))
        }
    }
}

/// GET /scrcpy/sessions — List all active scrcpy sessions.
pub async fn list_scrcpy_sessions(state: web::Data<AppState>) -> HttpResponse {
    let sessions = state.scrcpy_manager.list_sessions();
    HttpResponse::Ok().json(json!({
        "status": "success",
        "sessions": sessions,
        "count": sessions.len(),
    }))
}

/// POST /scrcpy/{udid}/tap — Send a tap event through the scrcpy control socket.
pub async fn scrcpy_tap(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<TapRequest>,
) -> HttpResponse {
    let udid = path.into_inner();
    let req = body.into_inner();

    // Single atomic lookup: get info + handle together to avoid TOCTOU race
    let result = async {
        let (info, handle) = state.scrcpy_manager.get_session_with_info(&udid)?;
        let width = req
            .width
            .unwrap_or(u16::try_from(info.width).unwrap_or(u16::MAX));
        let height = req
            .height
            .unwrap_or(u16::try_from(info.height).unwrap_or(u16::MAX));

        let mut session = handle.lock().await;
        session
            .send_touch(0, req.x, req.y, width, height, 0xFFFF)
            .await?;
        session
            .send_touch(1, req.x, req.y, width, height, 0)
            .await?;
        Ok::<(), String>(())
    }
    .await;

    match result {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "action": "tap",
            "x": req.x,
            "y": req.y,
        })),
        Err(e) if e == "ERR_SESSION_NOT_FOUND" => {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_SESSION_NOT_FOUND",
                "message": format!("No active scrcpy session for device '{}'", udid)
            }))
        }
        Err(e) => {
            tracing::error!("[Scrcpy] Tap failed for {}: {}", udid, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("Tap failed: {}", e)
            }))
        }
    }
}

/// POST /scrcpy/{udid}/key — Send a key event through the scrcpy control socket.
pub async fn scrcpy_key(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<KeyRequest>,
) -> HttpResponse {
    let udid = path.into_inner();
    let req = body.into_inner();

    let key_action = req.action.as_deref().unwrap_or("press");

    // Validate action before acquiring session lock
    if !matches!(key_action, "press" | "down" | "up") {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_ACTION",
            "message": format!("Invalid key action '{}'. Must be 'press', 'down', or 'up'", key_action)
        }));
    }

    let result = async {
        let handle = state.scrcpy_manager.get_session_handle(&udid)?;
        let mut session = handle.lock().await;
        match key_action {
            "down" => session.send_key(0, req.keycode).await?,
            "up" => session.send_key(1, req.keycode).await?,
            _ => {
                // "press" = down + up
                session.send_key(0, req.keycode).await?;
                session.send_key(1, req.keycode).await?;
            }
        }
        Ok::<(), String>(())
    }
    .await;

    match result {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "action": "key",
            "keycode": req.keycode,
            "key_action": key_action,
        })),
        Err(e) if e == "ERR_SESSION_NOT_FOUND" => {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_SESSION_NOT_FOUND",
                "message": format!("No active scrcpy session for device '{}'", udid)
            }))
        }
        Err(e) => {
            tracing::error!("[Scrcpy] Key event failed for {}: {}", udid, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("Key event failed: {}", e)
            }))
        }
    }
}

/// Maximum swipe duration to prevent lock starvation (5 seconds).
const MAX_SWIPE_DURATION_MS: u64 = 5000;

/// POST /scrcpy/{udid}/swipe — Send a swipe gesture through the scrcpy control socket.
pub async fn scrcpy_swipe(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<SwipeRequest>,
) -> HttpResponse {
    let udid = path.into_inner();
    let req = body.into_inner();

    let duration_ms = req.duration_ms.unwrap_or(300).min(MAX_SWIPE_DURATION_MS);
    let steps = req.steps.unwrap_or(20).max(1);
    let step_delay = std::time::Duration::from_millis(duration_ms / steps as u64);

    // Single atomic lookup: get info + handle together to avoid TOCTOU race
    let result = async {
        let (info, handle) = state.scrcpy_manager.get_session_with_info(&udid)?;
        let width = u16::try_from(info.width).unwrap_or(u16::MAX);
        let height = u16::try_from(info.height).unwrap_or(u16::MAX);

        // Lock is held for the entire swipe duration to prevent interleaved
        // touch events from other callers corrupting the gesture sequence.
        let mut session = handle.lock().await;

        // Touch down at start position
        session
            .send_touch(0, req.start_x, req.start_y, width, height, 0xFFFF)
            .await?;

        // Interpolated move steps
        for i in 1..=steps {
            let x = req.start_x as i64
                + (req.end_x as i64 - req.start_x as i64) * i as i64 / steps as i64;
            let y = req.start_y as i64
                + (req.end_y as i64 - req.start_y as i64) * i as i64 / steps as i64;
            tokio::time::sleep(step_delay).await;
            session
                .send_touch(2, x as u32, y as u32, width, height, 0xFFFF)
                .await?;
        }

        // Touch up at end position
        session
            .send_touch(1, req.end_x, req.end_y, width, height, 0)
            .await?;

        Ok::<(), String>(())
    }
    .await;

    match result {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "action": "swipe",
            "start": {"x": req.start_x, "y": req.start_y},
            "end": {"x": req.end_x, "y": req.end_y},
            "duration_ms": duration_ms,
            "steps": steps,
        })),
        Err(e) if e == "ERR_SESSION_NOT_FOUND" => {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_SESSION_NOT_FOUND",
                "message": format!("No active scrcpy session for device '{}'", udid)
            }))
        }
        Err(e) => {
            tracing::error!("[Scrcpy] Swipe failed for {}: {}", udid, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("Swipe failed: {}", e)
            }))
        }
    }
}

// ─── Scrcpy Recording Endpoints ───

/// POST /scrcpy/{udid}/recording/start — Start recording the scrcpy session.
pub async fn start_scrcpy_recording(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    match state.scrcpy_manager.start_recording(&udid) {
        Ok(info) => HttpResponse::Ok().json(json!({
            "status": "success",
            "recording": info,
        })),
        Err(e) if e == "ERR_SESSION_NOT_FOUND" => {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_SESSION_NOT_FOUND",
                "message": format!("No active scrcpy session for device '{}'", udid)
            }))
        }
        Err(e) if e == "ERR_RECORDING_ALREADY_ACTIVE" => {
            HttpResponse::Conflict().json(json!({
                "status": "error",
                "error": "ERR_RECORDING_ALREADY_ACTIVE",
                "message": format!("Device '{}' already has an active recording", udid)
            }))
        }
        Err(e) => {
            tracing::error!("[Scrcpy] Failed to start recording for {}: {}", udid, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("Failed to start recording: {}", e)
            }))
        }
    }
}

/// POST /scrcpy/{udid}/recording/stop — Stop recording the scrcpy session.
pub async fn stop_scrcpy_recording(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    match state.scrcpy_manager.stop_recording(&udid).await {
        Ok(info) => HttpResponse::Ok().json(json!({
            "status": "success",
            "recording": info,
        })),
        Err(e) if e == "ERR_SESSION_NOT_FOUND" => {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_SESSION_NOT_FOUND",
                "message": format!("No active scrcpy session for device '{}'", udid)
            }))
        }
        Err(e) if e == "ERR_NO_ACTIVE_RECORDING" => {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_NO_ACTIVE_RECORDING",
                "message": format!("No active recording for device '{}'", udid)
            }))
        }
        Err(e) => {
            tracing::error!("[Scrcpy] Failed to stop recording for {}: {}", udid, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("Failed to stop recording: {}", e)
            }))
        }
    }
}

/// GET /scrcpy/recordings — List all scrcpy recordings.
pub async fn list_scrcpy_recordings(state: web::Data<AppState>) -> HttpResponse {
    let recordings = state.scrcpy_manager.list_recordings();
    HttpResponse::Ok().json(json!({
        "status": "success",
        "recordings": recordings,
        "count": recordings.len(),
    }))
}

/// GET /scrcpy/recordings/{id} — Get a specific recording's metadata.
pub async fn get_scrcpy_recording(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    match state.scrcpy_manager.get_recording(&id) {
        Some(info) => HttpResponse::Ok().json(json!({
            "status": "success",
            "recording": info,
        })),
        None => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_RECORDING_NOT_FOUND",
            "message": format!("Recording '{}' not found", id)
        })),
    }
}

/// GET /scrcpy/recordings/{id}/download — Download a recording file.
pub async fn download_scrcpy_recording(
    state: web::Data<AppState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let id = path.into_inner();

    let file_path = match state.scrcpy_manager.get_recording_file_path(&id) {
        Some(p) => p,
        None => {
            return HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_RECORDING_NOT_FOUND",
                "message": format!("Recording '{}' not found", id)
            }));
        }
    };

    let named_file = match actix_files::NamedFile::open(&file_path) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(
                "[Scrcpy] Failed to open recording file {:?}: {}",
                file_path,
                e
            );
            return HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_RECORDING_NOT_FOUND",
                "message": "Recording file not found on disk"
            }));
        }
    };

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recording.h264")
        .to_string();

    let file_with_disposition = named_file.set_content_disposition(
        actix_web::http::header::ContentDisposition {
            disposition: actix_web::http::header::DispositionType::Attachment,
            parameters: vec![actix_web::http::header::DispositionParam::Filename(
                filename,
            )],
        },
    );

    file_with_disposition.into_response(&req)
}

/// DELETE /scrcpy/recordings/{id} — Delete a recording.
pub async fn delete_scrcpy_recording(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();

    match state.scrcpy_manager.delete_recording(&id) {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Recording deleted"
        })),
        Err(e) if e == "ERR_RECORDING_NOT_FOUND" => {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_RECORDING_NOT_FOUND",
                "message": format!("Recording '{}' not found", id)
            }))
        }
        Err(e) if e == "ERR_RECORDING_ACTIVE" => {
            HttpResponse::Conflict().json(json!({
                "status": "error",
                "error": "ERR_RECORDING_ACTIVE",
                "message": format!("Recording '{}' is still active. Stop it before deleting.", id)
            }))
        }
        Err(e) => {
            tracing::error!("[Scrcpy] Failed to delete recording {}: {}", id, e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("Failed to delete recording: {}", e)
            }))
        }
    }
}

/// Resolve a device UDID to its ADB serial, using cache then DB fallback.
async fn resolve_device_serial(
    state: &AppState,
    udid: &str,
) -> Result<String, HttpResponse> {
    // Try device info cache first
    if let Some(cached) = state.device_info_cache.get(udid).await {
        let serial = cached
            .get("serial")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !serial.is_empty() {
            return Ok(serial);
        }
    }

    // Fallback to DB
    let phone_service =
        crate::services::phone_service::PhoneService::new(state.db.clone());
    let device = phone_service
        .query_info_by_udid(udid)
        .await
        .map_err(|e| {
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": format!("Database error: {}", e)
            }))
        })?
        .ok_or_else(|| {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_DEVICE_NOT_FOUND",
                "message": format!("Device '{}' not found", udid)
            }))
        })?;

    // Cache the device info for future lookups
    state.device_info_cache.insert(udid.to_string(), device.clone()).await;

    let serial = device
        .get("serial")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if serial.is_empty() {
        return Err(HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_DEVICE_NO_SERIAL",
            "message": format!("Device '{}' has no serial number (USB connection required for scrcpy)", udid)
        })));
    }

    Ok(serial)
}
