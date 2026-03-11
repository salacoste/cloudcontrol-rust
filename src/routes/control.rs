use crate::device::adb::Adb;
use crate::device::atx_client::AtxClient;
use crate::models::recording::{ActionType, RecordActionRequest};
use crate::services::device_resolver::DeviceResolver;
use crate::services::device_service::DeviceService;
use crate::state::AppState;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use base64::Engine;
use futures::StreamExt;
use serde_json::{json, Value};
use std::io::Cursor;
use std::sync::Arc;

// ─── Mock screenshot for stress testing ───

fn generate_mock_screenshot() -> String {
    let img = image::RgbImage::from_fn(1080, 2400, |_, _| image::Rgb([50u8, 50, 50]));
    let mut buf = Cursor::new(Vec::new());
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 50);
    encoder.encode_image(&img).ok();
    base64::engine::general_purpose::STANDARD.encode(buf.into_inner())
}

static MOCK_DATA: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn get_mock_screenshot() -> &'static str {
    MOCK_DATA.get_or_init(generate_mock_screenshot)
}

// ═══════════════ DEVICE RESOLUTION HELPER ═══════════════

/// Helper function to get device info and ATX client.
/// Uses the shared DeviceResolver module (Story 13-1).
async fn get_device_client(
    state: &AppState,
    udid: &str,
) -> Result<(Value, Arc<AtxClient>), HttpResponse> {
    DeviceResolver::new(state)
        .get_device_client(udid)
        .await
        .map_err(|e| e.into())
}

// ═══════════════ PAGE ROUTES ═══════════════

/// GET / → 302 redirect to /async
pub async fn index(_state: web::Data<AppState>) -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", "/async"))
        .finish()
}

/// GET /devices/{udid}/remote → remote.html
pub async fn remote(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let phone_service = state.phone_service.clone();
    let device = match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(d)) => d,
        _ => return HttpResponse::NotFound().body("Device not found"),
    };

    let mut ctx = tera::Context::new();
    ctx.insert("Udid", &udid);
    ctx.insert("deviceInfo", &device);
    ctx.insert("device", &device.to_string());
    ctx.insert("v", &json!({}));

    match state.tera.render("remote.html", &ctx) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

/// GET /async → device_synchronous.html (auto-load all online devices)
pub async fn async_list_get(state: web::Data<AppState>) -> HttpResponse {
    let phone_service = state.phone_service.clone();
    let devices = match phone_service.query_device_list_by_present().await {
        Ok(d) => d,
        Err(_) => vec![],
    };

    let mut ctx = tera::Context::new();

    if devices.is_empty() {
        ctx.insert("list", "[]");
        ctx.insert("Width", &0i64);
        ctx.insert("Height", &0i64);
        ctx.insert("Udid", "");
        ctx.insert("deviceInfo", &json!({}));
        ctx.insert("device", &json!({}));
        ctx.insert("v", "");
    } else {
        let ip_list: Vec<Value> = devices.iter().map(|dev| {
            let display = dev.get("display").cloned().unwrap_or(json!({"width":1080,"height":1920}));
            json!({
                "src": dev.get("ip").and_then(|v| v.as_str()).unwrap_or(""),
                "des": dev.get("ip").and_then(|v| v.as_str()).unwrap_or(""),
                "width": display.get("width").and_then(|v| v.as_i64()).unwrap_or(1080),
                "height": display.get("height").and_then(|v| v.as_i64()).unwrap_or(1920),
                "port": dev.get("port").and_then(|v| v.as_i64()).unwrap_or(9008),
                "udid": dev.get("udid").and_then(|v| v.as_str()).unwrap_or(""),
                "model": dev.get("model").and_then(|v| v.as_str()).unwrap_or(""),
            })
        }).collect();

        let first = &devices[0];
        let first_display = first.get("display").cloned().unwrap_or(json!({"width":1080,"height":1920}));

        ctx.insert("list", &serde_json::to_string(&ip_list).unwrap_or_default());
        ctx.insert("Width", &first_display.get("width").and_then(|v| v.as_i64()).unwrap_or(1080));
        ctx.insert("Height", &first_display.get("height").and_then(|v| v.as_i64()).unwrap_or(1920));
        ctx.insert("Udid", first.get("udid").and_then(|v| v.as_str()).unwrap_or(""));
        ctx.insert("deviceInfo", &json!({}));
        ctx.insert("device", &json!({}));
        ctx.insert("v", "");
    }

    match state.tera.render("device_synchronous.html", &ctx) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

/// POST /async → device_synchronous.html
pub async fn async_list_page(
    state: web::Data<AppState>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let udids_str = match form.get("devices") {
        Some(d) => d,
        None => return HttpResponse::BadRequest().body("Missing devices"),
    };

    let udid_list: Vec<&str> = udids_str.split(',').collect();
    let phone_service = state.phone_service.clone();

    let mut ip_list = Vec::new();
    let mut first_device: Option<Value> = None;

    for (i, udid) in udid_list.iter().enumerate() {
        if let Ok(Some(dev)) = phone_service.query_info_by_udid(udid).await {
            if i == 0 {
                first_device = Some(dev.clone());
            }
            let display = dev.get("display").cloned().unwrap_or(json!({"width":1080,"height":1920}));
            ip_list.push(json!({
                "src": dev.get("ip").and_then(|v| v.as_str()).unwrap_or(""),
                "des": dev.get("ip").and_then(|v| v.as_str()).unwrap_or(""),
                "width": display.get("width").and_then(|v| v.as_i64()).unwrap_or(1080),
                "height": display.get("height").and_then(|v| v.as_i64()).unwrap_or(1920),
                "port": dev.get("port").and_then(|v| v.as_i64()).unwrap_or(9008),
                "udid": dev.get("udid").and_then(|v| v.as_str()).unwrap_or(""),
                "model": dev.get("model").and_then(|v| v.as_str()).unwrap_or(""),
            }));
        }
    }

    let device = first_device.unwrap_or(json!({"ip":"","port":9008,"display":{"width":1080,"height":1920},"udid":""}));
    let display = device.get("display").cloned().unwrap_or(json!({"width":1080,"height":1920}));

    let mut ctx = tera::Context::new();
    ctx.insert("list", &serde_json::to_string(&ip_list).unwrap_or_default());
    ctx.insert("Width", &display.get("width").and_then(|v| v.as_i64()).unwrap_or(1080));
    ctx.insert("Height", &display.get("height").and_then(|v| v.as_i64()).unwrap_or(1920));
    ctx.insert("Udid", device.get("udid").and_then(|v| v.as_str()).unwrap_or(""));
    ctx.insert("deviceInfo", &json!({}));
    ctx.insert("device", &json!({}));
    ctx.insert("v", "{{v.des}}");

    match state.tera.render("device_synchronous.html", &ctx) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

/// GET /installfile → file.html
pub async fn installfile(state: web::Data<AppState>) -> HttpResponse {
    match state.tera.render("file.html", &tera::Context::new()) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

/// GET /test → test.html (device diagnostic page, reads UDID from ?udid= query param)
pub async fn test_page(state: web::Data<AppState>) -> HttpResponse {
    match state.tera.render("test.html", &tera::Context::new()) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

// ═══════════════ DEVICE API ═══════════════

/// GET /list?tag=xxx → JSON array of online devices (optionally filtered by tag)
pub async fn device_list(
    state: web::Data<AppState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let phone_service = state.phone_service.clone();

    // Check if tag filter is provided
    if let Some(tag) = query.get("tag") {
        if !tag.is_empty() {
            match phone_service.query_devices_by_tag(tag).await {
                Ok(devices) => HttpResponse::Ok().json(devices),
                Err(e) => HttpResponse::InternalServerError().json(json!({"error": e})),
            }
        } else {
            match phone_service.query_device_list().await {
                Ok(devices) => HttpResponse::Ok().json(devices),
                Err(e) => HttpResponse::InternalServerError().json(json!({"error": e})),
            }
        }
    } else {
        match phone_service.query_device_list().await {
            Ok(devices) => HttpResponse::Ok().json(devices),
            Err(e) => HttpResponse::InternalServerError().json(json!({"error": e})),
        }
    }
}

/// GET /devices/{udid}/info → device info JSON (with product data if linked)
pub async fn device_info(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let phone_service = state.phone_service.clone();
    match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(device)) => {
            let mut device_obj = device.clone();
            if let Some(product_id) = device.get("product_id").and_then(|v| v.as_i64()) {
                match state.db.get_product(product_id).await {
                    Ok(Some(product)) => {
                        if let Some(obj) = device_obj.as_object_mut() {
                            obj.insert(
                                "product".to_string(),
                                serde_json::to_value(&product).unwrap_or_default(),
                            );
                        }
                    }
                    Ok(None) => {
                        if let Some(obj) = device_obj.as_object_mut() {
                            obj.insert("product".to_string(), Value::Null);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch product {} for device {}: {}", product_id, udid, e);
                        if let Some(obj) = device_obj.as_object_mut() {
                            obj.insert("product".to_string(), Value::Null);
                        }
                    }
                }
            } else if let Some(obj) = device_obj.as_object_mut() {
                obj.insert("product".to_string(), Value::Null);
            }
            HttpResponse::Ok().json(device_obj)
        }
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Device not found"})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": e})),
    }
}

/// GET /devices/{udid}/edit → edit.html (device product association page)
pub async fn edit_page(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let mut ctx = tera::Context::new();
    ctx.insert("Udid", &udid);

    match state.tera.render("edit.html", &ctx) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

/// PUT /devices/{udid}/product → save device-product association
pub async fn update_device_product(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: String,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    // Parse JSON body manually — edit.html's jQuery $.ajax sends JSON.stringify()
    // without setting contentType: "application/json", so web::Json would reject it
    let body: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::BadRequest().json(json!({"error": "Invalid JSON body"}));
        }
    };

    // Extract product id from body
    let product_id = match body.get("id").and_then(|v| v.as_i64()) {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(json!({"error": "Missing product id"}));
        }
    };

    // Validate product exists
    match state.db.get_product(product_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound().json(json!({"error": "Product not found"}));
        }
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(json!({"error": format!("Database error: {}", e)}));
        }
    }

    // Update device's product_id and verify device exists
    match state.db.update_device_product(&udid, product_id).await {
        Ok(true) => HttpResponse::Ok().json(json!({"status": "success"})),
        Ok(false) => HttpResponse::NotFound().json(json!({"error": "Device not found"})),
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({"error": format!("Failed to update device product: {}", e)})),
    }
}

