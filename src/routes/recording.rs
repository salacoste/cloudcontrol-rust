use crate::models::recording::{
    RecordActionRequest, StartRecordingRequest,
    StopRecordingRequest, EditActionRequest,
};
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
