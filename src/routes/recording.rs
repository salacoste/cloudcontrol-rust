use crate::device::adb::Adb;
use crate::models::recording::{
    ActionType, RecordActionRequest, RecordedAction, StartRecordingRequest,
    StopRecordingRequest, EditActionRequest,
    StartPlaybackRequest,
};
use crate::services::recording_service::PlaybackStatus;
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use serde_json::json;

/// POST /api/recordings/start - Start a new recording session
pub async fn start_recording(
    state: web::Data<AppState>,
    body: web::Json<StartRecordingRequest>,
) -> HttpResponse {
    let name = body.name.as_deref().unwrap_or("Untitled Recording");

    match state.recording_service.start_recording(&body.device_udid, name).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            let error_code = if e.contains("already has an active") {
                "ERR_RECORDING_ALREADY_ACTIVE"
            } else {
                "ERR_RECORDING_START_FAILED"
            };
            HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": error_code,
                "message": e
            }))
        }
    }
}

/// POST /api/recordings/{id}/action - Record an action in the active session
pub async fn record_action(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: web::Json<RecordActionRequest>,
) -> HttpResponse {
    let recording_id = path.into_inner();

    // Get the recording to find device_udid
    match state.recording_service.get_recording(recording_id).await {
        Ok(recording) => {
            match state.recording_service.record_action(&recording.device_udid, body.into_inner()).await {
                Ok(response) => HttpResponse::Ok().json(response),
                Err(e) => {
                    let error_code = if e == "ERR_NO_ACTIVE_RECORDING" {
                        "ERR_NO_ACTIVE_RECORDING"
                    } else {
                        "ERR_RECORD_ACTION_FAILED"
                    };
                    HttpResponse::BadRequest().json(json!({
                        "status": "error",
                        "error": error_code,
                        "message": e
                    }))
                }
            }
        }
        Err(e) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_RECORDING_NOT_FOUND",
            "message": e
        }))
    }
}

/// POST /api/recordings/{id}/stop - Stop and save a recording
pub async fn stop_recording(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: web::Json<StopRecordingRequest>,
) -> HttpResponse {
    let recording_id = path.into_inner();

    // Get the recording to find device_udid
    match state.recording_service.get_recording(recording_id).await {
        Ok(recording) => {
            match state.recording_service.stop_recording(&recording.device_udid, body.into_inner()).await {
                Ok(response) => HttpResponse::Ok().json(response),
                Err(e) => {
                    let status = if e == "ERR_NO_ACTIVE_RECORDING" {
                        actix_web::http::StatusCode::BAD_REQUEST
                    } else {
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
                    };
                    HttpResponse::build(status).json(json!({
                        "status": "error",
                        "error": "ERR_RECORDING_STOP_FAILED",
                        "message": e
                    }))
                }
            }
        }
        Err(e) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_RECORDING_NOT_FOUND",
            "message": e
        }))
    }
}

/// GET /api/recordings - List all recordings
pub async fn list_recordings(state: web::Data<AppState>) -> HttpResponse {
    match state.recording_service.list_recordings().await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_LIST_RECORDINGS_FAILED",
            "message": e
        }))
    }
}

/// GET /api/recordings/{id} - Get a specific recording with actions
pub async fn get_recording(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let recording_id = path.into_inner();

    match state.recording_service.get_recording(recording_id).await {
        Ok(recording) => HttpResponse::Ok().json(json!({
            "status": "success",
            "recording": recording
        })),
        Err(e) => {
            if e.contains("not found") {
                HttpResponse::NotFound().json(json!({
                    "status": "error",
                    "error": "ERR_RECORDING_NOT_FOUND",
                    "message": e
                }))
            } else {
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "error": "ERR_GET_RECORDING_FAILED",
                    "message": e
                }))
            }
        }
    }
}

/// DELETE /api/recordings/{id} - Delete a recording
pub async fn delete_recording(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let recording_id = path.into_inner();

    match state.recording_service.delete_recording(recording_id).await {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": format!("Recording {} deleted", recording_id)
        })),
        Err(e) => {
            if e.contains("not found") {
                HttpResponse::NotFound().json(json!({
                    "status": "error",
                    "error": "ERR_RECORDING_NOT_FOUND",
                    "message": e
                }))
            } else {
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "error": "ERR_DELETE_RECORDING_FAILED",
                    "message": e
                }))
            }
        }
    }
}