/// GET /devices/{udid}/property → property.html (asset number page)
pub async fn property_page(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let phone_service = state.phone_service.clone();
    let current_property_id = match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(device)) => device
            .get("property_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        Ok(None) => {
            return HttpResponse::NotFound().json(json!({"error": "Device not found"}));
        }
        Err(e) => {
            tracing::warn!("Failed to query device {}: {}", udid, e);
            return HttpResponse::InternalServerError()
                .json(json!({"error": format!("Database error: {}", e)}));
        }
    };

    let mut ctx = tera::Context::new();
    ctx.insert("Udid", &udid);
    ctx.insert("CurrentPropertyId", &current_property_id);

    match state.tera.render("property.html", &ctx) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

/// GET /providers → providers.html (provider registry page)
pub async fn providers_page(state: web::Data<AppState>) -> HttpResponse {
    match state.tera.render("providers.html", &tera::Context::new()) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

/// POST /api/v1/devices/{udid}/property → save asset/property number
pub async fn update_device_property(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: String,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    // Parse JSON body manually — jQuery $.ajax sends without Content-Type header
    let body: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::BadRequest().json(json!({"error": "Invalid JSON body"}));
        }
    };

    // Extract property_id — support both "property_id" and "id" fields
    let property_id = body
        .get("property_id")
        .or_else(|| body.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    if property_id.is_empty() {
        return HttpResponse::BadRequest().json(json!({"error": "Missing property_id"}));
    }

    if property_id.len() > 100 {
        return HttpResponse::BadRequest()
            .json(json!({"error": "property_id exceeds maximum length of 100 characters"}));
    }

    match state.db.update_device_property(&udid, property_id).await {
        Ok(true) => HttpResponse::Ok().json(json!({"status": "success"})),
        Ok(false) => HttpResponse::NotFound().json(json!({"error": "Device not found"})),
        Err(e) => {
            tracing::warn!("Failed to update property for device {}: {}", udid, e);
            HttpResponse::InternalServerError()
                .json(json!({"error": format!("Database error: {}", e)}))
        }
    }
}

// ═══════════════ SCREENSHOT ═══════════════

/// GET /inspector/{udid}/screenshot → base64 JSON
pub async fn inspector_screenshot(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Mock device
    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HttpResponse::Ok().json(json!({
            "type": "jpeg",
            "encoding": "base64",
            "data": get_mock_screenshot(),
        }));
    }

    let quality: u8 = query
        .get("quality")
        .and_then(|v| v.parse().ok())
        .unwrap_or(70)
        .max(30)
        .min(95);
    let scale: f64 = query
        .get("scale")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0)
        .max(0.25)
        .min(1.0);

    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("");
    let is_usb = Adb::is_usb_serial(serial);

    // Primary: u2 JSON-RPC takeScreenshot with device-side scale+compress
    if let Ok(jpeg_bytes) = client.screenshot_scaled(scale, quality).await {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes);
        return HttpResponse::Ok().json(json!({
            "type": "jpeg",
            "encoding": "base64",
            "data": b64,
        }));
    }

    // Fallback 1: USB ADB screencap
    if is_usb && !serial.is_empty() {
        if let Ok(b64) = DeviceService::screenshot_usb_base64(serial, quality, scale).await {
            return HttpResponse::Ok().json(json!({
                "type": "jpeg",
                "encoding": "base64",
                "data": b64,
            }));
        }
    }

    // Fallback 2: u2 full screenshot + server-side resize
    match DeviceService::screenshot_base64(&client, quality, scale).await {
        Ok(b64) => HttpResponse::Ok().json(json!({
            "type": "jpeg",
            "encoding": "base64",
            "data": b64,
        })),
        Err(e) => {
            // Final fallback: ADB screencap
            if !serial.is_empty() && !is_usb {
                if let Ok(png_bytes) = Adb::screencap(serial).await {
                    if let Ok(b64) = DeviceService::encode_screenshot(&png_bytes, quality, scale) {
                        return HttpResponse::Ok().json(json!({
                            "type": "jpeg",
                            "encoding": "base64",
                            "data": b64,
                        }));
                    }
                }
            }
            HttpResponse::InternalServerError().json(json!({"status":"error","message":e}))
        }
    }
}

/// GET /inspector/{udid}/screenshot/img → JPEG binary with caching + dedup
pub async fn inspector_screenshot_img(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let quality: u8 = query
        .get("q")
        .and_then(|v| v.parse().ok())
        .unwrap_or(60)
        .max(20)
        .min(95);
    let scale: f64 = query
        .get("s")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.6)
        .max(0.3)
        .min(1.0);

    let cache_key = format!("{}_{}_{}", udid, quality, scale);

    // Check cache
    if let Some(cached) = state.screenshot_cache.get(&cache_key) {
        return HttpResponse::Ok()
            .content_type("image/jpeg")
            .insert_header(("Cache-Control", "no-cache"))
            .insert_header(("X-Cache", "HIT"))
            .body(cached);
    }

    // Request deduplication
    if let Some(mut rx) = state.screenshot_cache.try_subscribe(&cache_key) {
        // Wait for the in-flight request
        if rx.changed().await.is_ok() {
            if let Some(data) = rx.borrow().clone() {
                return HttpResponse::Ok()
                    .content_type("image/jpeg")
                    .insert_header(("Cache-Control", "no-cache"))
                    .insert_header(("X-Cache", "DEDUP"))
                    .body(data);
            }
        }
    }

    // Register pending request
    let sender = state.screenshot_cache.register_pending(&cache_key);

    let result = async {
        let (device, client) = get_device_client(&state, &udid).await?;
        let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("");
        let is_usb = Adb::is_usb_serial(serial);
        let t0 = std::time::Instant::now();

        // Primary: u2 JSON-RPC takeScreenshot (device-side scale+compress, fastest)
        if let Ok(jpeg_bytes) = client.screenshot_scaled(scale, quality).await {
            let elapsed = t0.elapsed().as_secs_f64();
            state.metrics.record_screenshot_latency(elapsed);
            tracing::info!(
                "[HTTP] /screenshot/img u2-scaled total={:.0}ms | {}KB",
                elapsed * 1000.0,
                jpeg_bytes.len() / 1024,
            );
            return Ok(jpeg_bytes);
        }

        // Fallback 1: USB ADB screencap
        if is_usb && !serial.is_empty() {
            if let Ok(jpeg_bytes) = DeviceService::screenshot_usb_jpeg(serial, quality, scale).await {
                let elapsed = t0.elapsed().as_secs_f64();
                state.metrics.record_screenshot_latency(elapsed);
                tracing::info!(
                    "[HTTP] /screenshot/img ADB-fallback total={:.0}ms | {}KB",
                    elapsed * 1000.0,
                    jpeg_bytes.len() / 1024,
                );
                return Ok(jpeg_bytes);
            }
        }

        // Fallback 2: u2 full screenshot + server-side resize
        match DeviceService::screenshot_jpeg(&client, quality, scale).await {
            Ok(bytes) => {
                let elapsed = t0.elapsed().as_secs_f64();
                state.metrics.record_screenshot_latency(elapsed);
                tracing::info!(
                    "[HTTP] /screenshot/img u2-resize total={:.0}ms | {}KB",
                    elapsed * 1000.0,
                    bytes.len() / 1024,
                );
                Ok(bytes)
            }
            Err(_) => {
                // Final fallback: ADB screencap
                if !serial.is_empty() && !is_usb {
                    if let Ok(png_bytes) = Adb::screencap(serial).await {
                        if let Ok(jpeg_bytes) = DeviceService::raw_screenshot_to_jpeg(&png_bytes, quality, scale) {
                            let elapsed = t0.elapsed().as_secs_f64();
                            state.metrics.record_screenshot_latency(elapsed);
                            return Ok(jpeg_bytes);
                        }
                    }
                }
                Err(HttpResponse::NotFound().body("Screenshot failed"))
            }
        }
    }
    .await;

    // Clean up pending
    state.screenshot_cache.clear_pending(&cache_key);

    match result {
        Ok(img_data) => {
            // Cache the result
            state.screenshot_cache.set(&cache_key, img_data.clone());

            // Notify waiting requests
            let _ = sender.send(Some(img_data.clone()));

            HttpResponse::Ok()
                .content_type("image/jpeg")
                .insert_header(("Cache-Control", "no-cache"))
                .insert_header(("X-Cache", "MISS"))
                .body(img_data)
        }
        Err(resp) => {
            let _ = sender.send(None);
            resp
        }
    }
}

// ═══════════════ BATCH SCREENSHOT ═══════════════

use serde::Deserialize;
use std::collections::HashMap as StdHashMap;

#[derive(Deserialize)]
pub struct BatchScreenshotRequest {
    devices: Vec<String>,
    #[serde(default)]
    quality: Option<u8>,
    #[serde(default)]
    scale: Option<f64>,
}

