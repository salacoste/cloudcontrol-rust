# CI/CD Integration Guide

This guide explains how to integrate cloudcontrol-rust with CI/CD pipelines for automated Android device testing.

---

## Prerequisites

- cloudcontrol-rust running and accessible from your CI/CD runner
- Android devices connected to the device farm
- `curl` and `jq` available on the runner

All examples use the `CLOUDCONTROL_URL` environment variable (default: `http://localhost:8000`).

---

## Quick Start

### 1. Wait for System Health

Poll the health endpoint until the system is ready:

```bash
CLOUDCONTROL_URL="${CLOUDCONTROL_URL:-http://localhost:8000}"

# Poll until healthy (HTTP 200)
until curl -sf "${CLOUDCONTROL_URL}/api/v1/health" > /dev/null 2>&1; do
    sleep 2
done
```

The health endpoint returns HTTP 503 when unhealthy (database or connection pool issues).

### 2. Discover Devices

```bash
curl -sf "${CLOUDCONTROL_URL}/api/v1/devices" | jq .
```

Response:
```json
{
  "status": "success",
  "data": [
    {
      "udid": "abc123",
      "model": "Pixel 6",
      "status": "connected",
      "ip": "192.168.1.100",
      "port": 7912,
      "battery": 85
    }
  ],
  "timestamp": "2026-03-08T12:00:00Z"
}
```

### 3. Capture a Screenshot

```bash
UDID="abc123"
curl -sf "${CLOUDCONTROL_URL}/api/v1/devices/${UDID}/screenshot?quality=80" \
  | jq -r '.data.data' | base64 -d > screenshot.jpg
```

Query parameters:
- `?quality=1-100` — JPEG quality (default: 80)
- `?format=jpeg|png` — Image format

### 4. Execute a Tap

```bash
curl -sf -X POST \
  -H "Content-Type: application/json" \
  -d '{"x": 540, "y": 960}' \
  "${CLOUDCONTROL_URL}/api/v1/devices/${UDID}/tap"
```

---

## API Reference for CI/CD

### Health & Status

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/health` | GET | System health check (200 = healthy, 503 = unhealthy) |
| `/api/v1/status` | GET | Device farm summary (counts by status, average battery) |
| `/api/v1/metrics` | GET | Prometheus-compatible metrics |

### Device Operations

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/devices` | GET | List all connected devices |
| `/api/v1/devices/{udid}` | GET | Get single device info |
| `/api/v1/devices/{udid}/screenshot` | GET | Capture screenshot (query: `?quality=`, `?format=`) |
| `/api/v1/devices/{udid}/tap` | POST | Tap at coordinates `{"x": N, "y": N}` |
| `/api/v1/devices/{udid}/swipe` | POST | Swipe gesture `{"x1":, "y1":, "x2":, "y2":, "duration":}` |
| `/api/v1/devices/{udid}/input` | POST | Text input `{"text": "...", "clear": true/false}` |
| `/api/v1/devices/{udid}/keyevent` | POST | Key event `{"key": "home\|back\|enter\|..."}` |

### Batch Operations

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/batch/tap` | POST | Batch tap `{"udids": [...], "x": N, "y": N}` |
| `/api/v1/batch/swipe` | POST | Batch swipe |
| `/api/v1/batch/input` | POST | Batch text input |

### Documentation

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/openapi.json` | GET | OpenAPI 3.0 specification |

### WebSocket

| Endpoint | Protocol | Description |
|----------|----------|-------------|
| `/api/v1/ws/screenshot/{udid}` | WebSocket | Real-time screenshot streaming (binary JPEG frames) |
| `/api/v1/ws/nio` | WebSocket | JSON-RPC 2.0 device automation (tap, swipe, input, keyevent, batch ops, queries) |

### NIO WebSocket JSON-RPC Methods

The `/api/v1/ws/nio` endpoint accepts JSON-RPC 2.0 messages for device-agnostic automation:

| Method | Params | Description |
|--------|--------|-------------|
| `tap` | `{udid, x, y}` | Execute tap on device |
| `swipe` | `{udid, x1, y1, x2, y2, duration?}` | Execute swipe gesture |
| `input` | `{udid, text}` | Send text input |
| `keyevent` | `{udid, key}` | Send key event |
| `batchTap` | `{udids[], x, y}` | Parallel tap on multiple devices |
| `batchSwipe` | `{udids[], x1, y1, x2, y2, duration?}` | Parallel swipe |
| `batchInput` | `{udids[], text}` | Parallel text input |
| `listDevices` | none | List connected devices |
| `getDevice` | `{udid}` | Get device info |
| `screenshot` | `{udid, quality?, scale?}` | Capture base64 screenshot |
| `getStatus` | none | Device farm status summary |

**Example request:**
```json
{"jsonrpc":"2.0","method":"tap","params":{"udid":"abc123","x":100,"y":200},"id":1}
```

**Example response:**
```json
{"jsonrpc":"2.0","result":"ok","id":1}
```