/// POST /api/recordings/{id}/pause - Pause an active recording
pub async fn pause_recording(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let recording_id = path.into_inner();

    // Get the recording to find device_udid
    match state.recording_service.get_recording(recording_id).await {
        Ok(recording) => {
            match state.recording_service.pause_recording(&recording.device_udid).await {
                Ok(()) => HttpResponse::Ok().json(json!({
                    "status": "success",
                    "message": "Recording paused"
                })),
                Err(e) => {
                    let error_code = if e == "ERR_NO_ACTIVE_RECORDING" {
                        "ERR_NO_ACTIVE_RECORDING"
                    } else if e == "ERR_RECORDING_ALREADY_PAUSED" {
                        "ERR_RECORDING_ALREADY_PAUSED"
                    } else {
                        "ERR_PAUSE_FAILED"
                    };
                    HttpResponse::BadRequest().json(json!({
                        "status": "error",
                        "error": error_code,
                        "message": e
                    }))
                }
            }
        }
        Err(e) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_RECORDING_NOT_FOUND",
            "message": e
        }))
    }
}

/// POST /api/recordings/{id}/resume - Resume a paused recording
pub async fn resume_recording(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let recording_id = path.into_inner();

    // Get the recording to find device_udid
    match state.recording_service.get_recording(recording_id).await {
        Ok(recording) => {
            match state.recording_service.resume_recording(&recording.device_udid).await {
                Ok(()) => HttpResponse::Ok().json(json!({
                    "status": "success",
                    "message": "Recording resumed"
                })),
                Err(e) => {
                    let error_code = if e == "ERR_NO_ACTIVE_RECORDING" {
                        "ERR_NO_ACTIVE_RECORDING"
                    } else if e == "ERR_RECORDING_NOT_PAUSED" {
                        "ERR_RECORDING_NOT_PAUSED"
                    } else {
                        "ERR_RESUME_FAILED"
                    };
                    HttpResponse::BadRequest().json(json!({
                        "status": "error",
                        "error": error_code,
                        "message": e
                    }))
                }
            }
        }
        Err(e) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_RECORDING_NOT_FOUND",
            "message": e
        }))
    }
}

/// POST /api/recordings/{id}/cancel - Cancel recording without saving
pub async fn cancel_recording(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let recording_id = path.into_inner();

    // Get the recording to find device_udid
    match state.recording_service.get_recording(recording_id).await {
        Ok(recording) => {
            match state.recording_service.cancel_recording(&recording.device_udid).await {
                Ok(()) => HttpResponse::Ok().json(json!({
                    "status": "success",
                    "message": "Recording cancelled and discarded"
                })),
                Err(e) => {
                    let error_code = if e == "ERR_NO_ACTIVE_RECORDING" {
                        "ERR_NO_ACTIVE_RECORDING"
                    } else {
                        "ERR_CANCEL_FAILED"
                    };
                    HttpResponse::BadRequest().json(json!({
                        "status": "error",
                        "error": error_code,
                        "message": e
                    }))
                }
            }
        }
        Err(e) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_RECORDING_NOT_FOUND",
            "message": e
        }))
    }
}

/// GET /api/recordings/{id}/status - Get recording status
pub async fn get_recording_status(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let recording_id = path.into_inner();

    // Get the recording to find device_udid
    match state.recording_service.get_recording(recording_id).await {
        Ok(recording) => {
            match state.recording_service.get_recording_status(&recording.device_udid).await {
                Ok(status) => HttpResponse::Ok().json(status),
                Err(e) => {
                    let error_code = if e == "ERR_NO_ACTIVE_RECORDING" {
                        "ERR_NO_ACTIVE_RECORDING"
                    } else {
                        "ERR_STATUS_FAILED"
                    };
                    HttpResponse::BadRequest().json(json!({
                        "status": "error",
                        "error": error_code,
                        "message": e
                    }))
                }
            }
        }
        Err(e) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_RECORDING_NOT_FOUND",
            "message": e
        }))
    }
}