/// POST /api/screenshot/batch - capture screenshots from multiple devices concurrently.
/// Returns HTTP 207 Multi-Status for partial failures.
pub async fn batch_screenshot(
    state: web::Data<AppState>,
    body: web::Json<BatchScreenshotRequest>,
) -> HttpResponse {
    let devices = &body.devices;

    if devices.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "No devices specified"
        }));
    }

    // Batch size limit to prevent resource exhaustion
    const MAX_BATCH_SIZE: usize = 50;
    if devices.len() > MAX_BATCH_SIZE {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": format!("Too many devices. Maximum is {}", MAX_BATCH_SIZE)
        }));
    }

    // Check for duplicate UDIDs
    let mut seen = std::collections::HashSet::new();
    for udid in devices {
        if !seen.insert(udid) {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": format!("Duplicate device UDID: {}", udid)
            }));
        }
    }

    let quality: u8 = body.quality.unwrap_or(70).clamp(30, 95);
    let scale: f64 = body.scale.unwrap_or(1.0).clamp(0.25, 1.0);

    // Capture screenshots concurrently
    let mut tasks = Vec::new();
    for udid in devices.clone() {
        let state_clone = state.clone();
        let task = async move {
            let udid = udid.clone();
            let t0 = std::time::Instant::now();
            // Get device and client
            match get_device_client(&state_clone, &udid).await {
                Ok((device, client)) => {
                    // Mock device
                    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
                        return (udid, Ok(get_mock_screenshot().to_string()));
                    }

                    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("");
                    let is_usb = Adb::is_usb_serial(serial);

                    // Primary: u2 JSON-RPC takeScreenshot
                    if let Ok(jpeg_bytes) = client.screenshot_scaled(scale, quality).await {
                        let elapsed = t0.elapsed().as_secs_f64();
                        state_clone.metrics.record_screenshot_latency(elapsed);
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes);
                        return (udid, Ok(b64));
                    }

                    // Fallback 1: USB ADB screencap
                    if is_usb && !serial.is_empty() {
                        if let Ok(b64) = DeviceService::screenshot_usb_base64(serial, quality, scale).await {
                            let elapsed = t0.elapsed().as_secs_f64();
                            state_clone.metrics.record_screenshot_latency(elapsed);
                            return (udid, Ok(b64));
                        }
                    }

                    // Fallback 2: u2 full screenshot + server-side resize
                    match DeviceService::screenshot_base64(&client, quality, scale).await {
                        Ok(b64) => {
                            let elapsed = t0.elapsed().as_secs_f64();
                            state_clone.metrics.record_screenshot_latency(elapsed);
                            (udid, Ok(b64))
                        }
                        Err(e) => {
                            // Final fallback: ADB screencap
                            if !serial.is_empty() && !is_usb {
                                if let Ok(png_bytes) = Adb::screencap(serial).await {
                                    if let Ok(b64) = DeviceService::encode_screenshot(&png_bytes, quality, scale) {
                                        let elapsed = t0.elapsed().as_secs_f64();
                                        state_clone.metrics.record_screenshot_latency(elapsed);
                                        return (udid, Ok(b64));
                                    }
                                }
                            }
                            (udid, Err(e))
                        }
                    }
                }
                Err(_e) => {
                    // Device lookup failed
                    (udid, Err("Device not found".to_string()))
                }
            }
        };
        tasks.push(task);
    }

    // Execute all tasks concurrently
    let results: Vec<(String, Result<String, String>)> = futures::future::join_all(tasks).await;

    // Build response
    let mut response_results: StdHashMap<String, serde_json::Value> = StdHashMap::new();
    let mut success_count = 0;
    let mut failure_count = 0;

    for (udid, result) in results {
        match result {
            Ok(data) => {
                response_results.insert(udid.clone(), json!({
                    "status": "success",
                    "data": data,
                    "type": "jpeg"
                }));
                success_count += 1;
            }
            Err(e) => {
                // More precise error classification matching AC spec
                let error_code = if e.contains("not found") {
                    "ERR_DEVICE_NOT_FOUND"
                } else if e.contains("disconnected") || e.contains("unreachable") {
                    "ERR_DEVICE_DISCONNECTED"
                } else {
                    "ERR_SCREENSHOT_FAILED"
                };
                response_results.insert(udid.clone(), json!({
                    "status": "error",
                    "error": error_code,
                    "message": e
                }));
                failure_count += 1;
            }
        }
    }

    // Determine overall status and HTTP status code
    let overall_status = if failure_count == 0 {
        "success"
    } else if success_count == 0 {
        "failed"
    } else {
        "partial"
    };

    // Return 200 OK even for complete failures - these are device-side issues, not server errors
    // Client should check the "status" field for actual result
    let http_status = if failure_count == 0 || success_count == 0 {
        actix_web::http::StatusCode::OK
    } else {
        actix_web::http::StatusCode::MULTI_STATUS
    };

    HttpResponse::build(http_status).json(json!({
        "status": overall_status,
        "total": devices.len(),
        "success": success_count,
        "failed": failure_count,
        "results": response_results
    }))
}

// ═══════════════ BATCH CONTROL OPERATIONS ═══════════════

#[derive(Deserialize)]
pub struct BatchTapRequest {
    udids: Vec<String>,
    x: i32,
    y: i32,
}

#[derive(Deserialize)]
pub struct BatchSwipeRequest {
    udids: Vec<String>,
    x: i32,
    y: i32,
    x2: i32,
    y2: i32,
    #[serde(default = "default_swipe_duration")]
    duration: i32,
}

fn default_swipe_duration() -> i32 { 200 }

#[derive(Deserialize)]
pub struct BatchInputRequest {
    udids: Vec<String>,
    text: String,
    #[serde(default)]
    clear: bool,
}

/// POST /api/batch/tap - Execute tap on multiple devices in parallel
pub async fn batch_tap(
    state: web::Data<AppState>,
    body: web::Json<BatchTapRequest>,
) -> HttpResponse {
    let udids = &body.udids;

    if udids.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_NO_DEVICES_SELECTED",
            "message": "At least one device must be selected"
        }));
    }

    // Batch size limit (per NFR requirements)
    const MAX_BATCH_SIZE: usize = 20;
    if udids.len() > MAX_BATCH_SIZE {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_BATCH_TOO_LARGE",
            "message": format!("Too many devices. Maximum is {}", MAX_BATCH_SIZE)
        }));
    }

    let x = body.x;
    let y = body.y;

    // Execute taps concurrently
    let mut tasks = Vec::new();
    for udid in udids.clone() {
        let state_clone = state.clone();
        let task = async move {
            let udid = udid.clone();
            match get_device_client(&state_clone, &udid).await {
                Ok((device, client)) => {
                    // Mock device
                    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
                        return (udid, Ok(()));
                    }

                    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("").to_string();

                    // Try ATX first
                    match client.click(x, y).await {
                        Ok(_) => (udid, Ok(())),
                        Err(e) => {
                            tracing::warn!("[BATCH_TAP] ATX failed {}: {}, trying ADB fallback", udid, e);
                            if !serial.is_empty() {
                                match Adb::input_tap(&serial, x, y).await {
                                    Ok(_) => (udid, Ok(())),
                                    Err(e2) => (udid, Err(("ERR_TAP_FAILED", e2))),
                                }
                            } else {
                                (udid, Err(("ERR_TAP_FAILED", e)))
                            }
                        }
                    }
                }
                Err(resp) => {
                    // Extract error from HttpResponse
                    let error_code = if resp.status() == actix_web::http::StatusCode::NOT_FOUND {
                        "ERR_DEVICE_NOT_FOUND"
                    } else if resp.status() == actix_web::http::StatusCode::SERVICE_UNAVAILABLE {
                        "ERR_DEVICE_DISCONNECTED"
                    } else {
                        "ERR_DEVICE_ERROR"
                    };
                    (udid, Err((error_code, format!("Device error: {}", resp.status()))))
                }
            }
        };
        tasks.push(task);
    }

    let results = futures::future::join_all(tasks).await;

    // Collect results
    let mut batch_results = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    for (udid, result) in results {
        match result {
            Ok(_) => {
                batch_results.push(json!({"udid": udid, "status": "success"}));
                successful += 1;
            }
            Err((code, msg)) => {
                batch_results.push(json!({
                    "udid": udid,
                    "status": "error",
                    "error": code,
                    "message": msg
                }));
                failed += 1;
            }
        }
    }

    let overall_status = if failed == 0 { "success" } else if successful == 0 { "failed" } else { "partial" };
    // Return 200 OK even for complete failures - these are device-side issues, not server errors
    // Client should check the "status" field for actual result
    let http_status = if failed == 0 || successful == 0 {
        actix_web::http::StatusCode::OK
    } else {
        actix_web::http::StatusCode::MULTI_STATUS
    };

    HttpResponse::build(http_status).json(json!({
        "status": overall_status,
        "total": udids.len(),
        "successful": successful,
        "failed": failed,
        "results": batch_results
    }))
}

/// POST /api/batch/swipe - Execute swipe on multiple devices in parallel
pub async fn batch_swipe(
    state: web::Data<AppState>,
    body: web::Json<BatchSwipeRequest>,
) -> HttpResponse {
    let udids = &body.udids;

    if udids.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_NO_DEVICES_SELECTED",
            "message": "At least one device must be selected"
        }));
    }

    const MAX_BATCH_SIZE: usize = 20;
    if udids.len() > MAX_BATCH_SIZE {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_BATCH_TOO_LARGE",
            "message": format!("Too many devices. Maximum is {}", MAX_BATCH_SIZE)
        }));
    }

    let x = body.x;
    let y = body.y;
    let x2 = body.x2;
    let y2 = body.y2;
    let duration_ms = body.duration.max(50).min(2000);
    let duration = duration_ms as f64 / 1000.0;

    let mut tasks = Vec::new();
    for udid in udids.clone() {
        let state_clone = state.clone();
        let task = async move {
            let udid = udid.clone();
            match get_device_client(&state_clone, &udid).await {
                Ok((device, client)) => {
                    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
                        return (udid, Ok(()));
                    }

                    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("").to_string();

                    match client.swipe(x, y, x2, y2, duration.max(0.05).min(2.0)).await {
                        Ok(_) => (udid, Ok(())),
                        Err(e) => {
                            tracing::warn!("[BATCH_SWIPE] ATX failed {}: {}, trying ADB fallback", udid, e);
                            if !serial.is_empty() {
                                match Adb::input_swipe(&serial, x, y, x2, y2, duration_ms).await {
                                    Ok(_) => (udid, Ok(())),
                                    Err(e2) => (udid, Err(("ERR_SWIPE_FAILED", e2))),
                                }
                            } else {
                                (udid, Err(("ERR_SWIPE_FAILED", e)))
                            }
                        }
                    }
                }
                Err(resp) => {
                    // Extract error from HttpResponse - distinguish between not found and disconnected
                    let error_code = if resp.status() == actix_web::http::StatusCode::NOT_FOUND {
                        "ERR_DEVICE_NOT_FOUND"
                    } else if resp.status() == actix_web::http::StatusCode::SERVICE_UNAVAILABLE {
                        "ERR_DEVICE_DISCONNECTED"
                    } else {
                        "ERR_DEVICE_ERROR"
                    };
                    (udid, Err((error_code, format!("Device error: {}", resp.status()))))
                }
            }
        };
        tasks.push(task);
    }

    let results = futures::future::join_all(tasks).await;

    let mut batch_results = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    for (udid, result) in results {
        match result {
            Ok(_) => {
                batch_results.push(json!({"udid": udid, "status": "success"}));
                successful += 1;
            }
            Err((code, msg)) => {
                batch_results.push(json!({
                    "udid": udid,
                    "status": "error",
                    "error": code,
                    "message": msg
                }));
                failed += 1;
            }
        }
    }

    let overall_status = if failed == 0 { "success" } else if successful == 0 { "failed" } else { "partial" };
    // Return 200 OK even for complete failures - these are device-side issues, not server errors
    // Client should check the "status" field for actual result
    let http_status = if failed == 0 || successful == 0 {
        actix_web::http::StatusCode::OK
    } else {
        actix_web::http::StatusCode::MULTI_STATUS
    };

    HttpResponse::build(http_status).json(json!({
        "status": overall_status,
        "total": udids.len(),
        "successful": successful,
        "failed": failed,
        "results": batch_results
    }))
}

