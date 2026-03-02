use crate::device::adb::Adb;
use crate::device::atx_client::AtxClient;
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

// ─── Helper: get device + atx client ───

async fn resolve_device_connection(device: &Value, ip: &str, port: i64) -> (String, i64) {
    // If IP is valid and non-loopback, use it directly
    if !ip.is_empty() && ip != "127.0.0.1" {
        return (ip.to_string(), port);
    }

    // Already forwarded (ip=127.0.0.1 with a non-standard port) → use as-is
    if ip == "127.0.0.1" && port != 9008 {
        return (ip.to_string(), port);
    }

    // USB/emulator device with empty IP: try adb forward
    let serial = device
        .get("serial")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !serial.is_empty() {
        if let Ok(local_port) = crate::device::adb::Adb::forward(serial, 9008).await {
            return ("127.0.0.1".to_string(), local_port as i64);
        }
    }

    (ip.to_string(), port)
}

async fn get_device_client(
    state: &AppState,
    udid: &str,
) -> Result<(Value, Arc<AtxClient>), HttpResponse> {
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
        .map_err(|e| HttpResponse::InternalServerError().json(json!({"status":"error","message":e})))?
        .ok_or_else(|| HttpResponse::NotFound().json(json!({"status":"error","message":"Device not found"})))?;

    state.device_info_cache.insert(udid.to_string(), device.clone()).await;

    let ip = device.get("ip").and_then(|v| v.as_str()).unwrap_or("");
    let port = device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
    let (final_ip, final_port) = resolve_device_connection(&device, ip, port).await;
    let client = state.connection_pool.get_or_create(udid, &final_ip, final_port).await;
    Ok((device, client))
}

// ═══════════════ PAGE ROUTES ═══════════════

/// GET / → index.html (static file response)
pub async fn index(state: web::Data<AppState>) -> HttpResponse {
    match state.tera.render("index.html", &tera::Context::new()) {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => {
            // Fallback: serve as static file
            HttpResponse::Ok()
                .content_type("text/html")
                .body("<h1>CloudControl</h1>")
        }
    }
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

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
    let device = match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(d)) => d,
        _ => return HttpResponse::NotFound().body("Device not found"),
    };

    let mut ctx = tera::Context::new();
    ctx.insert("IP", device.get("ip").and_then(|v| v.as_str()).unwrap_or(""));
    ctx.insert("Port", &device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008));
    ctx.insert("Udid", &udid);
    ctx.insert("deviceInfo", &device);
    ctx.insert("device", &device.to_string());
    ctx.insert("v", &json!({}));

    match state.tera.render("remote.html", &ctx) {
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
    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());

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
    ctx.insert("IP", device.get("ip").and_then(|v| v.as_str()).unwrap_or(""));
    ctx.insert("Port", &device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008));
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

// ═══════════════ DEVICE API ═══════════════

/// GET /list → JSON array of online devices
pub async fn device_list(state: web::Data<AppState>) -> HttpResponse {
    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
    match phone_service.query_device_list().await {
        Ok(devices) => HttpResponse::Ok().json(devices),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": e})),
    }
}

