#!/usr/bin/env bash
set -euo pipefail

# batch-tap-test.sh — Demonstrate batch tap operations across multiple devices.
#
# Usage:
#   ./batch-tap-test.sh X Y [UDID1,UDID2,...]
#
# Environment:
#   CLOUDCONTROL_URL  Base URL (default: http://localhost:8000)

CLOUDCONTROL_URL="${CLOUDCONTROL_URL:-http://localhost:8000}"

if [ $# -lt 2 ]; then
    echo "Usage: $0 X Y [UDID1,UDID2,...]"
    echo "  X, Y    Tap coordinates"
    echo "  UDIDs   Comma-separated device list (default: all connected)"
    exit 1
fi

TAP_X="$1"
TAP_Y="$2"

# ─── Resolve device list ───

if [ $# -ge 3 ]; then
    # User-provided UDIDs
    IFS=',' read -ra UDIDS <<< "$3"
else
    # Auto-discover all connected devices
    response=$(curl -sf "${CLOUDCONTROL_URL}/api/v1/devices")
    api_status=$(echo "$response" | jq -r '.status')
    if [ "$api_status" != "success" ]; then
        echo "ERROR: Failed to list devices" >&2
        exit 1
    fi
    UDIDS=()
    while IFS= read -r line; do
        UDIDS+=("$line")
    done < <(echo "$response" | jq -r '.data[].udid')
fi

if [ ${#UDIDS[@]} -eq 0 ]; then
    echo "ERROR: No devices available" >&2
    exit 1
fi

echo "Sending tap (${TAP_X}, ${TAP_Y}) to ${#UDIDS[@]} device(s) ..."

# ─── Build JSON payload ───

udid_json=$(printf '%s\n' "${UDIDS[@]}" | jq -R . | jq -s .)
payload=$(jq -n \
    --argjson udids "$udid_json" \
    --argjson x "$TAP_X" \
    --argjson y "$TAP_Y" \
    '{udids: $udids, x: $x, y: $y}')

# ─── Execute batch tap ───

response=$(curl -sf -X POST \
    -H "Content-Type: application/json" \
    -d "$payload" \
    "${CLOUDCONTROL_URL}/api/v1/batch/tap")

api_status=$(echo "$response" | jq -r '.status')

if [ "$api_status" = "success" ]; then
    total=$(echo "$response" | jq '.data.total')
    succeeded=$(echo "$response" | jq '.data.succeeded')
    failed=$(echo "$response" | jq '.data.failed')
    echo "Batch tap complete: ${succeeded}/${total} succeeded, ${failed} failed"

    # Show per-device results
    echo "$response" | jq -r '.data.results[] | "  \(.udid): \(.status)"' 2>/dev/null
else
    echo "ERROR: Batch tap failed" >&2
    echo "$response" | jq . 2>/dev/null >&2
    exit 1
fi