/// PUT /api/recordings/{id}/actions/{action_id} - Edit an action
pub async fn edit_action(
    state: web::Data<AppState>,
    path: web::Path<(i64, i64)>,
    body: web::Json<EditActionRequest>,
) -> HttpResponse {
    let (recording_id, action_id) = path.into_inner();

    match state.recording_service.update_action(recording_id, action_id, body.into_inner()).await {
        Ok(action) => HttpResponse::Ok().json(json!({
            "status": "success",
            "action": action
        })),
        Err(e) => {
            let error_code = if e == "ERR_ACTION_NOT_FOUND" {
                "ERR_ACTION_NOT_FOUND"
            } else {
                "ERR_EDIT_ACTION_FAILED"
            };
            let status = if e == "ERR_ACTION_NOT_FOUND" {
                actix_web::http::StatusCode::NOT_FOUND
            } else {
                actix_web::http::StatusCode::BAD_REQUEST
            };
            HttpResponse::build(status).json(json!({
                "status": "error",
                "error": error_code,
                "message": e
            }))
        }
    }
}

/// DELETE /api/recordings/{id}/actions/{action_id} - Delete an action
pub async fn delete_action(
    state: web::Data<AppState>,
    path: web::Path<(i64, i64)>,
) -> HttpResponse {
    let (recording_id, action_id) = path.into_inner();

    match state.recording_service.delete_action(recording_id, action_id).await {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Action deleted and remaining actions renumbered"
        })),
        Err(e) => {
            let error_code = if e == "ERR_ACTION_NOT_FOUND" {
                "ERR_ACTION_NOT_FOUND"
            } else {
                "ERR_DELETE_ACTION_FAILED"
            };
            let status = if e == "ERR_ACTION_NOT_FOUND" {
                actix_web::http::StatusCode::NOT_FOUND
            } else {
                actix_web::http::StatusCode::BAD_REQUEST
            };
            HttpResponse::build(status).json(json!({
                "status": "error",
                "error": error_code,
                "message": e
            }))
        }
    }
}

// ==================== Playback Endpoints ====================

