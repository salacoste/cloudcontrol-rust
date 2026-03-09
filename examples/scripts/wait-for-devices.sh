#!/usr/bin/env bash
set -euo pipefail

# wait-for-devices.sh — Poll cloudcontrol-rust health endpoint until ready,
# then verify devices are connected.
#
# Usage:
#   ./wait-for-devices.sh [MIN_DEVICES]
#
# Environment:
#   CLOUDCONTROL_URL  Base URL (default: http://localhost:8000)
#   HEALTH_TIMEOUT    Max seconds to wait for health (default: 120)
#   DEVICE_TIMEOUT    Max seconds to wait for devices (default: 60)

CLOUDCONTROL_URL="${CLOUDCONTROL_URL:-http://localhost:8000}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-120}"
DEVICE_TIMEOUT="${DEVICE_TIMEOUT:-60}"
MIN_DEVICES="${1:-1}"

# ─── Health check polling ───

echo "Waiting for cloudcontrol-rust at ${CLOUDCONTROL_URL} ..."
elapsed=0
while [ "$elapsed" -lt "$HEALTH_TIMEOUT" ]; do
    status=$(curl -s -o /dev/null -w "%{http_code}" "${CLOUDCONTROL_URL}/api/v1/health" 2>/dev/null || true)
    if [ "$status" = "200" ]; then
        echo "Health check passed."
        break
    fi
    sleep 2
    elapsed=$((elapsed + 2))
done

if [ "$elapsed" -ge "$HEALTH_TIMEOUT" ]; then
    echo "ERROR: Health check timed out after ${HEALTH_TIMEOUT}s" >&2
    exit 1
fi

# ─── Device readiness polling ───

echo "Waiting for at least ${MIN_DEVICES} device(s) ..."
elapsed=0
while [ "$elapsed" -lt "$DEVICE_TIMEOUT" ]; do
    response=$(curl -sf "${CLOUDCONTROL_URL}/api/v1/devices" 2>/dev/null || true)
    if [ -n "$response" ]; then
        api_status=$(echo "$response" | jq -r '.status // empty' 2>/dev/null || true)
        if [ "$api_status" = "success" ]; then
            count=$(echo "$response" | jq '.data | length' 2>/dev/null || echo 0)
            if [ "$count" -ge "$MIN_DEVICES" ]; then
                echo "Found ${count} device(s). Ready."
                echo "$response" | jq -r '.data[].udid' 2>/dev/null
                exit 0
            fi
        fi
    fi
    sleep 3
    elapsed=$((elapsed + 3))
done

echo "ERROR: Only found ${count:-0} device(s), need ${MIN_DEVICES}" >&2
exit 1
