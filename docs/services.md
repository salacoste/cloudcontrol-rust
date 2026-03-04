# Services

## Overview

Services contain the business logic layer, separating HTTP handling from device operations.

---

## PhoneService

**File:** `src/services/phone_service.rs`

**Purpose:** Device lifecycle management

### Methods

| Method | Purpose |
|--------|---------|
| `on_connected(identifier, host)` | Register new device, fetch info from ATX agent |
| `re_connected(identifier, host)` | Update IP on reconnection |
| `offline_connected(identifier)` | Mark device offline |
| `update_field(identifier, item)` | Generic field update |
| `query_info_by_udid(udid)` | Get device by UDID |
| `query_device_list()` | Get all devices |
| `query_device_list_by_present()` | Get online devices only |
| `delete_devices()` | Clear all devices |

### On Connected Flow

```rust
pub async fn on_connected(&self, identifier: &str, host: &str) -> Result<(), String> {
    // 1. Fetch device info from ATX agent
    let url = format!("http://{}:9008/info", host);
    let info = client.get(&url).send().await?.json().await?;

    // 2. Build device data
    let data = json!({
        "udid": identifier,
        "ip": host,
        "port": 9008,
        "present": true,
        "ready": true,
        // ... merge ATX agent fields
    });

    // 3. Upsert to database
    self.db.upsert(identifier, &data).await?;
}
```

---

## DeviceService

**File:** `src/services/device_service.rs`

**Purpose:** High-level device operations

### Methods

| Method | Purpose |
|--------|---------|
| `screenshot_base64(client, quality, scale)` | Screenshot as base64 JPEG |
| `screenshot_jpeg(client, quality, scale)` | Screenshot as raw JPEG bytes |
| `screenshot_usb_base64(serial, quality, scale)` | USB screenshot (base64) |
| `screenshot_usb_jpeg(serial, quality, scale)` | USB screenshot (raw bytes) |
| `encode_screenshot(raw_bytes, quality, scale)` | Convert raw to base64 |
| `dump_hierarchy(client)` | Get UI hierarchy as JSON |

### Image Processing

```rust
// Uses Nearest filter for speed (matches Python resample=0)
fn resize_jpeg(data: &[u8], quality: u8, scale: f64) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(data)?;
    let img = if scale < 1.0 {
        img.resize(new_w, new_h, image::imageops::FilterType::Nearest)
    } else { img };

    let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder.encode_image(&img.to_rgb8())?;
    Ok(buf.into_inner())
}
```

### USB Screenshot Flow

```rust
pub async fn screenshot_usb_jpeg(serial: &str, quality: u8, scale: f64) -> Result<Vec<u8>, String> {
    // 1. ADB screencap (PNG)
    let png_bytes = Adb::screencap(serial).await?;

    // 2. Convert to JPEG with scale
    let jpeg_bytes = Self::resize_jpeg(&png_bytes, quality, scale)?;

    // 3. Log timing
    tracing::info!("[Screenshot] USB q={} | total={:.0}ms", quality, total_ms);

    Ok(jpeg_bytes)
}
```

---

## FileService

**File:** `src/services/file_service.rs`

**Purpose:** File upload and installation tracking

### Methods

| Method | Purpose |
|--------|---------|
| `save_install_file(data)` | Save file record |
| `query_install_file(group, offset, limit, search)` | Paginated file list |
| `query_all_install_file()` | Count all files |
| `delete_install_file(group, filename)` | Delete file record |

---

## DeviceDetector

**File:** `src/services/device_detector.rs`

**Purpose:** Background USB device auto-detection

### How It Works

```rust
pub struct DeviceDetector {
    phone_service: PhoneService,
}

impl DeviceDetector {
    pub async fn start(&self) {
        tokio::spawn(async move {
            loop {
                // 1. Get USB devices via ADB
                let devices = Adb::devices().await;

                // 2. For each USB device
                for serial in devices {
                    // 3. Setup ADB forward
                    let port = Adb::forward(&serial, 9008).await;

                    // 4. Register with PhoneService
                    phone_service.update_field(&serial, &json!({
                        "serial": serial,
                        "ip": "127.0.0.1",
                        "port": port,
                        "present": true,
                    })).await;
                }

                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    }
}
```

---

## ScrcpyManager

**File:** `src/services/scrcpy_manager.rs`

**Purpose:** Manage scrcpy processes for screen mirroring

### Key Operations

- Start/stop scrcpy processes
- Parse video streams
- Handle WebSocket streaming
