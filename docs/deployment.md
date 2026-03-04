# Deployment

## Build

```bash
# Development
cargo build

# Production (optimized)
cargo build --release
```

**Release Profile:**
```toml
[profile.release]
opt-level = 3
lto = true
```

---

## Running

```bash
# Development
cargo run

# Production
./target/release/cloudcontrol

# With custom config
./target/release/cloudcontrol --config config/production.yaml
```

**Default Port:** 8000

---

## Requirements

### Runtime
- SQLite3
- ADB (for USB devices)
- scrcpy (optional, for screen mirroring)

### System
- Linux / macOS
- Network access to devices (WiFi: port 7912, USB: ADB)

---

## Directory Structure

```
cloudcontrol-rust/
├── config/
│   └── default_dev.yaml      # Configuration
├── database/
│   └── cloudcontrol.db       # SQLite database (auto-created)
├── resources/
│   ├── templates/            # HTML templates
│   └── static/               # Static files
└── target/release/
    └── cloudcontrol          # Binary
```

---

## Configuration

See [configuration.md](configuration.md) for details.

---

## Systemd Service (Linux)

```ini
[Unit]
Description=CloudControl Rust Server
After=network.target

[Service]
Type=simple
User=www-data
WorkingDirectory=/opt/cloudcontrol
ExecStart=/opt/cloudcontrol/cloudcontrol
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

---

## Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    sqlite3 \
    android-tools-adb \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/cloudcontrol /usr/local/bin/
COPY config/ /app/config/
COPY resources/ /app/resources/
WORKDIR /app
EXPOSE 8000
CMD ["cloudcontrol"]
```

---

## Monitoring

- **Logs:** stdout via tracing
- **Stats:** GET /nio/stats
- **Health:** GET / (redirects to /async)

---

## Troubleshooting

### USB Devices Not Detected
- Ensure ADB is installed and in PATH
- Check `adb devices` shows connected devices
- Verify USB debugging is enabled on device

### WiFi Connection Failed
- Verify device IP and port 7912
- Check firewall allows incoming connections
- Ensure uiautomator2 agent is running on device

### Database Errors
- Check file permissions on database directory
- Verify SQLite is installed
- Check disk space
