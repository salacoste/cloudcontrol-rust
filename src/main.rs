use cloudcontrol::config;
use cloudcontrol::db;
use cloudcontrol::error;
use cloudcontrol::pool;
use cloudcontrol::routes;
use cloudcontrol::services;
use cloudcontrol::state;
use cloudcontrol::utils;

use actix_files as fs;
use actix_web::middleware::ErrorHandlers;
use actix_web::{web, App, HttpServer};
use std::time::Duration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // ── Initialize logging ──
    let file_appender = tracing_appender::rolling::daily("log", "app.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    use tracing_subscriber::fmt::writer::MakeWriterExt;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cloudcontrol=info,actix_web=info".into()),
        )
        .with_writer(std::io::stdout.and(non_blocking))
        .init();

    tracing::info!("CloudControl Rust starting...");

    // ── Load configuration ──
    let config = config::AppConfig::load("config/default_dev.yaml")
        .expect("Failed to load configuration");
    let port = config.server.port;

    // ── Initialize database ──
    let db = db::Database::new("database", &config.db_configs.db_name)
        .await
        .expect("Failed to initialize database");

    // Restore persisted devices on startup (device state persistence)
    let phone_service = services::phone_service::PhoneService::new(db.clone());
    phone_service
        .restore_devices()
        .await
        .expect("Failed to restore devices");
    tracing::info!("Database initialized, device state restored");

    // ── Initialize connection pool ──
    let connection_pool = pool::connection_pool::ConnectionPool::new(
        1200,
        Duration::from_secs(600),
    );

    // ── Load templates ──
    let tera = tera::Tera::new("resources/templates/**/*")
        .expect("Failed to load templates");
    tracing::info!("Templates loaded");

    // ── Get host IP ──
    let host_ip = utils::host_ip::get_host_ip();
    tracing::info!("Host IP: {}", host_ip);

    // ── Build shared state ──
    let app_state = state::AppState::new(
        db.clone(),
        config.clone(),
        connection_pool,
        tera.clone(),
        host_ip,
    );

    // ── Start device detector ──
    let detector = services::device_detector::DeviceDetector::new(phone_service.clone());
    detector.start().await;
    tracing::info!("USB device auto-detection started");

    // ── Start WiFi discovery ──
    let wifi_discovery = services::wifi_discovery::WifiDiscovery::new(phone_service.clone());
    wifi_discovery.start().await;
    tracing::info!("WiFi device auto-discovery started");

    // ── Start HTTP server ──
    tracing::info!("Starting server on http://0.0.0.0:{}", port);

    let tera_data = web::Data::new(tera);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .app_data(tera_data.clone())
            // Error handlers for 404/500
            .wrap(
                ErrorHandlers::new()
                    .handler(actix_web::http::StatusCode::NOT_FOUND, error::handle_404)
                    .handler(
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                        error::handle_500,
                    ),
            )
            // ── Page routes ──
            .route("/", web::get().to(routes::control::index))
            .route(
                "/devices/{udid}/remote",
                web::get().to(routes::control::remote),
            )
            .route("/async", web::get().to(routes::control::async_list_get))
            .route("/async", web::post().to(routes::control::async_list_page))
            .route(
                "/installfile",
                web::get().to(routes::control::installfile),
            )
            // ── Device API ──
            .route("/list", web::get().to(routes::control::device_list))
            .route(
                "/devices/{udid}/info",
                web::get().to(routes::control::device_info),
            )
            // ── Screenshot ──
            .route(
                "/inspector/{udid}/screenshot",
                web::get().to(routes::control::inspector_screenshot),
            )
            .route(
                "/inspector/{udid}/screenshot/img",
                web::get().to(routes::control::inspector_screenshot_img),
            )
            // ── Touch / Input / Keyevent ──
            .route(
                "/inspector/{udid}/touch",
                web::post().to(routes::control::inspector_touch),
            )
            .route(
                "/inspector/{udid}/input",
                web::post().to(routes::control::inspector_input),
            )
            .route(
                "/inspector/{udid}/keyevent",
                web::post().to(routes::control::inspector_keyevent),
            )
            // ── Hierarchy ──
            .route(
                "/inspector/{udid}/hierarchy",
                web::get().to(routes::control::inspector_hierarchy),
            )
            // ── File upload ──
            .route(
                "/inspector/{udid}/upload",
                web::post().to(routes::control::inspector_upload),
            )
            .route("/upload", web::post().to(routes::control::store_file_handler))
            .route(
                "/upload_group/{path}",
                web::post().to(routes::control::upload_group),
            )
            // ── Heartbeat ──
            .route(
                "/heartbeat",
                web::post().to(routes::control::heartbeat),
            )
            // ── Shell ──
            .route("/shell", web::post().to(routes::control::shell))
            // ── WiFi Connect ──
            .route(
                "/api/wifi-connect",
                web::post().to(routes::control::wifi_connect),
            )
            // ── Manual Device Addition ──
            .route(
                "/api/devices/add",
                web::post().to(routes::control::add_device),
            )
            // ── Files management ──
            .route("/files", web::get().to(routes::control::files))
            .route(
                "/file/delete/{group}/{filename}",
                web::get().to(routes::control::file_delete),
            )
            // ── ATX Agent ──
            .route(
                "/atxagent",
                web::get().to(routes::control::atxagent),
            )
            // ── WebSocket stubs ──
            .route("/feeds", web::get().to(routes::control::feeds))
            .route(
                "/devices/{query}/reserved",
                web::get().to(routes::control::reserved),
            )
            // ── ADB Shell WebSocket ──
            .route(
                "/devices/{udid}/shell",
                web::get().to(routes::control::adb_shell_ws),
            )
            // ── NIO WebSocket ──
            .route(
                "/nio/{udid}/ws",
                web::get().to(routes::nio::nio_websocket),
            )
            .route("/nio/stats", web::get().to(routes::nio::nio_stats))
            // ── Scrcpy WebSocket ──
            .route(
                "/scrcpy/{udid}/ws",
                web::get().to(routes::scrcpy_ws::scrcpy_websocket),
            )
            .route(
                "/scrcpy/{udid}/status",
                web::get().to(routes::scrcpy_ws::scrcpy_status),
            )
            // ── Static files ──
            .service(fs::Files::new("/static", "resources/static").show_files_listing())
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}