/// POST /api/batch/input - Execute text input on multiple devices in parallel
pub async fn batch_input(
    state: web::Data<AppState>,
    body: web::Json<BatchInputRequest>,
) -> HttpResponse {
    let udids = &body.udids;

    if udids.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_NO_DEVICES_SELECTED",
            "message": "At least one device must be selected"
        }));
    }

    if body.text.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "Text cannot be empty"
        }));
    }

    const MAX_BATCH_SIZE: usize = 20;
    if udids.len() > MAX_BATCH_SIZE {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_BATCH_TOO_LARGE",
            "message": format!("Too many devices. Maximum is {}", MAX_BATCH_SIZE)
        }));
    }

    let text = body.text.clone();
    let clear = body.clear;

    let mut tasks = Vec::new();
    for udid in udids.clone() {
        let state_clone = state.clone();
        let text_clone = text.clone();
        let task = async move {
            let udid = udid.clone();
            match get_device_client(&state_clone, &udid).await {
                Ok((device, client)) => {
                    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
                        return (udid, Ok(()));
                    }

                    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("").to_string();

                    // Clear field first if requested (best-effort, log failures)
                    if clear {
                        if let Err(e) = client.shell_cmd("input keyevent --longpress 29").await {
                            tracing::debug!("[BATCH_INPUT] Clear select-all failed for {}: {}", udid, e);
                        }
                        if let Err(e) = client.shell_cmd("input keyevent 67").await {
                            tracing::debug!("[BATCH_INPUT] Clear backspace failed for {}: {}", udid, e);
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    }

                    match client.input_text(&text_clone).await {
                        Ok(_) => (udid, Ok(())),
                        Err(e) => {
                            tracing::warn!("[BATCH_INPUT] ATX failed {}: {}, trying ADB fallback", udid, e);
                            if !serial.is_empty() {
                                match Adb::input_text(&serial, &text_clone).await {
                                    Ok(_) => (udid, Ok(())),
                                    Err(e2) => (udid, Err(("ERR_INPUT_FAILED", e2))),
                                }
                            } else {
                                (udid, Err(("ERR_INPUT_FAILED", e)))
                            }
                        }
                    }
                }
                Err(resp) => {
                    // Extract error from HttpResponse - distinguish between not found and disconnected
                    let error_code = if resp.status() == actix_web::http::StatusCode::NOT_FOUND {
                        "ERR_DEVICE_NOT_FOUND"
                    } else if resp.status() == actix_web::http::StatusCode::SERVICE_UNAVAILABLE {
                        "ERR_DEVICE_DISCONNECTED"
                    } else {
                        "ERR_DEVICE_ERROR"
                    };
                    (udid, Err((error_code, format!("Device error: {}", resp.status()))))
                }
            }
        };
        tasks.push(task);
    }

    let results = futures::future::join_all(tasks).await;

    let mut batch_results = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    for (udid, result) in results {
        match result {
            Ok(_) => {
                batch_results.push(json!({"udid": udid, "status": "success"}));
                successful += 1;
            }
            Err((code, msg)) => {
                batch_results.push(json!({
                    "udid": udid,
                    "status": "error",
                    "error": code,
                    "message": msg
                }));
                failed += 1;
            }
        }
    }

    let overall_status = if failed == 0 { "success" } else if successful == 0 { "failed" } else { "partial" };
    // Return 200 OK even for complete failures - these are device-side issues, not server errors
    // Client should check the "status" field for actual result
    let http_status = if failed == 0 || successful == 0 {
        actix_web::http::StatusCode::OK
    } else {
        actix_web::http::StatusCode::MULTI_STATUS
    };

    HttpResponse::build(http_status).json(json!({
        "status": overall_status,
        "total": udids.len(),
        "successful": successful,
        "failed": failed,
        "results": batch_results
    }))
}

// ═══════════════ TOUCH / INPUT / KEYEVENT ═══════════════

/// Helper function to resolve swipe pattern to coordinates
/// Returns (x, y, x2, y2, duration_ms) or None if no pattern
fn resolve_swipe_pattern(pattern: &str, width: i32, height: i32) -> Option<(i32, i32, i32, i32, i32)> {
    match pattern {
        "scroll_up" => {
            // Bottom-center to top-center
            Some((width / 2, (height as f64 * 0.8) as i32, width / 2, (height as f64 * 0.2) as i32, 200))
        }
        "scroll_down" => {
            // Top-center to bottom-center
            Some((width / 2, (height as f64 * 0.2) as i32, width / 2, (height as f64 * 0.8) as i32, 200))
        }
        "back" => {
            // Left edge swipe right (Android back gesture)
            Some((0, height / 2, (width as f64 * 0.3) as i32, height / 2, 250))
        }
        "forward" => {
            // Right edge swipe left (Android forward gesture)
            // Start just inside right edge to avoid bounds check
            Some((width - 1, height / 2, (width as f64 * 0.7) as i32, height / 2, 250))
        }
        _ => None,
    }
}

/// POST /inspector/{udid}/touch → fire-and-forget
pub async fn inspector_touch(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Value>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "Missing device UDID"
        }));
    }

    let action = body.get("action").and_then(|v| v.as_str()).unwrap_or("click");
    let pattern = body.get("pattern").and_then(|v| v.as_str());

    // Get device info first (needed for display dimensions and pattern resolution)
    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Get display dimensions
    let display = device.get("display").cloned().unwrap_or(json!({"width":1080,"height":1920}));
    let display_width = display.get("width").and_then(|v| v.as_i64()).unwrap_or(1080) as i32;
    let display_height = display.get("height").and_then(|v| v.as_i64()).unwrap_or(1920) as i32;

    // Resolve coordinates: pattern-based or explicit
    let (x, y, x2, y2, duration_ms): (i32, i32, i32, i32, i32) = if action == "swipe" && pattern.is_some() {
        let pat = pattern.unwrap();
        match resolve_swipe_pattern(pat, display_width, display_height) {
            Some(coords) => coords,
            None => return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": format!("Unknown swipe pattern: {}. Available: scroll_up, scroll_down, back, forward", pat)
            })),
        }
    } else {
        // Explicit coordinates
        let x = match body.get("x").and_then(|v| v.as_f64()) {
            Some(v) => v as i32,
            None => return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": "Missing x coordinate"
            })),
        };
        let y = match body.get("y").and_then(|v| v.as_f64()) {
            Some(v) => v as i32,
            None => return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": "Missing y coordinate"
            })),
        };
        let x2 = body.get("x2").and_then(|v| v.as_f64()).unwrap_or(x as f64) as i32;
        let y2 = body.get("y2").and_then(|v| v.as_f64()).unwrap_or(y as f64) as i32;
        let duration_ms = body.get("duration").and_then(|v| v.as_f64()).unwrap_or(200.0) as i32;

        (x, y, x2, y2, duration_ms)
    };

    // Validate coordinates against display bounds
    let max_x = (display_width - 1).max(0);
    let max_y = (display_height - 1).max(0);

    if x < 0 || x > max_x {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": format!("X coordinate {} out of bounds (0-{})", x, max_x)
        }));
    }
    if y < 0 || y > max_y {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": format!("Y coordinate {} out of bounds (0-{})", y, max_y)
        }));
    }

    if action == "swipe" {
        if x2 < 0 || x2 > max_x {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": format!("X2 coordinate {} out of bounds (0-{})", x2, max_x)
            }));
        }
        if y2 < 0 || y2 > max_y {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": format!("Y2 coordinate {} out of bounds (0-{})", y2, max_y)
            }));
        }
        if duration_ms <= 0 {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "error": "ERR_INVALID_REQUEST",
                "message": "Duration must be positive"
            }));
        }
    }

    // Mock device
    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HttpResponse::Ok().json(json!({"status": "ok"}));
    }

    // Check if recording is active and not paused for this device
    if state.recording_service.should_capture(&udid).await {
        let action_type = if action == "swipe" {
            ActionType::Swipe
        } else {
            ActionType::Tap
        };
        let request = RecordActionRequest {
            action_type,
            x: Some(x),
            y: Some(y),
            x2: if action == "swipe" { Some(x2) } else { None },
            y2: if action == "swipe" { Some(y2) } else { None },
            duration_ms: if action == "swipe" { Some(duration_ms) } else { None },
            text: None,
            key_code: None,
        };
        if let Err(e) = state.recording_service.record_action(&udid, request).await {
            tracing::warn!("[RECORDING] Failed to record action for {}: {}", udid, e);
        }
    }

    // Fire-and-forget
    let action = action.to_string();
    let duration = duration_ms as f64 / 1000.0;
    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("").to_string();

    tokio::spawn(async move {
        if action == "swipe" {
            // Use ADB input swipe directly — ATX JSON-RPC swipe returns success
            // but does nothing on many MIUI/Xiaomi devices
            if !serial.is_empty() {
                if let Err(e) = Adb::input_swipe(&serial, x, y, x2, y2, duration_ms.max(50).min(2000)).await {
                    tracing::warn!("[TOUCH] ADB swipe failed {}: {}, trying ATX fallback", client.udid, e);
                    if let Err(e2) = client.swipe(x, y, x2, y2, duration.max(0.05).min(2.0)).await {
                        tracing::error!("[TOUCH] ATX swipe also failed {}: {}", client.udid, e2);
                    }
                }
            } else {
                if let Err(e) = client.swipe(x, y, x2, y2, duration.max(0.05).min(2.0)).await {
                    tracing::error!("[TOUCH] ATX swipe failed {}: {}", client.udid, e);
                }
            }
        } else {
            if let Err(e) = client.click(x, y).await {
                tracing::warn!("[TOUCH] u2 click failed {}: {}, trying ADB fallback", client.udid, e);
                if !serial.is_empty() {
                    if let Err(e2) = Adb::input_tap(&serial, x, y).await {
                        tracing::error!("[TOUCH] ADB tap also failed {}: {}", client.udid, e2);
                    }
                }
            }
        }
    });

    HttpResponse::Ok().json(json!({"status": "ok"}))
}