/// Execute a single recorded action on a target device via ATX client with ADB fallback.
async fn execute_action_on_device(
    state: &AppState,
    target_device_udid: &str,
    action: &RecordedAction,
) -> Result<(), String> {
    // Get device client from connection pool
    let device = state.device_info_cache.get(target_device_udid).await;
    let (ip, port, serial) = if let Some(ref dev) = device {
        let ip = dev.get("ip").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let port = dev.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
        let serial = dev.get("serial").and_then(|v| v.as_str()).unwrap_or("").to_string();
        (ip, port, serial)
    } else {
        // Try to look up the device from DB
        let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
        let dev = phone_service.query_info_by_udid(target_device_udid).await
            .map_err(|e| format!("Device lookup failed: {}", e))?
            .ok_or_else(|| format!("Device {} not found", target_device_udid))?;
        let ip = dev.get("ip").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let port = dev.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
        let serial = dev.get("serial").and_then(|v| v.as_str()).unwrap_or("").to_string();
        state.device_info_cache.insert(target_device_udid.to_string(), dev).await;
        (ip, port, serial)
    };

    let client = state.connection_pool.get_or_create(target_device_udid, &ip, port).await;

    match action.action_type {
        ActionType::Tap => {
            let x = action.x.unwrap_or(0);
            let y = action.y.unwrap_or(0);
            if let Err(e) = client.click(x, y).await {
                tracing::warn!("[PLAYBACK] ATX tap failed, trying ADB: {}", e);
                if !serial.is_empty() {
                    Adb::input_tap(&serial, x, y).await?;
                } else {
                    return Err(e);
                }
            }
        }
        ActionType::Swipe => {
            let x = action.x.unwrap_or(0);
            let y = action.y.unwrap_or(0);
            let x2 = action.x2.unwrap_or(x);
            let y2 = action.y2.unwrap_or(y);
            let duration_ms = action.duration_ms.unwrap_or(200);
            let duration_secs = (duration_ms as f64 / 1000.0).max(0.05).min(2.0);
            if let Err(e) = client.swipe(x, y, x2, y2, duration_secs).await {
                tracing::warn!("[PLAYBACK] ATX swipe failed, trying ADB: {}", e);
                if !serial.is_empty() {
                    Adb::input_swipe(&serial, x, y, x2, y2, duration_ms.max(50).min(2000)).await?;
                } else {
                    return Err(e);
                }
            }
        }
        ActionType::Input => {
            let text = action.text.as_deref().unwrap_or("");
            if text.is_empty() {
                return Ok(());
            }
            if let Err(e) = client.input_text(text).await {
                tracing::warn!("[PLAYBACK] ATX input failed, trying ADB: {}", e);
                if !serial.is_empty() {
                    Adb::input_text(&serial, text).await?;
                } else {
                    return Err(e);
                }
            }
        }
        ActionType::KeyEvent => {
            let key_code = action.key_code.unwrap_or(0);
            if let Err(e) = client.shell_cmd(&format!("input keyevent {}", key_code)).await {
                tracing::warn!("[PLAYBACK] ATX keyevent failed, trying ADB: {}", e);
                if !serial.is_empty() {
                    let key_str = key_code.to_string();
                    Adb::input_keyevent(&serial, &key_str).await?;
                } else {
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

/// POST /api/recordings/{id}/play - Start playback on a target device
pub async fn start_playback(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    body: web::Json<StartPlaybackRequest>,
) -> HttpResponse {
    let recording_id = path.into_inner();
    let request = body.into_inner();
    let target_device_udid = request.target_device_udid.clone();
    let speed = if request.speed <= 0.0 { 1.0 } else { request.speed };

    match state.recording_service.start_playback(recording_id, request).await {
        Ok(response) => {
            // Spawn background task to execute playback actions
            let recording_service = state.recording_service.clone();
            let app_state = state.get_ref().clone();
            let udid = target_device_udid.clone();

            // Get actions for playback
            let actions = match recording_service.get_recording(recording_id).await {
                Ok(recording) => recording.actions,
                Err(e) => {
                    tracing::error!("[PLAYBACK] Failed to get recording actions: {}", e);
                    let _ = recording_service.stop_playback(&udid).await;
                    return HttpResponse::InternalServerError().json(json!({
                        "status": "error",
                        "error": "ERR_PLAYBACK_START_FAILED",
                        "message": format!("Failed to load actions: {}", e)
                    }));
                }
            };

            tokio::spawn(async move {
                let base_delay_ms: u64 = 500;
                let delay = std::time::Duration::from_millis(
                    (base_delay_ms as f64 / speed as f64) as u64
                );

                tracing::info!(
                    "[PLAYBACK] Starting execution: {} actions at {}x speed on device {}",
                    actions.len(), speed, udid
                );

                for (index, action) in actions.iter().enumerate() {
                    // Check playback state (pause/stop support)
                    loop {
                        match recording_service.get_playback_session(&udid).await {
                            Some(session) => match session.status {
                                PlaybackStatus::Playing => break,
                                PlaybackStatus::Paused => {
                                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                                    continue;
                                }
                                PlaybackStatus::Stopped | PlaybackStatus::Completed => {
                                    tracing::info!("[PLAYBACK] Stopped/completed for device {}", udid);
                                    return;
                                }
                            },
                            None => {
                                tracing::info!("[PLAYBACK] Session removed for device {}", udid);
                                return;
                            }
                        }
                    }

                    // Execute the action
                    tracing::debug!(
                        "[PLAYBACK] Executing action {}/{}: {:?} on {}",
                        index + 1, actions.len(), action.action_type, udid
                    );

                    if let Err(e) = execute_action_on_device(&app_state, &udid, action).await {
                        tracing::error!(
                            "[PLAYBACK] Action {}/{} failed on {}: {}",
                            index + 1, actions.len(), udid, e
                        );
                        // Continue with remaining actions despite errors
                    }

                    // Advance progress
                    if let Err(e) = recording_service.advance_playback(&udid).await {
                        tracing::error!("[PLAYBACK] Failed to advance progress: {}", e);
                    }

                    // Inter-action delay (skip after last action)
                    if index < actions.len() - 1 {
                        tokio::time::sleep(delay).await;
                    }
                }

                // Mark playback as completed
                if let Err(e) = recording_service.mark_playback_complete(&udid).await {
                    tracing::warn!("[PLAYBACK] Failed to mark complete: {}", e);
                    // Clean up session
                    let _ = recording_service.stop_playback(&udid).await;
                }
                tracing::info!("[PLAYBACK] Completed all {} actions on device {}", actions.len(), udid);
            });

            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let (status, error_code) = if e.contains("not found") {
                (actix_web::http::StatusCode::NOT_FOUND, "ERR_RECORDING_NOT_FOUND")
            } else if e == "ERR_RECORDING_HAS_NO_ACTIONS" {
                (actix_web::http::StatusCode::BAD_REQUEST, "ERR_RECORDING_HAS_NO_ACTIONS")
            } else if e == "ERR_PLAYBACK_ALREADY_ACTIVE" {
                (actix_web::http::StatusCode::BAD_REQUEST, "ERR_PLAYBACK_ALREADY_ACTIVE")
            } else {
                (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "ERR_PLAYBACK_START_FAILED")
            };
            HttpResponse::build(status).json(json!({
                "status": "error",
                "error": error_code,
                "message": e
            }))
        }
    }
}

/// GET /api/recordings/{id}/playback/status - Get playback status
pub async fn get_playback_status(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let _recording_id = path.into_inner();

    // Get target_device_udid from query params
    let target_device_udid = match query.get("target_device_udid") {
        Some(udid) => udid.clone(),
        None => {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_MISSING_DEVICE_UDID",
                "message": "target_device_udid query parameter is required"
            }));
        }
    };

    match state.recording_service.get_playback_status(&target_device_udid).await {
        Ok(status) => HttpResponse::Ok().json(status),
        Err(e) => {
            let error_code = if e == "ERR_NO_ACTIVE_PLAYBACK" {
                "ERR_NO_ACTIVE_PLAYBACK"
            } else {
                "ERR_PLAYBACK_STATUS_FAILED"
            };
            HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": error_code,
                "message": e
            }))
        }
    }
}

