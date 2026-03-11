use crate::device::adb::Adb;
use crate::device::atx_client::AtxClient;
use crate::services::device_service::DeviceService;
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// NIO WebSocket handler — replaces Python `nio_channel.py`.
///
/// Message format: `{type, data, id}`
/// Event types: screenshot, touch, swipe, input, keyevent, subscribe, unsubscribe

/// GET /nio/{udid}/ws → WebSocket endpoint
pub async fn nio_websocket(
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

    // Track WebSocket connection (Story 12-6)
    state.metrics.increment_ws_count();

    actix_web::rt::spawn(async move {
        // Get device info and create client
        let phone_service =
            crate::services::phone_service::PhoneService::new(state.db.clone());
        let device = match phone_service.query_info_by_udid(&udid_clone).await {
            Ok(Some(d)) => d,
            _ => {
                let _ = session
                    .text(
                        serde_json::to_string(&json!({"status":"error","message":"Device not found"}))
                            .unwrap(),
                    )
                    .await;
                let _ = session.close(None).await;
                state.metrics.decrement_ws_count(); // Story 12-6: cleanup on early exit
                return;
            }
        };

        let ip = device
            .get("ip")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let port = device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
        let serial = device
            .get("serial")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let client = Arc::new(AtxClient::new(&ip, port, &udid_clone));

        // Screenshot streaming state
        let screenshot_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>> =
            Arc::new(Mutex::new(None));
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

        tracing::info!("[NIO] WebSocket session started: {}", udid_clone);

        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                actix_ws::Message::Text(text) => {
                    let data: Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let event_type = data
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let event_data = data.get("data").cloned().unwrap_or(json!({}));

                    let result = match event_type {
                        "subscribe" => {
                            let target = data
                                .get("target")
                                .or_else(|| event_data.get("target"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            if target == "screenshot" {
                                let interval_ms = data
                                    .get("interval")
                                    .or_else(|| event_data.get("interval"))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(50);

                                // Start screenshot streaming
                                let mut task_guard = screenshot_task.lock().await;
                                if task_guard.is_none() || task_guard.as_ref().unwrap().is_finished()
                                {
                                    let client = client.clone();
                                    let mut session_clone = session.clone();
                                    let running = running.clone();
                                    let serial = device
                                        .get("serial")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let is_usb = Adb::is_usb_serial(&serial);
                                    let metrics = state.metrics.clone(); // Story 12-6: for latency recording

                                    let handle = tokio::spawn(async move {
                                        let min_interval =
                                            Duration::from_millis(interval_ms.max(30));
                                        let mut frame_count: u64 = 0;
                                        let stream_start = std::time::Instant::now();

                                        while running.load(std::sync::atomic::Ordering::Relaxed) {
                                            let start = std::time::Instant::now();

                                            // Primary: u2 JSON-RPC takeScreenshot (device-side scale+compress)
                                            // Fallback: ADB screencap (slow, 1.3s per frame)
                                            let result = client
                                                .screenshot_scaled(0.5, 40)
                                                .await
                                                .or_else(|_| -> Result<Vec<u8>, String> {
                                                    // Will try ADB fallback synchronously — NOT used inline,
                                                    // schedule it as async below
                                                    Err("u2 failed".into())
                                                });

                                            // If u2 failed, try ADB screencap as fallback
                                            let result = match result {
                                                Ok(bytes) => Ok(bytes),
                                                Err(_) if is_usb && !serial.is_empty() => {
                                                    tracing::warn!("[NIO] u2 failed, falling back to ADB screencap");
                                                    DeviceService::screenshot_usb_jpeg(
                                                        &serial, 40, 0.5,
                                                    )
                                                    .await
                                                }
                                                Err(e) => Err(e),
                                            };

                                            match result {
                                                Ok(jpeg_bytes) => {
                                                    let t_capture = start.elapsed();
                                                    let jpeg_size = jpeg_bytes.len();

                                                    // Record screenshot latency (Story 12-6)
                                                    metrics.record_screenshot_latency(t_capture.as_secs_f64());

                                                    let t_send_start = std::time::Instant::now();
                                                    // Send as binary WebSocket frame
                                                    if session_clone
                                                        .binary(jpeg_bytes)
                                                        .await
                                                        .is_err()
                                                    {
                                                        break;
                                                    }
                                                    let t_send = t_send_start.elapsed();

                                                    frame_count += 1;
                                                    let total = start.elapsed();
                                                    let avg_fps = frame_count as f64 / stream_start.elapsed().as_secs_f64();

                                                    // Log every 20 frames
                                                    if frame_count % 20 == 0 {
                                                        tracing::info!(
                                                            "[NIO] frame#{} | capture={:.0}ms | ws_send={:.0}ms | total={:.0}ms | {}KB | avg {:.1}fps",
                                                            frame_count,
                                                            t_capture.as_secs_f64() * 1000.0,
                                                            t_send.as_secs_f64() * 1000.0,
                                                            total.as_secs_f64() * 1000.0,
                                                            jpeg_size / 1024,
                                                            avg_fps,
                                                        );
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::error!(
                                                        "[NIO] Screenshot stream error: {}",
                                                        e
                                                    );
                                                    tokio::time::sleep(Duration::from_millis(500))
                                                        .await;
                                                    continue;
                                                }
                                            }

                                            // Smart interval: only sleep remaining time
                                            let elapsed = start.elapsed();
                                            if elapsed < min_interval {
                                                tokio::time::sleep(min_interval - elapsed).await;
                                            }
                                        }
                                    });

                                    *task_guard = Some(handle);
                                }

                                json!({"status": "ok", "type": "subscribed", "target": "screenshot"})
                            } else {
                                json!({"status": "ok", "type": "subscribed", "target": target})
                            }
                        }

                        "unsubscribe" => {
                            let target = data
                                .get("target")
                                .or_else(|| event_data.get("target"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            if target == "screenshot" {
                                let mut task_guard = screenshot_task.lock().await;
                                if let Some(handle) = task_guard.take() {
                                    handle.abort();
                                }
                            }

                            json!({"status": "ok", "type": "unsubscribed", "target": target})
                        }

                        "screenshot" => {
                            let quality = event_data
                                .get("quality")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(60) as u8;

                            // Primary: u2 JSON-RPC (device-side compress)
                            // Fallback: ADB screencap
                            let serial = device
                                .get("serial")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let result = client.screenshot_scaled(0.5, quality).await
                                .or_else(|_| -> Result<Vec<u8>, String> { Err("u2 failed".into()) });
                            let result = match result {
                                Ok(bytes) => Ok(bytes),
                                Err(_) if Adb::is_usb_serial(serial) && !serial.is_empty() => {
                                    DeviceService::screenshot_usb_jpeg(serial, quality, 0.5).await
                                }
                                Err(e) => Err(e),
                            };

                            match result {
                                Ok(jpeg_bytes) => {
                                    let _ = session.binary(jpeg_bytes).await;
                                    continue;
                                }
                                Err(e) => json!({"status": "error", "message": e}),
                            }
                        }

                        "touch" => {
                            let x = event_data
                                .get("x")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0) as i32;
                            let y = event_data
                                .get("y")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0) as i32;

                            match client.click(x, y).await {
                                Ok(_) => json!({"status": "ok", "type": "touch"}),
                                Err(e) => json!({"status": "error", "message": e}),
                            }
                        }

                        "swipe" => {
                            let x1 = event_data.get("x1").and_then(|v| v.as_i64()).unwrap_or(0)
                                as i32;
                            let y1 = event_data.get("y1").and_then(|v| v.as_i64()).unwrap_or(0)
                                as i32;
                            let x2 = event_data.get("x2").and_then(|v| v.as_i64()).unwrap_or(0)
                                as i32;
                            let y2 = event_data.get("y2").and_then(|v| v.as_i64()).unwrap_or(0)
                                as i32;
                            let duration_ms = (event_data.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.2) * 1000.0) as i32;

                            // Use ADB input swipe — ATX JSON-RPC swipe silently fails on MIUI
                            if !serial.is_empty() {
                                match Adb::input_swipe(&serial, x1, y1, x2, y2, duration_ms.max(50).min(2000)).await {
                                    Ok(_) => json!({"status": "ok", "type": "swipe"}),
                                    Err(e) => json!({"status": "error", "message": e}),
                                }
                            } else {
                                match client.swipe(x1, y1, x2, y2, (duration_ms as f64 / 1000.0).max(0.05).min(2.0)).await {
                                    Ok(_) => json!({"status": "ok", "type": "swipe"}),
                                    Err(e) => json!({"status": "error", "message": e}),
                                }
                            }
                        }

                        "input" => {
                            let text = event_data
                                .get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            if !text.is_empty() {
                                match client.input_text(text).await {
                                    Ok(_) => json!({"status": "ok", "type": "input"}),
                                    Err(e) => json!({"status": "error", "message": e}),
                                }
                            } else {
                                json!({"status": "ok", "type": "input"})
                            }
                        }

                        "keyevent" => {
                            let key = event_data
                                .get("key")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            let android_key = match key {
                                "Enter" => "enter",
                                "Backspace" => "del",
                                "Delete" => "forward_del",
                                "Home" => "home",
                                "Back" => "back",
                                "Tab" => "tab",
                                "Escape" => "back",
                                "ArrowUp" => "dpad_up",
                                "ArrowDown" => "dpad_down",
                                "ArrowLeft" => "dpad_left",
                                "ArrowRight" => "dpad_right",
                                other => other,
                            };

                            match client.press_key(android_key).await {
                                Ok(_) => json!({"status": "ok", "type": "keyevent"}),
                                Err(e) => json!({"status": "error", "message": e}),
                            }
                        }

                        _ => json!({"status": "error", "message": format!("Unknown event: {}", event_type)}),
                    };

                    let _ = session
                        .text(serde_json::to_string(&result).unwrap())
                        .await;
                }

                actix_ws::Message::Close(_) => break,
                _ => {}
            }
        }

        // Cleanup
        running.store(false, std::sync::atomic::Ordering::Relaxed);
        let mut task_guard = screenshot_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }

        // Decrement WebSocket count (Story 12-6)
        state.metrics.decrement_ws_count();

        tracing::info!("[NIO] WebSocket session closed: {}", udid_clone);
    });

    resp
}

/// GET /nio/stats → performance statistics JSON
pub async fn nio_stats(state: web::Data<AppState>) -> HttpResponse {
    let stats = json!({
        "connection_pool": state.connection_pool.stats(),
        "sessions": state.heartbeat_sessions.len(),
    });
    HttpResponse::Ok().json(stats)
}