/// POST /inspector/{udid}/input → fire-and-forget
pub async fn inspector_input(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Value>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "UDID cannot be empty"
        }));
    }

    let text = body
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let clear = body.get("clear").and_then(|v| v.as_bool()).unwrap_or(false);

    if text.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "Text cannot be empty"
        }));
    }

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HttpResponse::Ok().json(json!({"status": "ok"}));
    }

    // Check if recording is active and not paused for this device
    if state.recording_service.should_capture(&udid).await {
        let request = RecordActionRequest {
            action_type: ActionType::Input,
            x: None,
            y: None,
            x2: None,
            y2: None,
            duration_ms: None,
            text: Some(text.clone()),
            key_code: None,
        };
        if let Err(e) = state.recording_service.record_action(&udid, request).await {
            tracing::warn!("[RECORDING] Failed to record input action for {}: {}", udid, e);
        }
    }

    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("").to_string();
    tokio::spawn(async move {
        // Clear field first if requested: select all (Ctrl+A) then delete
        if clear {
            let mut clear_success = false;

            // Try ATX shell first: Ctrl+A (select all) then Delete
            // Android Ctrl+A is: keydown CtrlLeft, keydown A, keyup A, keyup CtrlLeft
            let atx_clear_cmd = "input keyevent --longpress 29"; // Longpress A for select-all behavior
            if let Err(e) = client.shell_cmd(atx_clear_cmd).await {
                tracing::warn!("[INPUT] ATX select-all failed for {}: {}", client.udid, e);
            } else {
                // Then delete
                if let Err(e) = client.shell_cmd("input keyevent 67").await { // KEYCODE_DEL
                    tracing::warn!("[INPUT] ATX delete failed for {}: {}", client.udid, e);
                } else {
                    clear_success = true;
                }
            }

            // Fallback to ADB if ATX failed and serial is available
            if !clear_success && !serial.is_empty() {
                tracing::info!("[INPUT] Trying ADB clear fallback for {}", client.udid);
                // Use text replacement approach: set text to empty via clipboard
                // This is more reliable than key events for clearing
                if let Err(e) = Adb::shell(&serial, "input keyevent KEYCODE_MOVE_END && input keyevent --longpress KEYCODE_DEL").await {
                    tracing::warn!("[INPUT] ADB clear fallback failed {}: {}", client.udid, e);
                }
            }

            // Small delay to ensure clear completes
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        if let Err(e) = client.input_text(&text).await {
            tracing::warn!("[INPUT] u2 failed {}: {}, trying ADB fallback", client.udid, e);
            if !serial.is_empty() {
                if let Err(e2) = Adb::input_text(&serial, &text).await {
                    tracing::error!("[INPUT] ADB fallback also failed {}: {}", client.udid, e2);
                }
            }
        }
    });

    HttpResponse::Ok().json(json!({"status": "ok"}))
}

/// POST /inspector/{udid}/keyevent → fire-and-forget
pub async fn inspector_keyevent(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Value>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "UDID cannot be empty"
        }));
    }

    let key = body
        .get("key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Define supported keys for validation
    const SUPPORTED_KEYS: &[&str] = &[
        "home", "back", "menu", "power", "wakeup",
        "volume_up", "volume_down",
        "enter", "tab", "del", "forward_del",
        "dpad_up", "dpad_down", "dpad_left", "dpad_right"
    ];

    // Android keycode mapping (normalize case)
    let android_key = match key.to_lowercase().as_str() {
        "enter" => "enter",
        "backspace" | "del" => "del",
        "delete" | "forward_del" => "forward_del",
        "home" => "home",
        "back" => "back",
        "tab" => "tab",
        "escape" => "back",
        "arrowup" | "dpad_up" => "dpad_up",
        "arrowdown" | "dpad_down" => "dpad_down",
        "arrowleft" | "dpad_left" => "dpad_left",
        "arrowright" | "dpad_right" => "dpad_right",
        "menu" => "menu",
        "power" => "power",
        "wakeup" => "wakeup",
        "volume_up" => "volume_up",
        "volume_down" => "volume_down",
        other => other,
    }
    .to_string();

    // Validate key is supported
    if !SUPPORTED_KEYS.contains(&android_key.as_str()) {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": format!("Invalid key action: {}. Supported keys: {}",
                key, SUPPORTED_KEYS.join(", "))
        }));
    }

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HttpResponse::Ok().json(json!({"status": "ok"}));
    }

    // Check if recording is active and not paused for this device
    if state.recording_service.should_capture(&udid).await {
        // Map key name to Android keycode for recording
        let key_code: i32 = match android_key.as_str() {
            "home" => 3,
            "back" => 4,
            "menu" => 82,
            "power" => 26,
            "wakeup" => 224,
            "volume_up" => 24,
            "volume_down" => 25,
            "enter" => 66,
            "tab" => 61,
            "del" => 67,
            "forward_del" => 112,
            "dpad_up" => 19,
            "dpad_down" => 20,
            "dpad_left" => 21,
            "dpad_right" => 22,
            _ => 0,
        };
        let request = RecordActionRequest {
            action_type: ActionType::KeyEvent,
            x: None,
            y: None,
            x2: None,
            y2: None,
            duration_ms: None,
            text: None,
            key_code: Some(key_code),
        };
        if let Err(e) = state.recording_service.record_action(&udid, request).await {
            tracing::warn!("[RECORDING] Failed to record keyevent action for {}: {}", udid, e);
        }
    }

    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("").to_string();
    tokio::spawn(async move {
        if let Err(e) = client.press_key(&android_key).await {
            tracing::warn!("[KEYEVENT] u2 failed {}: {}, trying ADB fallback", client.udid, e);
            if !serial.is_empty() {
                if let Err(e2) = Adb::input_keyevent(&serial, &android_key).await {
                    tracing::error!("[KEYEVENT] ADB fallback also failed {}: {}", client.udid, e2);
                }
            }
        }
    });

    HttpResponse::Ok().json(json!({"status": "ok"}))
}

// ═══════════════ HIERARCHY ═══════════════

/// GET /inspector/{udid}/hierarchy → JSON
pub async fn inspector_hierarchy(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "UDID cannot be empty"
        }));
    }

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Mock device handling
    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HttpResponse::Ok().json(json!({
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
        Ok(hierarchy) => HttpResponse::Ok().json(hierarchy),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_HIERARCHY_FAILED",
            "message": e
        })),
    }
}

// ═══════════════ FILE UPLOAD ═══════════════

/// POST /inspector/{udid}/upload → upload file to device
pub async fn inspector_upload(
    state: web::Data<AppState>,
    path: web::Path<String>,
    mut payload: Multipart,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let (_device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    while let Some(Ok(mut field)) = payload.next().await {
        let filename = field
            .content_disposition()
            .and_then(|cd| cd.get_filename().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        // Read file data
        let mut data = Vec::new();
        while let Some(Ok(chunk)) = field.next().await {
            data.extend_from_slice(&chunk);
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
            return HttpResponse::InternalServerError().json(json!({"status":"error","message":e}));
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

        return HttpResponse::Ok().json(json!({
            "status": "ok",
            "message": format!("File uploaded to: {}", device_path),
            "path": device_path,
        }));
    }

    HttpResponse::BadRequest().json(json!({"status":"error","message":"No file uploaded"}))
}

// ═══════════════ ROTATION ═══════════════

/// POST /inspector/{udid}/rotation → fix device rotation via ATX agent
pub async fn inspector_rotation(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let (_device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Forward rotation fix to device ATX agent
    let url = format!("{}/info/rotation", client.base_url());
    match client.http_client().post(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            HttpResponse::build(actix_web::http::StatusCode::from_u16(status.as_u16()).unwrap_or(actix_web::http::StatusCode::OK))
                .content_type("application/json")
                .body(body)
        }
        Err(e) => {
            tracing::warn!("[ROTATION] Failed for {}: {}", udid, e);
            HttpResponse::InternalServerError().json(json!({"error": format!("rotation fix failed: {}", e)}))
        }
    }
}

// ═══════════════ INSPECTOR SHELL (HTTP) ═══════════════

/// POST /inspector/{udid}/shell → execute shell command via HTTP proxy
pub async fn inspector_shell(
    state: web::Data<AppState>,
    path: web::Path<String>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let command = form.get("command").cloned().unwrap_or_default();
    if command.is_empty() {
        return HttpResponse::BadRequest().json(json!({"error": "command is required"}));
    }

    // Safety checks
    if is_dangerous_command(&command) {
        return HttpResponse::Forbidden().json(json!({"error": "Command blocked for safety"}));
    }
    if has_dangerous_metacharacters(&command) {
        return HttpResponse::Forbidden().json(json!({"error": "Command contains blocked shell metacharacters"}));
    }

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("");

    match client.shell_cmd(&command).await {
        Ok(output) => HttpResponse::Ok().json(json!({"output": output})),
        Err(_) if !serial.is_empty() => {
            // ADB fallback
            match Adb::shell(serial, &command).await {
                Ok(output) => HttpResponse::Ok().json(json!({"output": output})),
                Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("{}", e)})),
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("{}", e)})),
    }
}

/// GET /inspector/{udid}/shell → execute shell command via HTTP proxy (GET variant for legacy support)
pub async fn inspector_shell_get(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let command = query.get("command").cloned().unwrap_or_default();
    if command.is_empty() {
        return HttpResponse::BadRequest().json(json!({"error": "command is required"}));
    }

    if is_dangerous_command(&command) {
        return HttpResponse::Forbidden().json(json!({"error": "Command blocked for safety"}));
    }
    if has_dangerous_metacharacters(&command) {
        return HttpResponse::Forbidden().json(json!({"error": "Command contains blocked shell metacharacters"}));
    }

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("");

    match client.shell_cmd(&command).await {
        Ok(output) => HttpResponse::Ok().json(json!({"output": output})),
        Err(_) if !serial.is_empty() => {
            match Adb::shell(serial, &command).await {
                Ok(output) => HttpResponse::Ok().json(json!({"output": output})),
                Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("{}", e)})),
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("{}", e)})),
    }
}