/// GET /devices/{udid}/info → device info JSON
pub async fn device_info(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
    match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(device)) => HttpResponse::Ok().json(device),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Device not found"})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": e})),
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

    match DeviceService::screenshot_base64(&client, quality, scale).await {
        Ok(b64) => HttpResponse::Ok().json(json!({
            "type": "jpeg",
            "encoding": "base64",
            "data": b64,
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({"status":"error","message":e})),
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
        let (_device, client) = get_device_client(&state, &udid).await?;
        DeviceService::screenshot_jpeg(&client, quality, scale)
            .await
            .map_err(|e| HttpResponse::NotFound().body(e))
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

// ═══════════════ TOUCH / INPUT / KEYEVENT ═══════════════

/// POST /inspector/{udid}/touch → fire-and-forget
pub async fn inspector_touch(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Value>,
) -> HttpResponse {
    let udid = path.into_inner();
    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let action = body.get("action").and_then(|v| v.as_str()).unwrap_or("click");
    let x = match body.get("x").and_then(|v| v.as_f64()) {
        Some(v) => v as i32,
        None => return HttpResponse::BadRequest().json(json!({"status":"error","message":"Missing coordinates"})),
    };
    let y = match body.get("y").and_then(|v| v.as_f64()) {
        Some(v) => v as i32,
        None => return HttpResponse::BadRequest().json(json!({"status":"error","message":"Missing coordinates"})),
    };

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Mock device
    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HttpResponse::Ok().json(json!({"status": "ok"}));
    }

    // Fire-and-forget
    let action = action.to_string();
    let x2 = body.get("x2").and_then(|v| v.as_f64()).unwrap_or(x as f64) as i32;
    let y2 = body.get("y2").and_then(|v| v.as_f64()).unwrap_or(y as f64) as i32;
    let duration = body.get("duration").and_then(|v| v.as_f64()).unwrap_or(200.0) / 1000.0;

    tokio::spawn(async move {
        let result = if action == "swipe" {
            client.swipe(x, y, x2, y2, duration.max(0.05).min(2.0)).await
        } else {
            client.click(x, y).await
        };
        if let Err(e) = result {
            tracing::error!("[TOUCH] Failed {}: {}", client.udid, e);
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
        return HttpResponse::BadRequest().finish();
    }

    let text = body
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if text.is_empty() {
        return HttpResponse::Ok().json(json!({"status": "ok"}));
    }

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HttpResponse::Ok().json(json!({"status": "ok"}));
    }

    tokio::spawn(async move {
        if let Err(e) = client.input_text(&text).await {
            tracing::error!("[INPUT] Failed {}: {}", client.udid, e);
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
        return HttpResponse::BadRequest().finish();
    }

    let key = body
        .get("key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let (device, client) = match get_device_client(&state, &udid).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if device.get("is_mock").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HttpResponse::Ok().json(json!({"status": "ok"}));
    }

    // Android keycode mapping
    let android_key = match key.as_str() {
        "Enter" => "enter",
        "Backspace" | "DEL" => "del",
        "Delete" => "forward_del",
        "Home" | "HOME" | "home" => "home",
        "Back" | "BACK" | "back" => "back",
        "Tab" => "tab",
        "Escape" => "back",
        "ArrowUp" => "dpad_up",
        "ArrowDown" => "dpad_down",
        "ArrowLeft" => "dpad_left",
        "ArrowRight" => "dpad_right",
        "Menu" | "MENU" | "menu" => "menu",
        "Power" | "POWER" | "power" => "power",
        "WAKEUP" | "wakeup" => "wakeup",
        other => other,
    }
    .to_string();

    tokio::spawn(async move {
        if let Err(e) = client.press_key(&android_key).await {
            tracing::error!("[KEYEVENT] Failed {}: {}", client.udid, e);
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
        return HttpResponse::BadRequest().finish();
    }

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
    let device = match phone_service.query_info_by_udid(&udid).await {
        Ok(Some(d)) => d,
        _ => return HttpResponse::NotFound().body("Device not found"),
    };

    let ip = device.get("ip").and_then(|v| v.as_str()).unwrap_or("");
    let port = device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
    let client = AtxClient::new(ip, port, &udid);

    match DeviceService::dump_hierarchy(&client).await {
        Ok(hierarchy) => HttpResponse::Ok().json(hierarchy),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": e})),
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
            "message": format!("文件已上传到: {}", device_path),
            "path": device_path,
        }));
    }

    HttpResponse::BadRequest().json(json!({"status":"error","message":"No file uploaded"}))
}

/// POST /upload → upload file with chmod + apk install
pub async fn store_file_handler(
    state: web::Data<AppState>,
    req: HttpRequest,
    mut payload: Multipart,
) -> HttpResponse {
    let udid = req
        .headers()
        .get("Access-Control-Allow-Origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
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

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
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

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
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
    let udid = req
        .headers()
        .get("Access-Control-Allow-Origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if udid.is_empty() {
        return HttpResponse::BadRequest().finish();
    }

    let command = form.get("command").cloned().unwrap_or_default();

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
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

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
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

    let phone_service = crate::services::phone_service::PhoneService::new(state.db.clone());
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
            HttpResponse::Ok().body(format!("atx-agent[{}]成功", method))
        }
        _ => HttpResponse::NotFound().body(format!("atx-agent[{}]失败", method)),
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

/// GET /devices/{query}/reserved → legacy WebSocket heartbeat
pub async fn reserved(
    req: HttpRequest,
    stream: web::Payload,
    _path: web::Path<String>,
) -> HttpResponse {
    match actix_ws::handle(&req, stream) {
        Ok((resp, mut session, mut msg_stream)) => {
            actix_web::rt::spawn(async move {
                while let Some(Ok(msg)) = msg_stream.next().await {
                    match msg {
                        actix_ws::Message::Text(text) => {
                            let _ = session
                                .text(format!("Hello, {}", text))
                                .await;
                        }
                        actix_ws::Message::Binary(data) => {
                            let _ = session.binary(data).await;
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
