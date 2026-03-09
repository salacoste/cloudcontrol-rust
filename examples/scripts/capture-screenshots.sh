#!/usr/bin/env bash
set -euo pipefail

# capture-screenshots.sh — Capture screenshots from all connected devices.
#
# Usage:
#   ./capture-screenshots.sh [OUTPUT_DIR]
#
# Environment:
#   CLOUDCONTROL_URL  Base URL (default: http://localhost:8000)
#   SCREENSHOT_QUALITY  JPEG quality 1-100 (default: 80)

CLOUDCONTROL_URL="${CLOUDCONTROL_URL:-http://localhost:8000}"
SCREENSHOT_QUALITY="${SCREENSHOT_QUALITY:-80}"
OUTPUT_DIR="${1:-./screenshots}"

mkdir -p "$OUTPUT_DIR"

# ─── Get device list ───

response=$(curl -sf "${CLOUDCONTROL_URL}/api/v1/devices")
api_status=$(echo "$response" | jq -r '.status')

if [ "$api_status" != "success" ]; then
    echo "ERROR: Failed to list devices" >&2
    echo "$response" | jq . 2>/dev/null >&2
    exit 1
fi

device_count=$(echo "$response" | jq '.data | length')
echo "Found ${device_count} device(s)"

if [ "$device_count" -eq 0 ]; then
    echo "No devices connected. Nothing to capture."
    exit 0
fi

# ─── Capture screenshots ───

failed=0
succeeded=0

while read -r udid; do
    echo "Capturing screenshot from ${udid} ..."
    screenshot_response=$(curl -sf "${CLOUDCONTROL_URL}/api/v1/devices/${udid}/screenshot?quality=${SCREENSHOT_QUALITY}" 2>/dev/null || true)

    if [ -z "$screenshot_response" ]; then
        echo "  WARN: No response for ${udid}" >&2
        failed=$((failed + 1))
        continue
    fi

    shot_status=$(echo "$screenshot_response" | jq -r '.status' 2>/dev/null || true)
    if [ "$shot_status" != "success" ]; then
        error_code=$(echo "$screenshot_response" | jq -r '.error // "unknown"' 2>/dev/null)
        echo "  WARN: Failed for ${udid}: ${error_code}" >&2
        failed=$((failed + 1))
        continue
    fi

    # Decode base64 screenshot to file
    timestamp=$(date +%Y%m%d_%H%M%S)
    outfile="${OUTPUT_DIR}/${udid}_${timestamp}.jpg"
    echo "$screenshot_response" | jq -r '.data.data' | base64 -d > "$outfile"
    echo "  Saved: ${outfile}"
    succeeded=$((succeeded + 1))
done < <(echo "$response" | jq -r '.data[].udid')

echo "Done. ${succeeded} captured, ${failed} failed."