/// POST /upload → upload file with chmod + apk install
pub async fn store_file_handler(
    state: web::Data<AppState>,
    req: HttpRequest,
    mut payload: Multipart,
) -> HttpResponse {
    // Extract device UDID from custom header (Story 12-5: semantic correctness)
    let udid = req
        .headers()
        .get("X-Device-UDID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let phone_service = state.phone_service.clone();
    let device = match phone_service.query_info_by_udid(udid).await {
        Ok(Some(d)) => d,
        _ => return HttpResponse::NotFound().body("Device not found"),
    };

    let ip = device.get("ip").and_then(|v| v.as_str()).unwrap_or("");
    let port = device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);

    let mut path = "/data/local/tmp/".to_string();
    let mut power = "755".to_string();
    let mut names = Vec::new();

    while let Some(Ok(mut field)) = payload.next().await {
        let name = field
            .content_disposition()
            .and_then(|cd| cd.get_name().map(|s| s.to_string()))
            .unwrap_or_default();

        if name == "path" {
            let mut data = Vec::new();
            while let Some(Ok(chunk)) = field.next().await {
                data.extend_from_slice(&chunk);
            }
            let p = String::from_utf8_lossy(&data).to_string();
            if !p.is_empty() {
                path = p;
            }
        } else if name == "power" {
            let mut data = Vec::new();
            while let Some(Ok(chunk)) = field.next().await {
                data.extend_from_slice(&chunk);
            }
            power = String::from_utf8_lossy(&data).to_string();
        } else if name == "file" {
            let filename = field
                .content_disposition()
                .and_then(|cd| cd.get_filename().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown".to_string());
            names.push(filename.clone());

            let mut data = Vec::new();
            while let Some(Ok(chunk)) = field.next().await {
                data.extend_from_slice(&chunk);
            }

            // Upload to device via atx-agent
            let upload_path = path.replace('_', "/");
            let upload_url = format!("http://{}:{}/upload{}", ip, port, upload_path);

            let client = reqwest::Client::new();
            let part = reqwest::multipart::Part::bytes(data)
                .file_name(filename.clone())
                .mime_str("application/octet-stream")
                .unwrap();
            let form = reqwest::multipart::Form::new().part("file", part);

            let _ = client.post(&upload_url).multipart(form).send().await;

            // chmod
            let atx = AtxClient::new(ip, port, udid);
            let _ = atx
                .shell_cmd(&format!("chmod {} {}{}", power, path, filename))
                .await;

            // APK install
            if filename.ends_with(".apk") {
                let _ = atx
                    .shell_cmd(&format!("pm install {}{}", path, filename))
                    .await;
            }
        }
    }

    HttpResponse::Ok().body(format!("upload {} successfully stored", names.join(",")))
}

/// POST /upload_group/{path} → broadcast upload to all online devices
pub async fn upload_group(
    state: web::Data<AppState>,
    path: web::Path<String>,
    mut payload: Multipart,
) -> HttpResponse {
    let upload_path = path.into_inner();

    let phone_service = state.phone_service.clone();
    let file_service = crate::services::file_service::FileService::new(state.db.clone());

    // Read the file from multipart
    let mut filename = String::new();
    let mut file_data = Vec::new();

    while let Some(Ok(mut field)) = payload.next().await {
        filename = field
            .content_disposition()
            .and_then(|cd| cd.get_filename().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        while let Some(Ok(chunk)) = field.next().await {
            file_data.extend_from_slice(&chunk);
        }
        break; // Only process first file
    }

    let devices = phone_service
        .query_device_list_by_present()
        .await
        .unwrap_or_default();

    let mut exceptions = Vec::new();
    let client = reqwest::Client::new();

    for dev in &devices {
        let ip = dev.get("ip").and_then(|v| v.as_str()).unwrap_or("");
        let port = dev.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
        let dev_udid = dev.get("udid").and_then(|v| v.as_str()).unwrap_or("");

        let url = format!(
            "http://{}:{}/upload/{}/",
            ip,
            port,
            upload_path.replace('_', "/")
        );

        let part = reqwest::multipart::Part::bytes(file_data.clone())
            .file_name(filename.clone())
            .mime_str("application/octet-stream")
            .unwrap();
        let form = reqwest::multipart::Form::new().part("file", part);

        match client
            .post(&url)
            .multipart(form)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(_) => {
                // APK install
                if filename.ends_with(".apk") {
                    let atx = AtxClient::new(ip, port, dev_udid);
                    let _ = atx
                        .shell_cmd(&format!(
                            "pm install /{}/{}",
                            upload_path.replace('_', "/"),
                            filename
                        ))
                        .await;
                }
            }
            Err(e) => {
                exceptions.push(format!("Exception: {} ip: {}", e, ip));
            }
        }
    }

    // Save file record
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let file_record = json!({
        "group": 0,
        "filename": filename,
        "filesize": 0,
        "upload_time": now,
        "who": "admin",
    });
    let _ = file_service.save_install_file(&file_record).await;

    let mut result = json!({});
    if !exceptions.is_empty() {
        result["exception"] = json!("true");
        result["exception_data"] = json!(exceptions);
    }

    HttpResponse::Ok().json(result)
}

// ═══════════════ HEARTBEAT ═══════════════

/// POST /heartbeat → device heartbeat keep-alive
pub async fn heartbeat(
    state: web::Data<AppState>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let identifier = match form.get("identifier") {
        Some(id) => id.clone(),
        None => return HttpResponse::BadRequest().body("Missing identifier"),
    };

    let phone_service = state.phone_service.clone();
    let sessions = state.heartbeat_sessions.clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    if let Some(mut session) = sessions.get_mut(&identifier) {
        // Existing session — reset timer
        session.timer = now + 20.0;
    } else {
        // New session
        let session = crate::state::HeartbeatSession {
            identifier: identifier.clone(),
            remote_host: "unknown".to_string(),
            timer: now + 20.0,
        };
        sessions.insert(identifier.clone(), session);

        // on_connected
        let _ = phone_service
            .on_connected(&identifier, "unknown")
            .await;

        // Start timer task
        let ps = phone_service.clone();
        let ident = identifier.clone();
        let sess = sessions.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64();

                let expired = sess
                    .get(&ident)
                    .map(|s| s.timer < now)
                    .unwrap_or(true);

                if expired {
                    sess.remove(&ident);
                    let _ = ps.offline_connected(&ident).await;
                    return;
                }
            }
        });
    }

    HttpResponse::Ok().body("hello kitty")
}

// ═══════════════ SHELL ═══════════════

/// POST /shell → execute shell command on device
pub async fn shell(
    state: web::Data<AppState>,
    req: HttpRequest,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    // Extract device UDID from custom header (Story 12-5: semantic correctness)
    let udid = req
        .headers()
        .get("X-Device-UDID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let command = form.get("command").cloned().unwrap_or_default();

    let phone_service = state.phone_service.clone();
    let device = match phone_service.query_info_by_udid(udid).await {
        Ok(Some(d)) => d,
        _ => return HttpResponse::NotFound().body("Device not found"),
    };

    let ip = device.get("ip").and_then(|v| v.as_str()).unwrap_or("");
    let port = device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
    let client = AtxClient::new(ip, port, udid);

    let _ = client.shell_cmd(&command).await;

    HttpResponse::Ok().body(format!("{} sized of 0 successfully stored", udid))
}

// ═══════════════ EXECUTE SHELL (NEW API) ═══════════════

/// Blocked command patterns for safety
const BLOCKED_COMMAND_PATTERNS: &[&str] = &[
    // Device disruption
    "reboot", "shutdown", "restart", "init 6", "init 0",
    "svc power reboot", "svc power shutdown",
    // Data destruction
    "rm -rf", "rm -r -f", "rm -fr",
    "factory-reset", "recovery",
    "format data", "format cache",
    // System modification
    "dd if=", "dd of=",
    "mount ", "umount ",
    "chmod -r 777", "chmod 777 /",
    // Process control
    "killall", "kill -9",
    "stop adbd", "stop zygote",
    // Package management (could break device)
    "pm uninstall", "pm clear",
];

/// Check if a command is dangerous/blocked
fn is_dangerous_command(cmd: &str) -> bool {
    let cmd_lower = cmd.to_lowercase();
    BLOCKED_COMMAND_PATTERNS.iter().any(|p| cmd_lower.contains(p))
}

/// Check for shell metacharacters that could enable command injection
fn has_dangerous_metacharacters(cmd: &str) -> bool {
    // Check for command chaining/injection patterns
    let dangerous_patterns = ["; ", " && ", " || ", "| ", "$((", "`", "$(", "> /", ">> /"];
    dangerous_patterns.iter().any(|p| cmd.contains(p))
}

/// POST /api/devices/{udid}/shell → execute shell command with safety checks
pub async fn execute_shell(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Value>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "UDID cannot be empty"
        }));
    }

    let command = body
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if command.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "Command cannot be empty"
        }));
    }

    // Check for dangerous commands
    if is_dangerous_command(command) {
        tracing::warn!("[SHELL] Blocked dangerous command attempted: {}", command);
        return HttpResponse::Forbidden().json(json!({
            "status": "error",
            "error": "ERR_DANGEROUS_COMMAND",
            "message": format!(
                "Command is blocked for safety. Blocked patterns include: reboot, rm -rf, factory-reset, dd, mount"
            )
        }));
    }

    // Warn about metacharacters but allow (log for audit)
    if has_dangerous_metacharacters(command) {
        tracing::warn!("[SHELL] Command contains shell metacharacters (allowed but logged): {}", command);
    }

    // Parse timeout (default 30s, max 60s, min 1s)
    let timeout_ms = body
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(30000)
        .max(1000)
        .min(60000);

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Mock device handling
    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HttpResponse::Ok().json(json!({
            "status": "success",
            "stdout": "mock output",
            "stderr": "",
            "exit_code": 0,
            "duration_ms": 10
        }));
    }

    let start = std::time::Instant::now();

    // Execute with timeout
    let timeout_duration = std::time::Duration::from_millis(timeout_ms);
    let command_owned = command.to_string();

    let result = tokio::time::timeout(
        timeout_duration,
        async {
            client.shell_cmd(&command_owned).await
        }
    ).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(output)) => {
            HttpResponse::Ok().json(json!({
                "status": "success",
                "stdout": output,
                "stderr": "",
                "exit_code": 0,
                "duration_ms": duration_ms
            }))
        }
        Ok(Err(e)) => {
            // Try ADB fallback for USB devices
            let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("");
            if !serial.is_empty() && crate::device::adb::Adb::is_usb_serial(serial) {
                match crate::device::adb::Adb::shell(serial, &command).await {
                    Ok(output) => {
                        HttpResponse::Ok().json(json!({
                            "status": "success",
                            "stdout": output,
                            "stderr": "",
                            "exit_code": 0,
                            "duration_ms": duration_ms
                        }))
                    }
                    Err(adb_err) => {
                        HttpResponse::InternalServerError().json(json!({
                            "status": "error",
                            "error": "ERR_COMMAND_FAILED",
                            "message": format!("Command failed (ATX: {}, ADB: {})", e, adb_err)
                        }))
                    }
                }
            } else {
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "error": "ERR_COMMAND_FAILED",
                    "message": e
                }))
            }
        }
        Err(_) => {
            // Timeout occurred
            tracing::warn!("[SHELL] Command timed out after {}ms: {}", timeout_ms, command);
            HttpResponse::RequestTimeout().json(json!({
                "status": "error",
                "error": "ERR_COMMAND_TIMEOUT",
                "message": format!("Command exceeded {}ms timeout", timeout_ms),
                "partial_stdout": "",
                "partial_stderr": "",
                "duration_ms": duration_ms
            }))
        }
    }
}

// ═══════════════ WIFI CONNECT ═══════════════

