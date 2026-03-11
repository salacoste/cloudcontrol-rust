use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;

/// GET /video/convert?fps=N&udid=UDID&name=MODEL → WebSocket for JPEG-to-MP4 recording.
///
/// Accepts binary WebSocket frames containing JPEG data.
/// Frames are piped to FFmpeg which encodes them into an MP4 file.
/// Closing the WebSocket connection automatically stops the recording.
pub async fn video_convert_ws(
    state: web::Data<AppState>,
    req: HttpRequest,
    stream: web::Payload,
) -> HttpResponse {
    // Parse query parameters from URL (with percent-decoding)
    let query = req.query_string();
    let params: std::collections::HashMap<String, String> = query
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?.to_string();
            let raw_val = parts.next().unwrap_or("");
            let val = urlencoding::decode(raw_val).unwrap_or_else(|_| raw_val.into()).into_owned();
            Some((key, val))
        })
        .collect();

    let udid = match params.get("udid") {
        Some(u) if !u.is_empty() => u.clone(),
        _ => return HttpResponse::BadRequest().body("Missing required query parameter: udid"),
    };

    let fps: u32 = params
        .get("fps")
        .and_then(|f: &String| f.parse().ok())
        .unwrap_or(2)
        .max(1)
        .min(30); // Clamp to 1-30 FPS

    let device_name = params.get("name").cloned();

    // Check FFmpeg availability
    if !state.ffmpeg_available {
        return HttpResponse::ServiceUnavailable().body(
            "FFmpeg is required for video recording but is not available on this system",
        );
    }

    // Upgrade to WebSocket
    let (resp, mut session, mut msg_stream) = match actix_ws::handle(&req, stream) {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::InternalServerError().body(format!("WebSocket error: {}", e))
        }
    };

    let state = state.into_inner().clone();

    actix_web::rt::spawn(async move {
        // Start recording
        let recording_id = match state.video_service.start_recording(&udid, fps, device_name).await {
            Ok(info) => {
                let _ = session
                    .text(
                        serde_json::to_string(&serde_json::json!({
                            "type": "recording_started",
                            "id": info.id,
                            "fps": fps,
                        }))
                        .unwrap_or_default(),
                    )
                    .await;
                info.id
            }
            Err(e) => {
                tracing::error!(
                    "[VideoWS] Failed to start recording for {}: {}",
                    udid,
                    e
                );
                let _ = session
                    .text(
                        serde_json::to_string(&serde_json::json!({
                            "type": "error",
                            "message": format!("Failed to start recording: {}", e),
                        }))
                        .unwrap_or_default(),
                    )
                    .await;
                let _ = session.close(None).await;
                return;
            }
        };

        // Process incoming frames
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                actix_ws::Message::Binary(data) => {
                    if let Err(e) = state.video_service.feed_frame(&recording_id, &data).await {
                        tracing::warn!(
                            "[VideoWS] Frame write error for recording {}: {}",
                            recording_id,
                            e
                        );
                        break;
                    }
                }
                actix_ws::Message::Close(_) => {
                    break;
                }
                actix_ws::Message::Ping(data) => {
                    let _ = session.pong(&data).await;
                }
                _ => {} // Ignore text and other messages
            }
        }

        // Auto-stop recording on disconnect
        match state.video_service.stop_recording(&recording_id).await {
            Ok(info) => {
                tracing::info!(
                    "[VideoWS] Recording {} completed — {} frames, {:?}ms",
                    recording_id,
                    info.frame_count,
                    info.duration_ms
                );
                let _ = session
                    .text(
                        serde_json::to_string(&serde_json::json!({
                            "type": "recording_stopped",
                            "id": info.id,
                            "frame_count": info.frame_count,
                            "duration_ms": info.duration_ms,
                            "file_path": info.file_path,
                        }))
                        .unwrap_or_default(),
                    )
                    .await;
            }
            Err(e) => {
                tracing::warn!(
                    "[VideoWS] Failed to stop recording {}: {}",
                    recording_id,
                    e
                );
            }
        }

        let _ = session.close(None).await;
    });

    resp
}
