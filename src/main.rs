use cloudcontrol::config;
use cloudcontrol::db;
use cloudcontrol::error;
use cloudcontrol::middleware;
use cloudcontrol::pool;
use cloudcontrol::routes;
use cloudcontrol::services;
use cloudcontrol::state;
use cloudcontrol::utils;

use actix_files as fs;
use actix_web::middleware::ErrorHandlers;
use actix_web::{web, App, HttpServer};
use clap::Parser;
use std::sync::Arc;
use std::time::Duration;

/// CloudControl server - WiFi-based mobile device group control and monitoring platform
#[derive(Parser)]
#[command(name = "cloudcontrol")]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// Path to configuration file (YAML format)
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,
}

/// Resolve config path with priority: CLI > ENV > default.
/// Uses eprintln! before tracing is initialized for boot debugging (Code Review Issue #4).
/// Supports tilde expansion for home directory paths (Code Review Issue #3).
fn resolve_config_path(args: &CliArgs) -> String {
    let raw_path = if let Some(path) = &args.config {
        eprintln!("[Config] Using CLI config path: {}", path);
        path.clone()
    } else if let Ok(path) = std::env::var("CONFIG_PATH") {
        eprintln!("[Config] Using CONFIG_PATH env: {}", path);
        path
    } else {
        eprintln!("[Config] Using default config path: config/default_dev.yaml");
        "config/default_dev.yaml".to_string()
    };

    // Expand tilde (~) to home directory (Code Review Issue #3)
    if raw_path.starts_with("~/") {
        if let Some(home) = std::env::var("HOME").ok() {
            return format!("{}{}", home, &raw_path[1..]);
        }
    }
    raw_path
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // ── Parse CLI args BEFORE logging init (Story 12-4) ──
    let args = CliArgs::parse();
    let config_path = resolve_config_path(&args);

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

    // ── Load configuration (Story 12-4: configurable path) ──
    // Note: eprintln! already logged path above for pre-tracing debugging (Issue #4)
    tracing::info!("[Config] Loading from: {}", config_path);
    let config = config::AppConfig::load(&config_path)
        .unwrap_or_else(|e| {
            tracing::error!("[Config] Failed to load: {}", e);
            panic!("Failed to load configuration from '{}': {}", config_path, e);
        });
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

    // ── Initialize connection pool (Story 12-4: configurable settings) ──
    let pool_max_size = config.pool.max_size;
    let pool_idle_timeout = Duration::from_secs(config.pool.idle_timeout_secs);
    tracing::info!(
        "[Pool] Initializing: max_size={}, idle_timeout={}s",
        pool_max_size,
        config.pool.idle_timeout_secs
    );
    let connection_pool = pool::connection_pool::ConnectionPool::new(
        pool_max_size,
        pool_idle_timeout,
    );

    // ── Load templates ──
    let tera = tera::Tera::new("resources/templates/**/*")
        .expect("Failed to load templates");
    tracing::info!("Templates loaded");

    // ── Get host IP ──
    let host_ip = utils::host_ip::get_host_ip();
    tracing::info!("Host IP: {}", host_ip);

    // ── Build shared state ──
    let mut app_state = state::AppState::new(
        db.clone(),
        config.clone(),
        connection_pool,
        tera.clone(),
        host_ip,
    );

    // ── Check FFmpeg availability (Story 11-1) ──
    app_state.ffmpeg_available = services::video_service::check_ffmpeg_available().await;

    // ── Video recording startup recovery (Story 11-2) ──
    if let Err(e) = app_state.video_service.recover_on_startup().await {
        tracing::warn!("Video recovery on startup failed: {}", e);
    }

    // ── Start device detector ──
    let detector = Arc::new(services::device_detector::DeviceDetector::new(phone_service.clone()));
    detector.start().await;
    tracing::info!("USB device auto-detection started");

    // ── Start WiFi discovery ──
    let wifi_discovery = Arc::new(services::wifi_discovery::WifiDiscovery::new(phone_service.clone()));
    wifi_discovery.start().await;
    tracing::info!("WiFi device auto-discovery started");

    // ── Log auth status (Story 12-1) ──
    if app_state.api_key_enabled {
        tracing::info!("API authentication enabled");
    } else {
        tracing::warn!("API authentication disabled — all endpoints are open");
    }

    // ── Create rate limiter (Story 12-2) ──
    let rate_limiter = app_state.config.rate_limit.as_ref().map(|cfg| {
        Arc::new(middleware::RateLimiter::new(cfg.clone()))
    });
    if app_state.rate_limiting_enabled {
        let cfg = app_state.config.rate_limit.as_ref().unwrap();
        tracing::info!(
            "Rate limiting enabled: {} req/{} sec per IP",
            cfg.requests_per_window,
            cfg.window_secs
        );
    } else {
        tracing::info!("Rate limiting disabled");
    }

    // ── Clone service refs for shutdown handler (Story 12-3) ──
    let scrcpy_ref = app_state.scrcpy_manager.clone();
    let video_ref = app_state.video_service.clone();
    let recording_ref = app_state.recording_service.clone();

    // ── Start HTTP server ──
    tracing::info!("Starting server on http://0.0.0.0:{}", port);

    let tera_data = web::Data::new(tera);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .app_data(tera_data.clone())
            // Rate limiting middleware (Story 12-2) — runs after auth
            .wrap(middleware::RateLimit::new(rate_limiter.clone()))
            // API key authentication middleware (Story 12-1) — runs first
            .wrap(middleware::ApiKeyAuth::new(app_state.config.api_key.clone()))
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
            .route("/test", web::get().to(routes::control::test_page))
            // ── Device API ──
            .route("/list", web::get().to(routes::control::device_list))
            .route(
                "/devices/{udid}/info",
                web::get().to(routes::control::device_info),
            )
            .route(
                "/devices/{udid}/edit",
                web::get().to(routes::control::edit_page),
            )
            .route(
                "/devices/{udid}/product",
                web::put().to(routes::control::update_device_product),
            )
            .route(
                "/devices/{udid}/property",
                web::get().to(routes::control::property_page),
            )
            .route(
                "/api/v1/devices/{udid}/property",
                web::post().to(routes::control::update_device_property),
            )
            .route(
                "/providers",
                web::get().to(routes::control::providers_page),
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
            .route(
                "/api/screenshot/batch",
                web::post().to(routes::control::batch_screenshot),
            )
            // ── Batch Control Operations ──
            .route(
                "/api/batch/tap",
                web::post().to(routes::control::batch_tap),
            )
            .route(
                "/api/batch/swipe",
                web::post().to(routes::control::batch_swipe),
            )
            .route(
                "/api/batch/input",
                web::post().to(routes::control::batch_input),
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
            // ── Rotation ──
            .route(
                "/inspector/{udid}/rotation",
                web::post().to(routes::control::inspector_rotation),
            )
            // ── Inspector Shell (HTTP proxy) ──
            .route(
                "/inspector/{udid}/shell",
                web::post().to(routes::control::inspector_shell),
            )
            .route(
                "/inspector/{udid}/shell",
                web::get().to(routes::control::inspector_shell_get),
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
            // ── Device Disconnect/Reconnect ──
            .route(
                "/api/devices/{udid}",
                web::delete().to(routes::control::disconnect_device),
            )
            .route(
                "/api/devices/{udid}/reconnect",
                web::post().to(routes::control::reconnect_device),
            )
            // ── Device Tags ──
            .route(
                "/api/devices/{udid}/tags",
                web::post().to(routes::control::add_device_tags),
            )
            .route(
                "/api/devices/{udid}/tags/{tag}",
                web::delete().to(routes::control::remove_device_tag),
            )
            // ── Connection History ──
            .route(
                "/api/devices/{udid}/history",
                web::get().to(routes::control::get_connection_history),
            )
            .route(
                "/api/devices/{udid}/stats",
                web::get().to(routes::control::get_connection_stats),
            )
            // ── Recording System ──
            .route(
                "/api/recordings/start",
                web::post().to(routes::recording::start_recording),
            )
            .route(
                "/api/recordings/{id}/action",
                web::post().to(routes::recording::record_action),
            )
            .route(
                "/api/recordings/{id}/stop",
                web::post().to(routes::recording::stop_recording),
            )
            .route(
                "/api/recordings/{id}/pause",
                web::post().to(routes::recording::pause_recording),
            )
            .route(
                "/api/recordings/{id}/resume",
                web::post().to(routes::recording::resume_recording),
            )
            .route(
                "/api/recordings/{id}/cancel",
                web::post().to(routes::recording::cancel_recording),
            )
            .route(
                "/api/recordings/{id}/status",
                web::get().to(routes::recording::get_recording_status),
            )
            .route(
                "/api/recordings/{id}/actions/{action_id}",
                web::delete().to(routes::recording::delete_action),
            )
            .route(
                "/api/recordings/{id}/actions/{action_id}",
                web::put().to(routes::recording::edit_action),
            )
            .route("/api/recordings", web::get().to(routes::recording::list_recordings))
            .route("/api/recordings/{id}", web::get().to(routes::recording::get_recording))
            .route("/api/recordings/{id}", web::delete().to(routes::recording::delete_recording))
            // ── API V1 Endpoints ──
            .route("/api/v1/devices", web::get().to(routes::api_v1::list_devices))
            .route("/api/v1/devices/{udid}", web::get().to(routes::api_v1::get_device))
            .route("/api/v1/devices/{udid}/screenshot", web::get().to(routes::api_v1::get_screenshot))
            .route("/api/v1/devices/{udid}/tap", web::post().to(routes::api_v1::tap))
            .route("/api/v1/devices/{udid}/swipe", web::post().to(routes::api_v1::swipe))
            .route("/api/v1/devices/{udid}/input", web::post().to(routes::api_v1::input))
            .route("/api/v1/devices/{udid}/keyevent", web::post().to(routes::api_v1::keyevent))
            .route("/api/v1/batch/tap", web::post().to(routes::api_v1::batch_tap))
            .route("/api/v1/batch/swipe", web::post().to(routes::api_v1::batch_swipe))
            .route("/api/v1/batch/input", web::post().to(routes::api_v1::batch_input))
            .route("/api/v1/openapi.json", web::get().to(routes::api_v1::openapi_spec))
            .route("/api/v1/version", web::get().to(routes::api_v1::get_version))
            // ── API V1 Status & Health Endpoints (Story 5-3) ──
            .route("/api/v1/status", web::get().to(routes::api_v1::get_device_status))
            .route("/api/v1/health", web::get().to(routes::api_v1::health_check))
            .route("/api/v1/metrics", web::get().to(routes::api_v1::get_metrics))
            // ── Product Catalog API ──
            .route("/api/v1/products", web::get().to(routes::api_v1::list_products))
            .route("/api/v1/products", web::post().to(routes::api_v1::create_product))
            .route("/api/v1/products/{id}", web::get().to(routes::api_v1::get_product))
            .route("/api/v1/products/{id}", web::put().to(routes::api_v1::update_product))
            .route("/api/v1/products/{id}", web::delete().to(routes::api_v1::delete_product))
            // ── Authentication API (Story 14-1) ──
            .route("/api/v1/auth/register", web::post().to(routes::auth::register))
            .route("/api/v1/auth/login", web::post().to(routes::auth::login))
            .route("/api/v1/auth/refresh", web::post().to(routes::auth::refresh))
            .route("/api/v1/auth/logout", web::post().to(routes::auth::logout))
            .route("/api/v1/auth/logout-all", web::post().to(routes::auth::logout_all))
            .route("/api/v1/auth/me", web::get().to(routes::auth::get_me))
            .route("/api/v1/auth/status", web::get().to(routes::auth::auth_status))
            // ── Admin API (Story 14-2: RBAC) ──
            .route("/api/v1/admin/users", web::get().to(routes::admin::list_users))
            .route("/api/v1/admin/users/{id}/role", web::post().to(routes::admin::assign_role))
            // ── Provider Registry API ──
            .route("/api/v1/providers", web::get().to(routes::api_v1::list_providers))
            .route("/api/v1/providers", web::post().to(routes::api_v1::create_provider))
            .route("/api/v1/providers/{id}", web::get().to(routes::api_v1::get_provider))
            .route("/api/v1/providers/{id}", web::put().to(routes::api_v1::update_provider))
            .route("/api/v1/providers/{id}/heartbeat", web::post().to(routes::api_v1::provider_heartbeat))
            // ── Hierarchy, Upload, Rotation (Story 10-4) ──
            .route("/api/v1/devices/{udid}/hierarchy", web::get().to(routes::api_v1::hierarchy))
            .route("/api/v1/devices/{udid}/upload", web::post().to(routes::api_v1::upload))
            .route("/api/v1/devices/{udid}/rotation", web::post().to(routes::api_v1::rotation))
            // ── Video Recording (Story 11-1) ──
            .route("/api/v1/videos", web::get().to(routes::api_v1::list_videos))
            .route("/api/v1/videos/{id}", web::get().to(routes::api_v1::get_video))
            .route("/api/v1/videos/{id}/download", web::get().to(routes::api_v1::download_video))
            .route("/api/v1/videos/{id}", web::delete().to(routes::api_v1::delete_video))
            .route("/api/v1/videos/{id}/stop", web::post().to(routes::api_v1::stop_video))
            // Legacy endpoint for edit.html compatibility (Story 8.2)
            .route("/products/{brand}/{model}", web::get().to(routes::api_v1::list_products_by_brand_model))
            // ── API V1 WebSocket Endpoints ──
            .route(
                "/api/v1/ws/screenshot/{udid}",
                web::get().to(routes::api_v1::ws_screenshot),
            )
            .route(
                "/api/v1/ws/nio",
                web::get().to(routes::api_v1::ws_nio),
            )
            // ── Playback System ──
            .route(
                "/api/recordings/{id}/play",
                web::post().to(routes::recording::start_playback),
            )
            .route(
                "/api/recordings/{id}/playback/status",
                web::get().to(routes::recording::get_playback_status),
            )
            .route(
                "/api/recordings/{id}/playback/stop",
                web::post().to(routes::recording::stop_playback),
            )
            .route(
                "/api/recordings/{id}/playback/pause",
                web::post().to(routes::recording::pause_playback),
            )
            .route(
                "/api/recordings/{id}/playback/resume",
                web::post().to(routes::recording::resume_playback),
            )
            // ── Shell Command Execution ──
            .route(
                "/api/devices/{udid}/shell",
                web::post().to(routes::control::execute_shell),
            )
            // ── Batch Report Export ──
            .route(
                "/api/batch/reports",
                web::get().to(routes::batch_report::list_batch_reports),
            )
            .route(
                "/api/batch/reports/{id}",
                web::get().to(routes::batch_report::get_batch_report),
            )
            .route(
                "/api/batch/reports/{id}",
                web::delete().to(routes::batch_report::delete_batch_report),
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
            // ── Video Recording WebSocket (Story 11-1) ──
            .route("/video/convert", web::get().to(routes::video_ws::video_convert_ws))
            // ── Scrcpy WebSocket ──
            .route(
                "/scrcpy/{udid}/ws",
                web::get().to(routes::scrcpy_ws::scrcpy_websocket),
            )
            .route(
                "/scrcpy/{udid}/status",
                web::get().to(routes::scrcpy_ws::scrcpy_status),
            )
            // ── Scrcpy Session Management ──
            .route(
                "/scrcpy/{udid}/start",
                web::post().to(routes::scrcpy::start_scrcpy_session),
            )
            .route(
                "/scrcpy/{udid}/stop",
                web::post().to(routes::scrcpy::stop_scrcpy_session),
            )
            .route(
                "/scrcpy/sessions",
                web::get().to(routes::scrcpy::list_scrcpy_sessions),
            )
            // ── Scrcpy Device Control ──
            .route(
                "/scrcpy/{udid}/tap",
                web::post().to(routes::scrcpy::scrcpy_tap),
            )
            .route(
                "/scrcpy/{udid}/key",
                web::post().to(routes::scrcpy::scrcpy_key),
            )
            .route(
                "/scrcpy/{udid}/swipe",
                web::post().to(routes::scrcpy::scrcpy_swipe),
            )
            // ── Scrcpy Recording ──
            .route(
                "/scrcpy/{udid}/recording/start",
                web::post().to(routes::scrcpy::start_scrcpy_recording),
            )
            .route(
                "/scrcpy/{udid}/recording/stop",
                web::post().to(routes::scrcpy::stop_scrcpy_recording),
            )
            .route(
                "/scrcpy/recordings",
                web::get().to(routes::scrcpy::list_scrcpy_recordings),
            )
            .route(
                "/scrcpy/recordings/{id}",
                web::get().to(routes::scrcpy::get_scrcpy_recording),
            )
            .route(
                "/scrcpy/recordings/{id}/download",
                web::get().to(routes::scrcpy::download_scrcpy_recording),
            )
            .route(
                "/scrcpy/recordings/{id}",
                web::delete().to(routes::scrcpy::delete_scrcpy_recording),
            )
            // ── Static files ──
            .service(fs::Files::new("/static", "resources/static").show_files_listing())
    })
    .shutdown_timeout(10)
    .bind(format!("0.0.0.0:{}", port))?
    .run();

    let server_handle = server.handle();

    // ── Spawn shutdown signal listener (Story 12-3) ──
    let detector_ref = detector.clone();
    let wifi_ref = wifi_discovery.clone();

    tokio::spawn(async move {
        shutdown_signal().await;
        tracing::info!("[Shutdown] Signal received, beginning graceful shutdown...");

        // Second signal = force exit immediately
        tokio::spawn(async {
            shutdown_signal().await;
            tracing::warn!("[Shutdown] Second signal received, forcing immediate exit");
            std::process::exit(1);
        });

        let cleanup = async {
            wifi_ref.stop().await;
            tracing::info!("[Shutdown] WiFi discovery stopped");

            detector_ref.stop().await;
            tracing::info!("[Shutdown] Device detector stopped");

            scrcpy_ref.stop_all_sessions().await;
            tracing::info!("[Shutdown] Scrcpy sessions stopped");

            video_ref.stop_all_active().await;
            tracing::info!("[Shutdown] Video recordings stopped");

            recording_ref.stop_all_playbacks().await;
            tracing::info!("[Shutdown] Playbacks stopped");

            server_handle.stop(true).await;
            tracing::info!("[Shutdown] HTTP server stopped");
        };

        if tokio::time::timeout(Duration::from_secs(10), cleanup)
            .await
            .is_err()
        {
            tracing::error!("[Shutdown] Cleanup timed out after 10s, forcing exit");
            std::process::exit(1);
        }
        tracing::info!("[Shutdown] Clean shutdown complete");
    });

    server.await
}

/// Wait for SIGINT (Ctrl+C) or SIGTERM shutdown signal.
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler");

        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
}

// ── Tests for CLI parsing and path resolution (Code Review Issue #6) ──
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_config_path_cli_arg() {
        let args = CliArgs {
            config: Some("/custom/config.yaml".to_string()),
        };
        let path = resolve_config_path(&args);
        assert_eq!(path, "/custom/config.yaml");
    }

    #[test]
    fn test_resolve_config_path_tilde_expansion() {
        // Test that ~/ is expanded to $HOME
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let args = CliArgs {
            config: Some("~/my-config.yaml".to_string()),
        };
        let path = resolve_config_path(&args);
        assert_eq!(path, format!("{}/my-config.yaml", home));
        assert!(!path.contains("~"));
    }

    #[test]
    fn test_resolve_config_path_tilde_expansion_trailing() {
        // Test edge case: just ~ with no trailing slash
        let args = CliArgs {
            config: Some("~".to_string()),
        };
        let path = resolve_config_path(&args);
        // Should not expand since it's just "~" not "~/"
        assert_eq!(path, "~");
    }

    #[test]
    fn test_resolve_config_path_no_expansion_for_relative() {
        // Test that relative paths are not modified
        let args = CliArgs {
            config: Some("config/local.yaml".to_string()),
        };
        let path = resolve_config_path(&args);
        assert_eq!(path, "config/local.yaml");
    }

    #[test]
    fn test_resolve_config_path_no_expansion_for_absolute() {
        // Test that absolute paths are not modified
        let args = CliArgs {
            config: Some("/etc/cloudcontrol/config.yaml".to_string()),
        };
        let path = resolve_config_path(&args);
        assert_eq!(path, "/etc/cloudcontrol/config.yaml");
    }
}