/// POST /api/wifi-connect → connect device via WiFi ADB
pub async fn wifi_connect(
    state: web::Data<AppState>,
    body: web::Json<Value>,
) -> HttpResponse {
    let address = body
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if address.is_empty() {
        return HttpResponse::BadRequest().json(json!({"status":"error","message":"Missing address"}));
    }

    if !address.contains(':') {
        return HttpResponse::BadRequest().json(json!({"status":"error","message":"Invalid format. Use IP:PORT"}));
    }

    let parts: Vec<&str> = address.rsplitn(2, ':').collect();
    let ip = parts.get(1).unwrap_or(&"").to_string();

    tracing::info!("[WiFi Connect] Connecting to {}...", address);

    // Step 1: adb connect
    match Adb::connect(&address).await {
        Ok(output) => {
            let lower = output.to_lowercase();
            if !lower.contains("connected") && !lower.contains("already") {
                return HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "message": format!("ADB connect failed: {}", output.trim()),
                }));
            }
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": e,
            }));
        }
    }

    // Step 2: Wait for device
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Step 3: Get device info from atx-agent
    let atx = AtxClient::from_url(&format!("http://{}:9008", ip), &address);
    let info = match atx.device_info().await {
        Ok(i) => i,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to initialize device: {}", e),
            }));
        }
    };

    let model = info
        .get("productName")
        .or_else(|| info.get("model"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let brand = info
        .get("brand")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let version = info
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let _serial = info
        .get("serial")
        .and_then(|v| v.as_str())
        .unwrap_or(&address.replace(':', "-"));

    let (width, height) = atx.window_size().await.unwrap_or((1080, 1920));
    let udid = format!("{}-{}", address.replace(':', "-"), model.replace(' ', "_"));
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let device_data = json!({
        "udid": udid,
        "serial": address,
        "ip": ip,
        "port": 9008,
        "present": true,
        "ready": true,
        "using": false,
        "is_server": false,
        "model": model,
        "brand": brand,
        "version": version,
        "sdk": info.get("sdk").and_then(|v| v.as_i64()).unwrap_or(30),
        "display": { "width": width, "height": height },
        "update_time": now,
    });

    let phone_service = state.phone_service.clone();
    if let Err(e) = phone_service.update_field(&udid, &device_data).await {
        return HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": format!("Failed to save device: {}", e),
        }));
    }

    HttpResponse::Ok().json(json!({
        "status": "ok",
        "message": "Device connected successfully",
        "udid": udid,
        "model": format!("{} {}", brand, model),
        "ip": ip,
    }))
}

// ═══════════════ MANUAL DEVICE ADDITION ═══════════════

/// Request body for manual device addition
#[derive(Deserialize)]
pub struct ManualDeviceRequest {
    pub ip: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    9008
}

/// POST /api/devices/add → manually add a WiFi device by IP:port
pub async fn add_device(
    state: web::Data<AppState>,
    body: web::Json<ManualDeviceRequest>,
) -> HttpResponse {
    let ip = body.ip.trim();
    let port = body.port;

    // Validate IP address format
    if ip.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "IP address is required"
        }));
    }

    // Validate IPv4 format
    if ip.parse::<std::net::Ipv4Addr>().is_err() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "Invalid IPv4 address format"
        }));
    }

    // Validate port range (already validated by u16, but check for 0)
    if port == 0 {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "Port must be between 1 and 65535"
        }));
    }

    tracing::info!("[AddDevice] Adding device {}:{}", ip, port);

    // Create AtxClient and validate connection
    let atx_url = format!("http://{}:{}", ip, port);
    let atx = AtxClient::from_url(&atx_url, &format!("{}:{}", ip, port));

    // Fetch device info with timeout
    let info = match atx.device_info().await {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!("[AddDevice] Failed to connect to {}:{}", ip, port);
            return HttpResponse::ServiceUnavailable().json(json!({
                "status": "error",
                "message": format!("Device unreachable: {}", e)
            }));
        }
    };

    // Build UDID from device info
    let ip_fallback = ip.replace('.', "-");
    let serial = info
        .get("serial")
        .and_then(|v| v.as_str())
        .unwrap_or(&ip_fallback);
    let hwaddr = info
        .get("hwaddr")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let model = info
        .get("productName")
        .or_else(|| info.get("model"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let brand = info
        .get("brand")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let version = info
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let sdk = info.get("sdk").and_then(|v| v.as_i64()).unwrap_or(30);
    let display = info.get("display").cloned().unwrap_or(json!({
        "width": 1080,
        "height": 1920
    }));
    let battery = info.get("battery").cloned().unwrap_or(json!({
        "level": 0
    }));

    // Use hwaddr if available, otherwise serial or ip:port
    let identifier = if !hwaddr.is_empty() {
        hwaddr.to_string()
    } else if !serial.is_empty() && serial != ip.replace('.', "-") {
        serial.to_string()
    } else {
        format!("{}:{}", ip, port)
    };

    let udid = format!("{}-{}", identifier.replace(':', "-"), model.replace(' ', "_"));

    let phone_service = state.phone_service.clone();

    // Check for duplicate device
    if let Ok(Some(existing)) = phone_service.query_info_by_udid(&udid).await {
        let is_present = existing
            .get("present")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_present {
            return HttpResponse::Conflict().json(json!({
                "status": "error",
                "message": "Device already connected",
                "udid": udid
            }));
        }
        // Device exists but disconnected - allow reconnection
        tracing::info!("[AddDevice] Reconnecting existing device: {}", udid);
    }

    // Build device data
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let device_data = json!({
        "udid": udid,
        "serial": serial,
        "ip": ip,
        "port": port,
        "present": true,
        "ready": true,
        "using": false,
        "is_server": false,
        "model": model,
        "brand": brand,
        "version": version,
        "sdk": sdk,
        "display": display,
        "battery": battery,
        "hwaddr": hwaddr,
        "agentVersion": info.get("agentVersion").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "update_time": now,
    });

    // Register device
    if let Err(e) = phone_service.update_field(&udid, &device_data).await {
        return HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": format!("Failed to register device: {}", e)
        }));
    }

    tracing::info!("[AddDevice] Device added successfully: {} ({})", udid, ip);

    HttpResponse::Ok().json(json!({
        "status": "success",
        "message": "Device added successfully",
        "device": {
            "udid": udid,
            "ip": ip,
            "port": port,
            "model": model,
            "brand": brand,
            "version": version,
            "display": display,
            "battery": battery
        }
    }))
}

// ═══════════════ DEVICE DISCONNECT/RECONNECT ═══════════════

/// DELETE /api/devices/{udid} → manually disconnect a device
pub async fn disconnect_device(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    let phone_service = state.phone_service.clone();

    // Check if device exists
    match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(_)) => {
            // Mark device as offline
            match phone_service.offline_connected(&udid).await {
                Ok(()) => {
                    // Clear heartbeat session so next heartbeat creates a new connect event
                    state.heartbeat_sessions.remove(&udid);
                    tracing::info!("[Disconnect] Device {} disconnected manually", udid);
                    HttpResponse::Ok().json(json!({
                        "status": "success",
                        "message": "Device disconnected successfully"
                    }))
                }
                Err(e) => {
                    tracing::error!("[Disconnect] Failed to disconnect {}: {}", udid, e);
                    HttpResponse::InternalServerError().json(json!({
                        "status": "error",
                        "message": format!("Failed to disconnect: {}", e)
                    }))
                }
            }
        }
        Ok(None) => {
            HttpResponse::NotFound().json(json!({
                "status": "error",
                "message": "Device not found"
            }))
        }
        Err(e) => {
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Database error: {}", e)
            }))
        }
    }
}

/// POST /api/devices/{udid}/reconnect → attempt to reconnect a disconnected device
pub async fn reconnect_device(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    let phone_service = state.phone_service.clone();

    // Get device info to find IP address
    let device = match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return HttpResponse::NotFound().json(json!({
                "status": "error",
                "message": "Device not found"
            }))
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Database error: {}", e)
            }))
        }
    };

    // Get IP and port from device info
    let ip = match device.get("ip").and_then(|v| v.as_str()) {
        Some(ip) => ip,
        None => {
            return HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "Device has no IP address stored"
            }))
        }
    };

    let port = device
        .get("port")
        .and_then(|v| v.as_i64())
        .unwrap_or(9008) as u16;

    // Try to connect to ATX agent to verify device is reachable
    let url = format!("http://{}:{}/info", ip, port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .no_proxy()
        .build()
        .unwrap_or_default();

    match client.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                // Device is reachable - update status
                match phone_service.on_connected(&udid, ip).await {
                    Ok(()) => {
                        tracing::info!("[Reconnect] Device {} reconnected successfully", udid);
                        HttpResponse::Ok().json(json!({
                            "status": "success",
                            "message": "Device reconnected successfully"
                        }))
                    }
                    Err(e) => {
                        tracing::error!("[Reconnect] Failed to update device {}: {}", udid, e);
                        HttpResponse::InternalServerError().json(json!({
                            "status": "error",
                            "message": format!("Failed to update device: {}", e)
                        }))
                    }
                }
            } else {
                HttpResponse::ServiceUnavailable().json(json!({
                    "status": "error",
                    "message": format!("Device returned status {}", resp.status())
                }))
            }
        }
        Err(e) => {
            tracing::warn!("[Reconnect] Device {} unreachable: {}", udid, e);
            HttpResponse::ServiceUnavailable().json(json!({
                "status": "error",
                "message": "Device unreachable"
            }))
        }
    }
}

// ═══════════════ TAG MANAGEMENT ═══════════════

/// POST /api/devices/{udid}/tags → add tags to device
pub async fn add_device_tags(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Value>,
) -> HttpResponse {
    let udid = path.into_inner();

    // Extract tags from request body
    let tags: Vec<String> = body
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    if tags.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "status": "error",
            "error": "ERR_INVALID_REQUEST",
            "message": "No valid tags provided"
        }));
    }

    let phone_service = state.phone_service.clone();

    match phone_service.add_tags(&udid, &tags).await {
        Ok(updated_tags) => {
            HttpResponse::Ok().json(json!({
                "status": "ok",
                "tags": updated_tags
            }))
        }
        Err(e) => {
            if e.contains("not found") {
                HttpResponse::NotFound().json(json!({
                    "status": "error",
                    "error": "ERR_DEVICE_NOT_FOUND",
                    "message": e
                }))
            } else {
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "message": e
                }))
            }
        }
    }
}

/// DELETE /api/devices/{udid}/tags/{tag} → remove tag from device
pub async fn remove_device_tag(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (udid, tag) = path.into_inner();

    let phone_service = state.phone_service.clone();

    match phone_service.remove_tag(&udid, &tag).await {
        Ok(updated_tags) => {
            HttpResponse::Ok().json(json!({
                "status": "ok",
                "tags": updated_tags
            }))
        }
        Err(e) => {
            if e.contains("not found") {
                HttpResponse::NotFound().json(json!({
                    "status": "error",
                    "error": "ERR_DEVICE_NOT_FOUND",
                    "message": e
                }))
            } else {
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "message": e
                }))
            }
        }
    }
}

