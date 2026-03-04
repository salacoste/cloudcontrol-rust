# CloudControl-Rust

> WiFi-based Android phone group control and monitoring platform

**Version:** 0.1.0
**Language:** Rust 2021 Edition
**Framework:** actix-web 4.x + tokio
**Database:** SQLite (sqlx 0.8)

---

## Overview

CloudControl-Rust is a Rust rewrite of the original Python [CloudControl](https://github.com/ZSJnbu/CloudControl) project. It provides WiFi-based management of Android devices with real-time screenshot streaming, batch operations, and WebSocket control channels.

## Key Features

- **WiFi Device Management** - Connect and control Android devices over WiFi
- **USB Device Support** - Automatic detection and ADB forwarding for USB-connected devices
- **Real-time Screenshot Streaming** - Low-latency WebSocket-based screenshot streaming
- **Batch Operations** - Execute operations on multiple devices simultaneously
- **Remote Inspector** - Web-based UI hierarchy inspection and remote control
- **Scrcpy Integration** - High-performance screen mirroring via scrcpy

## Technology Stack

| Component | Technology | Version |
|-----------|------------|---------|
| Web Framework | actix-web | 4.x |
| Async Runtime | tokio | 1.x |
| Database | SQLite (sqlx) | 0.8 |
| Template Engine | tera | 1.x |
| HTTP Client | reqwest | 0.12 |
| Caching | moka | 0.12 |
| Serialization | serde_json | 1.x |
| Image Processing | image | 0.25 |
| Logging | tracing | 0.1.x |

## Project Structure

```
cloudcontrol-rust/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Module exports
│   ├── config.rs            # Configuration
│   ├── error.rs             # Error handlers
│   ├── state.rs             # AppState
│   ├── db/
│   │   └── sqlite.rs         # Database operations
│   ├── device/
│   │   ├── adb.rs            # ADB wrapper
│   │   ├── atx_client.rs     # ATX Agent client
│   │   ├── atx_init.rs       # ATX initialization
│   │   └── scrcpy.rs          # Scrcpy integration
│   ├── models/
│   │   ├── device.rs         # Device model
│   │   └── file.rs           # File model
│   ├── pool/
│   │   ├── batch_processor.rs    # Batch processing
│   │   ├── connection_pool.rs    # Connection pool
│   │   └── screenshot_cache.rs   # Screenshot cache
│   ├── routes/
│   │   ├── control.rs        # HTTP routes
│   │   ├── nio.rs            # NIO WebSocket
│   │   └── scrcpy_ws.rs       # Scrcpy WebSocket
│   ├── services/
│   │   ├── device_detector.rs  # USB detection
│   │   ├── device_service.rs   # Device operations
│   │   ├── file_service.rs     # File management
│   │   ├── phone_service.rs    # Device lifecycle
│   │   └── scrcpy_manager.rs   # Scrcpy management
│   └── utils/
│       ├── hierarchy.rs       # XML to JSON
│       └── host_ip.rs          # Host IP
├── tests/
│   ├── common/mod.rs          # Test utilities
│   ├── test_config.rs         # Config tests
│   ├── test_database.rs       # Database tests
│   ├── test_services.rs       # Service tests
│   └── test_server.rs          # E2E tests
├── config/
│   └── default_dev.yaml       # Configuration file
├── resources/
│   ├── templates/             # HTML templates
│   └── static/                # Static files
└── Cargo.toml                 # Dependencies
```

## Quick Start

```bash
# Build
cargo build --release

# Run
cargo run

# Access
open http://localhost:8000
```

## Test Coverage

- **90 total tests**
- 52 unit tests
- 38 integration tests
- 20 E2E tests

## Documentation

- [Architecture](architecture.md) - System architecture
- [API Endpoints](api-endpoints.md) - HTTP and WebSocket endpoints
- [Data Models](data-models.md) - Database schemas
- [Services](services.md) - Business logic
- [Configuration](configuration.md) - Configuration options
- [Tests](tests.md) - Testing guide
- [Deployment](deployment.md) - Deployment guide

---

*Generated: 2025-03-05*
