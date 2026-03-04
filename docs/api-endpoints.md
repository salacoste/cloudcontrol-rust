# API Endpoints

## HTTP Endpoints

### Pages

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/` | `index` | Redirect to /async |
| GET | `/async` | `async_list_get` | Device list page |
| POST | `/async` | `async_list_page` | Device list (form) |
| GET | `/devices/{udid}/remote` | `remote` | Remote control page |
| GET | `/installfile` | `installfile` | File upload page |

### Device API

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/list` | `device_list` | List online devices |
| GET | `/devices/{udid}/info` | `device_info` | Get device info |

### Inspector API

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/inspector/{udid}/screenshot` | `inspector_screenshot` | Screenshot (base64 JSON) |
| GET | `/inspector/{udid}/screenshot/img` | `inspector_screenshot_img` | Screenshot (JPEG) |
| POST | `/inspector/{udid}/touch` | `inspector_touch` | Touch event |
| POST | `/inspector/{udid}/input` | `inspector_input` | Text input |
| POST | `/inspector/{udid}/keyevent` | `inspector_keyevent` | Key event |
| GET | `/inspector/{udid}/hierarchy` | `inspector_hierarchy` | UI hierarchy JSON |
| POST | `/inspector/{udid}/upload` | `inspector_upload` | Upload file to device |

### WiFi Connection

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/api/wifi-connect` | `wifi_connect` | Connect to WiFi device |

**Request Body:**
```json
{
  "address": "192.168.1.100:5555"
}
```

### File Management

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/files` | `files` | File list page |
| POST | `/upload` | `store_file_handler` | Upload file |
| POST | `/upload_group/{path}` | `upload_group` | Upload to group |
| GET | `/file/delete/{group}/{filename}` | `file_delete` | Delete file |

### Heartbeat

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/heartbeat` | `heartbeat` | Device heartbeat |

### Shell

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/shell` | `shell` | Execute shell command |

### ATX Agent

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/atxagent` | `atxagent` | ATX agent download |

---

## WebSocket Endpoints

### NIO WebSocket

**Path:** `GET /nio/{udid}/ws`

Real-time device control channel.

**Message Format:**
```json
{
  "type": "<event_type>",
  "data": { ... },
  "id": "<optional_request_id>"
}
```

**Event Types:**

| Type | Direction | Description |
|------|-----------|-------------|
| `subscribe` | Client → Server | Subscribe to events |
| `unsubscribe` | Client → Server | Unsubscribe from events |
| `screenshot` | Client → Server | Single screenshot request |
| `touch` | Client → Server | Touch event |
| `swipe` | Client → Server | Swipe gesture |
| `input` | Client → Server | Text input |
| `keyevent` | Client → Server | Key event |

**Subscribe Example:**
```json
{
  "type": "subscribe",
  "target": "screenshot",
  "interval": 50
}
```

**Screenshot Stream:**
- Server sends binary JPEG frames
- Client receives `ArrayBuffer` / `Blob`

**Touch Example:**
```json
{
  "type": "touch",
  "data": {"x": 540, "y": 960}
}
```

**Swipe Example:**
```json
{
  "type": "swipe",
  "data": {"x1": 540, "y1": 1500, "x2": 540, "y2": 500, "duration": 0.3}
}
```

**Key Event Example:**
```json
{
  "type": "keyevent",
  "data": {"key": "Enter"}
}
```

**Key Mappings:**
| Key | Android Key |
|-----|-------------|
| Enter | `enter` |
| Backspace | `del` |
| Delete | `forward_del` |
| Home | `home` |
| Back | `back` |
| Tab | `tab` |
| ArrowUp | `dpad_up` |
| ArrowDown | `dpad_down` |
| ArrowLeft | `dpad_left` |
| ArrowRight | `dpad_right` |

### ADB Shell WebSocket

**Path:** `GET /devices/{udid}/shell`

Real-time ADB shell session.

### Scrcpy WebSocket

**Path:** `GET /scrcpy/{udid}/ws`

High-performance screen mirroring via scrcpy.

**Status:** `GET /scrcpy/{udid}/status`

---

## NIO Stats

**Path:** `GET /nio/stats`

**Response:**
```json
{
  "connection_pool": {
    "total": 5,
    "max_size": 1200
  },
  "sessions": 3
}
```

---

## Response Formats

### Success Response
```json
{
  "status": "ok",
  "data": { ... }
}
```

### Error Response
```json
{
  "status": "error",
  "message": "Error description"
}
```

### Screenshot Response (JSON)
```json
{
  "type": "jpeg",
  "encoding": "base64",
  "data": "/9j/4AAQSkZJRg..."
}
```