// ═══════════════ Connection History ═══════════════

/// GET /api/devices/{udid}/history → connection history with session durations
pub async fn get_connection_history(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let udid = path.into_inner();
    let limit: i64 = query
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    let phone_service = state.phone_service.clone();

    match phone_service.get_connection_history(&udid, limit).await {
        Ok(history) => HttpResponse::Ok().json(json!({
            "status": "ok",
            "history": history
        })),
        Err(e) => {
            if e.contains("not found") {
                HttpResponse::NotFound().json(json!({
                    "status": "error",
                    "error": "ERR_DEVICE_NOT_FOUND",
                    "message": e
                }))
            } else {
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "message": e
                }))
            }
        }
    }
}

/// GET /api/devices/{udid}/stats → connection statistics and uptime
pub async fn get_connection_stats(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    let phone_service = state.phone_service.clone();

    match phone_service.get_connection_stats(&udid).await {
        Ok(stats) => HttpResponse::Ok().json(json!({
            "status": "ok",
            "stats": stats
        })),
        Err(e) => {
            if e.contains("not found") {
                HttpResponse::NotFound().json(json!({
                    "status": "error",
                    "error": "ERR_DEVICE_NOT_FOUND",
                    "message": e
                }))
            } else {
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "message": e
                }))
            }
        }
    }
}

// ═══════════════ FILES ═══════════════

/// GET /files?sort=&page=1 → paginated file list JSON
pub async fn files(
    state: web::Data<AppState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let sort = query.get("sort").cloned().unwrap_or_default();
    let page: i64 = query
        .get("page")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let file_service = crate::services::file_service::FileService::new(state.db.clone());

    let start = (page - 1) * 5;
    let list = file_service
        .query_install_file("0", start, 5, &sort)
        .await
        .unwrap_or_default();
    let total = file_service.query_all_install_file().await.unwrap_or(0);
    let last_page = (total / 5) + 1;

    let host = &state.host_ip;
    let port = state.config.server.port;
    let (next_page_url, prev_page_url) = if page < last_page {
        (
            format!("http://{}:{}/files?page={}", host, port, page + 1),
            if page > 1 {
                format!("http://{}:{}/files?page={}", host, port, page - 1)
            } else {
                format!("http://{}:{}/files?page={}", host, port, page)
            },
        )
    } else {
        (
            format!("http://{}:{}/files?page={}", host, port, page),
            format!("http://{}:{}/files?page={}", host, port, page - 1),
        )
    };

    HttpResponse::Ok().json(json!({
        "total": total,
        "per_page": 5,
        "current_page": page,
        "last_page": last_page,
        "next_page_url": next_page_url,
        "prev_page_url": prev_page_url,
        "from": start,
        "to": start + 5,
        "data": list,
    }))
}

/// GET /file/delete/{group}/{filename} → delete file and redirect
pub async fn file_delete(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (group, filename) = path.into_inner();

    let file_service = crate::services::file_service::FileService::new(state.db.clone());
    let _ = file_service.delete_install_file(&group, &filename).await;

    HttpResponse::Found()
        .insert_header(("Location", "/installfile"))
        .finish()
}

// ═══════════════ ATX AGENT ═══════════════

/// GET /atxagent?method=&udid= → control atx-agent
pub async fn atxagent(
    state: web::Data<AppState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let method = query.get("method").cloned().unwrap_or_default();
    let udid = query.get("udid").cloned().unwrap_or_default();

    let phone_service = state.phone_service.clone();
    let device = match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(d)) => d,
        _ => return HttpResponse::NotFound().body("Device not found"),
    };

    let ip = device.get("ip").and_then(|v| v.as_str()).unwrap_or("");
    let url = format!("http://{}:8001/api/v1.0/{}", ip, method);

    let client = reqwest::Client::new();
    let host_ip = &state.host_ip;
    let port = state.config.server.port;

    match client
        .post(&url)
        .form(&[("ip", format!("{}:{}", host_ip, port))])
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            HttpResponse::Ok().body(format!("atx-agent[{}] success", method))
        }
        _ => HttpResponse::NotFound().body(format!("atx-agent[{}] failed", method)),
    }
}

// ═══════════════ WEBSOCKET STUBS ═══════════════

/// GET /feeds → legacy WebSocket feed
pub async fn feeds(req: HttpRequest, stream: web::Payload) -> HttpResponse {
    match actix_ws::handle(&req, stream) {
        Ok((resp, mut session, mut msg_stream)) => {
            actix_web::rt::spawn(async move {
                while let Some(Ok(msg)) = msg_stream.next().await {
                    match msg {
                        actix_ws::Message::Text(_) => {
                            let _ = session
                                .text(serde_json::to_string(&json!({"error": false})).unwrap())
                                .await;
                        }
                        actix_ws::Message::Close(_) => break,
                        _ => {}
                    }
                }
            });
            resp
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// GET /devices/{udid}/reserved → WebSocket device reservation (Story 10-2)
pub async fn reserved(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();

    // Validate device exists in database
    match state.db.find_by_udid(&udid).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_DEVICE_NOT_FOUND",
                "message": format!("Device {} not found", udid)
            }));
        }
        Err(e) => {
            tracing::warn!("[RESERVED] Database error looking up device {}: {}", udid, e);
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_OPERATION_FAILED",
                "message": "Database error"
            }));
        }
    }

    // Upgrade to WebSocket
    match actix_ws::handle(&req, stream) {
        Ok((resp, mut session, mut msg_stream)) => {
            // Atomically check and reserve using entry() to prevent TOCTOU race
            let remote_addr = req
                .peer_addr()
                .map(|a| a.to_string())
                .unwrap_or_default();
            match state.reserved_devices.entry(udid.clone()) {
                dashmap::mapref::entry::Entry::Occupied(_) => {
                    let _ = session
                        .text(json!({"error": "Device already reserved"}).to_string())
                        .await;
                    let _ = session.close(None).await;
                    return resp;
                }
                dashmap::mapref::entry::Entry::Vacant(entry) => {
                    entry.insert(remote_addr);
                }
            }
            if let Err(e) = state.db.update(&udid, &json!({"using": true})).await {
                tracing::warn!("[RESERVED] Failed to set using_device=true for {}: {}", udid, e);
            }

            tracing::info!("[RESERVED] Device {} reserved", udid);

            // Spawn message handler with cleanup on disconnect
            let db = state.db.clone();
            let reserved = state.reserved_devices.clone();
            let udid_clone = udid.clone();
            actix_web::rt::spawn(async move {
                while let Some(Ok(msg)) = msg_stream.next().await {
                    match msg {
                        actix_ws::Message::Text(_) => {
                            let _ = session.text("pong").await;
                        }
                        actix_ws::Message::Ping(data) => {
                            let _ = session.pong(&data).await;
                        }
                        actix_ws::Message::Close(_) => break,
                        _ => {}
                    }
                }
                // Cleanup: release reservation on disconnect
                reserved.remove(&udid_clone);
                if let Err(e) = db.update(&udid_clone, &json!({"using": false})).await {
                    tracing::warn!("[RESERVED] Failed to set using_device=false for {}: {}", udid_clone, e);
                }
                tracing::info!("[RESERVED] Device {} released", udid_clone);
            });

            resp
        }
        Err(_) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_WEBSOCKET_UPGRADE_FAILED",
            "message": "WebSocket upgrade failed"
        })),
    }
}

/// GET /devices/{udid}/shell → ADB Shell WebSocket (server-side proxy)
pub async fn adb_shell_ws(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let udid = path.into_inner();
    let phone_service = state.phone_service.clone();
    let device = match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(d)) => d,
        _ => return HttpResponse::NotFound().body("Device not found"),
    };

    let serial = device
        .get("serial")
        .and_then(|v| v.as_str())
        .unwrap_or(&udid)
        .to_string();

    match actix_ws::handle(&req, stream) {
        Ok((resp, session, msg_stream)) => {
            actix_web::rt::spawn(async move {
                adb_shell_session(session, msg_stream, serial).await;
            });
            resp
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

async fn adb_shell_session(
    mut session: actix_ws::Session,
    mut msg_stream: actix_ws::MessageStream,
    serial: String,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::process::Command;

    tracing::info!("[ADB_SHELL] Starting adb shell for {}", serial);

    let mut proc = match Command::new("adb")
        .args(["-s", &serial, "shell"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("[ADB_SHELL] Failed to spawn adb: {}", e);
            let _ = session
                .text(format!("\r\n[ERROR] Failed to start adb shell: {}\r\n", e))
                .await;
            let _ = session.close(None).await;
            return;
        }
    };

    let mut stdout = proc.stdout.take().expect("stdout piped");
    let mut stderr = proc.stderr.take().expect("stderr piped");
    let mut stdin = proc.stdin.take().expect("stdin piped");

    let session_clone = session.clone();

    // Task: read stdout/stderr → send to WebSocket
    let stdout_task = actix_web::rt::spawn(async move {
        let mut session = session_clone;
        let mut out_buf = [0u8; 4096];
        let mut err_buf = [0u8; 4096];
        loop {
            tokio::select! {
                result = stdout.read(&mut out_buf) => {
                    match result {
                        Ok(0) => break,
                        Ok(n) => {
                            if session.binary(out_buf[..n].to_vec()).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                result = stderr.read(&mut err_buf) => {
                    match result {
                        Ok(0) => {}
                        Ok(n) => {
                            if session.binary(err_buf[..n].to_vec()).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
        let _ = session.text("\r\n[SESSION_TERMINATED]").await;
        let _ = session.close(None).await;
    });

    // Main loop: read WebSocket → write to stdin
    while let Some(Ok(msg)) = msg_stream.next().await {
        match msg {
            actix_ws::Message::Text(text) => {
                let text = text.to_string();
                if text.starts_with('\x00') {
                    if stdin.write_all(text[1..].as_bytes()).await.is_err() {
                        break;
                    }
                    let _ = stdin.flush().await;
                } else if text.starts_with('\x01') {
                    // resize - not supported in raw adb shell
                } else {
                    if stdin.write_all(text.as_bytes()).await.is_err() {
                        break;
                    }
                    let _ = stdin.flush().await;
                }
            }
            actix_ws::Message::Binary(data) => {
                if stdin.write_all(&data).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
            actix_ws::Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup
    let _ = proc.kill().await;
    stdout_task.abort();
    tracing::info!("[ADB_SHELL] Session ended for {}", serial);
}
