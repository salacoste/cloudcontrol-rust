use crate::services::scrcpy_manager::ScrcpyManager;
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde_json::json;
use tokio::sync::broadcast;

// ── Safe binary parsing helpers (Story 12-5: Crash Protection) ──

/// Parse touch event bytes safely. Returns None if data is malformed.
/// Expected format: 24 bytes minimum (full message is 28 bytes with pointer_id)
/// - [0]: msg_type (2)
/// - [1]: action (down/up/move)
/// - [2..10]: pointer_id (64-bit, not used)
/// - [10..14]: x (u32)
/// - [14..18]: y (u32)
/// - [18..20]: screen_width (u16)
/// - [20..22]: screen_height (u16)
/// - [22..24]: pressure (u16)
#[inline]
fn parse_touch_bytes(data: &[u8]) -> Option<(u8, u32, u32, u16, u16, u16)> {
    if data.len() < 24 {
        tracing::warn!(
            "[Scrcpy WS] Touch event too short: {} bytes, expected >= 24",
            data.len()
        );
        return None;
    }
    Some((
        data[1], // action
        u32::from_be_bytes(data[10..14].try_into().ok()?),
        u32::from_be_bytes(data[14..18].try_into().ok()?),
        u16::from_be_bytes(data[18..20].try_into().ok()?),
        u16::from_be_bytes(data[20..22].try_into().ok()?),
        u16::from_be_bytes(data[22..24].try_into().ok()?),
    ))
}

/// Parse key event bytes safely. Returns None if data is malformed.
/// Expected format: 14 bytes total
/// - [0]: msg_type (0)
/// - [1]: action (down/up)
/// - [2..6]: keycode (u32)
/// - [6..14]: metastate (64-bit, not used)
#[inline]
fn parse_key_bytes(data: &[u8]) -> Option<(u8, u32)> {
    if data.len() < 6 {
        tracing::warn!(
            "[Scrcpy WS] Key event too short: {} bytes, expected >= 6",
            data.len()
        );
        return None;
    }
    Some((
        data[1], // action
        u32::from_be_bytes(data[2..6].try_into().ok()?),
    ))
}

/// Safe JSON serialization that never panics.
/// Returns a JSON error message string if serialization fails.
#[inline]
fn safe_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| {
        tracing::warn!("[Scrcpy WS] JSON serialization failed: {}", e);
        r#"{"type":"error","message":"internal serialization error"}"#.to_string()
    })
}

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
                        .text(safe_json(&json!({
                            "type": "error",
                            "message": format!("No active scrcpy session for device '{}'. Start one with POST /scrcpy/{}/start", udid_clone, udid_clone)
                        })))
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
                    .text(safe_json(&json!({
                        "type": "error",
                        "message": "Session disappeared during connect"
                    })))
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
        if session.text(safe_json(&init_msg)).await.is_err() {
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
                            .text(safe_json(&json!({
                                "type": "error",
                                "message": "Session stopped"
                            })))
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
                            // Touch event from browser (Story 12-5: safe parsing)
                            2 => {
                                if let Some((action, x, y, w, h, pressure)) =
                                    parse_touch_bytes(&data)
                                {
                                    if let Err(e) =
                                        s.send_touch(action, x, y, w, h, pressure).await
                                    {
                                        tracing::warn!(
                                            "[Scrcpy WS] Touch send error: {}",
                                            e
                                        );
                                    }
                                }
                                // If parsing fails, skip message (already logged in helper)
                            }
                            // Key event from browser (Story 12-5: safe parsing)
                            0 => {
                                if let Some((action, keycode)) = parse_key_bytes(&data) {
                                    if let Err(e) = s.send_key(action, keycode).await {
                                        tracing::warn!(
                                            "[Scrcpy WS] Key send error: {}",
                                            e
                                        );
                                    }
                                }
                                // If parsing fails, skip message (already logged in helper)
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

// ── Unit tests for safe binary parsing (Story 12-5) ──
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_touch_bytes_valid() {
        // Create a valid touch message (24 bytes minimum)
        let mut data = vec![0u8; 28];
        data[0] = 2; // msg_type = touch
        data[1] = 0; // action = down
        // x at bytes 10-14
        data[10..14].copy_from_slice(&100u32.to_be_bytes());
        // y at bytes 14-18
        data[14..18].copy_from_slice(&200u32.to_be_bytes());
        // width at bytes 18-20
        data[18..20].copy_from_slice(&1080u16.to_be_bytes());
        // height at bytes 20-22
        data[20..22].copy_from_slice(&1920u16.to_be_bytes());
        // pressure at bytes 22-24
        data[22..24].copy_from_slice(&65535u16.to_be_bytes());

        let result = parse_touch_bytes(&data);
        assert!(result.is_some());
        let (action, x, y, w, h, pressure) = result.unwrap();
        assert_eq!(action, 0);
        assert_eq!(x, 100);
        assert_eq!(y, 200);
        assert_eq!(w, 1080);
        assert_eq!(h, 1920);
        assert_eq!(pressure, 65535);
    }

    #[test]
    fn test_parse_touch_bytes_too_short() {
        // Create data that's too short (< 24 bytes)
        let data = vec![2u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let result = parse_touch_bytes(&data);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_touch_bytes_exactly_24_bytes() {
        // Create exactly 24 bytes (minimum valid)
        let mut data = vec![0u8; 24];
        data[0] = 2; // msg_type = touch
        data[1] = 1; // action = up
        data[10..14].copy_from_slice(&500u32.to_be_bytes());

        let result = parse_touch_bytes(&data);
        assert!(result.is_some());
        let (action, x, _, _, _, _) = result.unwrap();
        assert_eq!(action, 1);
        assert_eq!(x, 500);
    }

    #[test]
    fn test_parse_key_bytes_valid() {
        // Create a valid key message (6 bytes minimum)
        let mut data = vec![0u8; 14];
        data[0] = 0; // msg_type = key
        data[1] = 0; // action = down
        data[2..6].copy_from_slice(&67u32.to_be_bytes()); // KEYCODE_C = 67

        let result = parse_key_bytes(&data);
        assert!(result.is_some());
        let (action, keycode) = result.unwrap();
        assert_eq!(action, 0);
        assert_eq!(keycode, 67);
    }

    #[test]
    fn test_parse_key_bytes_too_short() {
        // Create data that's too short (< 6 bytes)
        let data = vec![0u8, 1, 0, 0];

        let result = parse_key_bytes(&data);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_key_bytes_exactly_6_bytes() {
        // Create exactly 6 bytes (minimum valid)
        let mut data = vec![0u8; 6];
        data[0] = 0; // msg_type = key
        data[1] = 1; // action = up
        data[2..6].copy_from_slice(&66u32.to_be_bytes()); // KEYCODE_B = 66

        let result = parse_key_bytes(&data);
        assert!(result.is_some());
        let (action, keycode) = result.unwrap();
        assert_eq!(action, 1);
        assert_eq!(keycode, 66);
    }

    #[test]
    fn test_safe_json_valid() {
        use serde_json::json;
        let value = json!({"type": "test", "count": 42});
        let result = safe_json(&value);
        assert!(result.contains("\"type\":\"test\""));
        assert!(result.contains("\"count\":42"));
    }

    #[test]
    fn test_safe_json_never_panics() {
        // safe_json should never panic, even on complex nested structures
        use serde_json::json;
        let value = json!({
            "type": "init",
            "codec": "h264",
            "width": 1920,
            "height": 1080,
            "deviceName": "Test Device",
        });
        let result = safe_json(&value);
        assert!(!result.is_empty());
    }
}
