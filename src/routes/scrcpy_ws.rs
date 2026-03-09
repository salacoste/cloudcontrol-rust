use crate::services::scrcpy_manager::ScrcpyManager;
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde_json::json;
use tokio::sync::broadcast;

/// GET /scrcpy/{udid}/ws → Binary WebSocket for scrcpy video + control.
///
/// Connects to an existing managed scrcpy session (started via POST /scrcpy/{udid}/start).
/// Multiple clients can connect simultaneously — all receive the same broadcast frames.
/// Control messages (touch/key) are forwarded to the scrcpy control socket.
pub async fn scrcpy_websocket(
    state: web::Data<AppState>,
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().body("Missing udid");
    }

    let (resp, mut session, mut msg_stream) = match actix_ws::handle(&req, stream) {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().body(format!("WS error: {}", e)),
    };

    let state = state.into_inner().clone();
    let udid_clone = udid.clone();

    actix_web::rt::spawn(async move {
        // Look up managed session — must already be started via REST
        let (info, control_handle) =
            match state.scrcpy_manager.get_session_with_info(&udid_clone) {
                Ok(v) => v,
                Err(_) => {
                    let _ = session
                        .text(
                            serde_json::to_string(&json!({
                                "type": "error",
                                "message": format!("No active scrcpy session for device '{}'. Start one with POST /scrcpy/{}/start", udid_clone, udid_clone)
                            }))
                            .unwrap(),
                        )
                        .await;
                    let _ = session.close(None).await;
                    return;
                }
            };

        // Subscribe to broadcast channel for video frames
        let mut video_rx = match state.scrcpy_manager.subscribe_video(&udid_clone) {
            Ok(rx) => rx,
            Err(_) => {
                let _ = session
                    .text(
                        serde_json::to_string(
                            &json!({"type": "error", "message": "Session disappeared during connect"}),
                        )
                        .unwrap(),
                    )
                    .await;
                let _ = session.close(None).await;
                return;
            }
        };

        // AC3: Send metadata init message (JSON text frame) before binary frames
        let init_msg = json!({
            "type": "init",
            "codec": "h264",
            "width": info.width,
            "height": info.height,
            "deviceName": info.device_name,
        });
        if session
            .text(serde_json::to_string(&init_msg).unwrap())
            .await
            .is_err()
        {
            return;
        }

        tracing::info!(
            "[Scrcpy WS] Viewer connected for {} ({}x{})",
            udid_clone,
            info.width,
            info.height
        );

        // Spawn video consumer task: receives broadcast frames → sends to this WS client
        let session_clone = session.clone();
        let udid_video = udid_clone.clone();
        let video_task = tokio::spawn(async move {
            let mut session = session_clone;
            loop {
                match video_rx.recv().await {
                    Ok(frame) => {
                        // Frame is pre-serialized (flags + size + NAL data).
                        // Bytes::clone() is O(1) — just increments a ref count.
                        if session.binary(frame.data.clone()).await.is_err() {
                            break; // Client disconnected
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!(
                            "[Scrcpy WS] Viewer for {} lagged, skipped {} frames",
                            udid_video,
                            n
                        );
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!(
                            "[Scrcpy WS] Broadcast closed for {} (session stopped)",
                            udid_video
                        );
                        let _ = session
                            .text(
                                serde_json::to_string(
                                    &json!({"type": "error", "message": "Session stopped"}),
                                )
                                .unwrap(),
                            )
                            .await;
                        let _ = session.close(None).await;
                        break;
                    }
                }
            }
        });

        // Control receive loop: browser WS binary → scrcpy control socket
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                actix_ws::Message::Binary(data) => {
                    let mut s = control_handle.lock().await;
                    // Forward raw binary directly to scrcpy control socket
                    if data.len() >= 2 {
                        let msg_type = data[0];
                        match msg_type {
                            // Touch event from browser (28 bytes)
                            2 => {
                                if data.len() >= 28 {
                                    if let Err(e) = s
                                        .send_touch(
                                            data[1],
                                            u32::from_be_bytes(
                                                data[10..14].try_into().unwrap(),
                                            ),
                                            u32::from_be_bytes(
                                                data[14..18].try_into().unwrap(),
                                            ),
                                            u16::from_be_bytes(
                                                data[18..20].try_into().unwrap(),
                                            ),
                                            u16::from_be_bytes(
                                                data[20..22].try_into().unwrap(),
                                            ),
                                            u16::from_be_bytes(
                                                data[22..24].try_into().unwrap(),
                                            ),
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            "[Scrcpy WS] Touch send error: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            // Key event from browser (14 bytes)
                            0 => {
                                if data.len() >= 14 {
                                    if let Err(e) = s
                                        .send_key(
                                            data[1],
                                            u32::from_be_bytes(
                                                data[2..6].try_into().unwrap(),
                                            ),
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            "[Scrcpy WS] Key send error: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                actix_ws::Message::Close(_) => break,
                _ => {}
            }
        }

        // Cleanup: abort video consumer task for this viewer and close WS
        video_task.abort();
        let _ = session.close(None).await;

        tracing::info!("[Scrcpy WS] Viewer disconnected for {}", udid_clone);
    });

    resp
}

/// GET /scrcpy/{udid}/status → check if scrcpy is available for this device
pub async fn scrcpy_status(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    let phone_service =
        crate::services::phone_service::PhoneService::new(state.db.clone());
    let device = match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(d)) => d,
        _ => {
            return HttpResponse::Ok()
                .json(json!({"available": false, "reason": "device not found"}));
        }
    };

    let serial = device
        .get("serial")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if serial.is_empty() {
        return HttpResponse::Ok().json(json!({"available": false, "reason": "no serial"}));
    }

    // Check if JAR is available locally
    let jar_available = ScrcpyManager::jar_available();

    HttpResponse::Ok().json(json!({
        "available": jar_available,
        "serial": serial,
    }))
}