**JSON-RPC Error Codes:**

| Code | Meaning | When |
|------|---------|------|
| -32700 | Parse error | Malformed JSON |
| -32600 | Invalid Request | `jsonrpc` field is not `"2.0"` |
| -32601 | Method not found | Unknown method name |
| -32602 | Invalid params | Missing required params or wrong types |
| -32603 | Internal error | Device operation failed |
| -1 | Device not found | UDID not in system |

---

## Response Format

All `/api/v1/*` endpoints return a standardized JSON format.

**Success:**
```json
{
  "status": "success",
  "data": { ... },
  "timestamp": "2026-03-08T12:00:00Z"
}
```

**Error:**
```json
{
  "status": "error",
  "error": "ERR_DEVICE_NOT_FOUND",
  "message": "Device 'abc123' not found",
  "timestamp": "2026-03-08T12:00:00Z"
}
```

### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `ERR_DEVICE_NOT_FOUND` | 404 | Device UDID not in system |
| `ERR_DEVICE_DISCONNECTED` | 503 | Device exists but connection lost |
| `ERR_INVALID_REQUEST` | 400 | Malformed request body or parameters |
| `ERR_OPERATION_FAILED` | 500 | Device operation failed |
| `ERR_NO_DEVICES_SELECTED` | 400 | Batch operation with empty device list |

---

## Health Check Polling Pattern

The recommended CI/CD startup sequence:

1. Poll `GET /api/v1/health` until HTTP 200 with `"status": "healthy"`
2. Check `GET /api/v1/status` to verify connected device count
3. Proceed only when the expected number of devices are available

```bash
# Full readiness check
check_ready() {
    local url="${CLOUDCONTROL_URL:-http://localhost:8000}"
    local min_devices="${1:-1}"
    local healthy=false

    # Wait for health
    for i in $(seq 1 60); do
        if curl -sf "${url}/api/v1/health" > /dev/null 2>&1; then
            healthy=true
            break
        fi
        sleep 2
    done

    if [ "$healthy" != "true" ]; then
        echo "Health check timed out after 120s" >&2
        return 1
    fi

    # Check device count
    count=$(curl -sf "${url}/api/v1/devices" | jq '.data | length')
    if [ "$count" -lt "$min_devices" ]; then
        echo "Need ${min_devices} devices, found ${count}" >&2
        return 1
    fi
    return 0
}
```

---

## Monitoring Integration

### Prometheus Metrics

`GET /api/v1/metrics` returns Prometheus-compatible plain text:

```
# HELP cloudcontrol_connected_devices Number of connected devices
# TYPE cloudcontrol_connected_devices gauge
cloudcontrol_connected_devices 5

# HELP cloudcontrol_websocket_connections Active WebSocket connections
# TYPE cloudcontrol_websocket_connections gauge
cloudcontrol_websocket_connections 2

# HELP cloudcontrol_screenshot_latency_seconds Screenshot capture latency
# TYPE cloudcontrol_screenshot_latency_seconds summary
cloudcontrol_screenshot_latency_seconds{quantile="0.5"} 0.15
cloudcontrol_screenshot_latency_seconds{quantile="0.95"} 0.35
cloudcontrol_screenshot_latency_seconds{quantile="0.99"} 0.48
```

Add to your Prometheus `scrape_configs`:

```yaml
scrape_configs:
  - job_name: 'cloudcontrol'
    static_configs:
      - targets: ['your-server:8000']
    metrics_path: '/api/v1/metrics'
    scrape_interval: 15s
```

---

## Example Pipelines

### GitHub Actions

See [`examples/ci-cd/github-actions/device-test.yml`](../examples/ci-cd/github-actions/device-test.yml) for a complete workflow that:
- Waits for system health
- Discovers connected devices
- Captures screenshots from all devices
- Runs a tap smoke test
- Uploads screenshots as artifacts

### Jenkins

See [`examples/ci-cd/jenkins/Jenkinsfile`](../examples/ci-cd/jenkins/Jenkinsfile) for a pipeline that:
- Checks system health with retry
- Discovers and validates device count
- Tests devices in parallel (screenshot + tap)
- Generates status and metrics reports
- Archives screenshot artifacts

### Reusable Scripts

Helper scripts in [`examples/scripts/`](../examples/scripts/):

| Script | Description |
|--------|-------------|
| `wait-for-devices.sh` | Poll health endpoint, then verify device count |
| `capture-screenshots.sh` | Capture screenshots from all connected devices |
| `batch-tap-test.sh` | Demonstrate batch tap across multiple devices |

---

## OpenAPI Specification

The full API specification is available at:

```
GET /api/v1/openapi.json
```

Use this to generate client libraries or validate your integration:

```bash
# Download OpenAPI spec
curl -sf "${CLOUDCONTROL_URL}/api/v1/openapi.json" > openapi.json

# Validate with spectral (optional)
npx @stoplight/spectral-cli lint openapi.json
```