/// POST /api/recordings/{id}/playback/stop - Stop playback
pub async fn stop_playback(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let _recording_id = path.into_inner();

    // Get target_device_udid from query params
    let target_device_udid = match query.get("target_device_udid") {
        Some(udid) => udid.clone(),
        None => {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_MISSING_DEVICE_UDID",
                "message": "target_device_udid query parameter is required"
            }));
        }
    };

    match state.recording_service.stop_playback(&target_device_udid).await {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Playback stopped"
        })),
        Err(e) => {
            let error_code = if e == "ERR_NO_ACTIVE_PLAYBACK" {
                "ERR_NO_ACTIVE_PLAYBACK"
            } else {
                "ERR_PLAYBACK_STOP_FAILED"
            };
            HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": error_code,
                "message": e
            }))
        }
    }
}

/// POST /api/recordings/{id}/playback/pause - Pause playback
pub async fn pause_playback(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let _recording_id = path.into_inner();

    // Get target_device_udid from query params
    let target_device_udid = match query.get("target_device_udid") {
        Some(udid) => udid.clone(),
        None => {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_MISSING_DEVICE_UDID",
                "message": "target_device_udid query parameter is required"
            }));
        }
    };

    match state.recording_service.pause_playback_session(&target_device_udid).await {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Playback paused"
        })),
        Err(e) => {
            let error_code = if e == "ERR_NO_ACTIVE_PLAYBACK" {
                "ERR_NO_ACTIVE_PLAYBACK"
            } else if e == "ERR_PLAYBACK_NOT_PLAYING" {
                "ERR_PLAYBACK_NOT_PLAYING"
            } else {
                "ERR_PLAYBACK_PAUSE_FAILED"
            };
            HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": error_code,
                "message": e
            }))
        }
    }
}

/// POST /api/recordings/{id}/playback/resume - Resume playback
pub async fn resume_playback(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let _recording_id = path.into_inner();

    // Get target_device_udid from query params
    let target_device_udid = match query.get("target_device_udid") {
        Some(udid) => udid.clone(),
        None => {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_MISSING_DEVICE_UDID",
                "message": "target_device_udid query parameter is required"
            }));
        }
    };

    match state.recording_service.resume_playback_session(&target_device_udid).await {
        Ok(()) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": "Playback resumed"
        })),
        Err(e) => {
            let error_code = if e == "ERR_NO_ACTIVE_PLAYBACK" {
                "ERR_NO_ACTIVE_PLAYBACK"
            } else if e == "ERR_PLAYBACK_NOT_PAUSED" {
                "ERR_PLAYBACK_NOT_PAUSED"
            } else {
                "ERR_PLAYBACK_RESUME_FAILED"
            };
            HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": error_code,
                "message": e
            }))
        }
    }
}
