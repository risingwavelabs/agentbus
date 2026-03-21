#!/bin/bash
#
# Stream0 Demo: one primary agent automatically coordinates two workers
#
# Usage:
#   ./demo.sh                     # starts a local server and runs the demo
#   ./demo.sh http://yourserver   # runs against an existing server
#
set -euo pipefail

URL="${1:-}"
STARTED_SERVER=false
DB_PATH="/tmp/stream0-demo.db"

cleanup() {
    if [ "$STARTED_SERVER" = true ]; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
}

trap cleanup EXIT

if [ -z "$URL" ]; then
    echo "=== Building Stream0..."
    cargo build --release >/dev/null

    echo "=== Starting server on http://localhost:8080..."
    STREAM0_DB_PATH="$DB_PATH" ./target/release/stream0 >/dev/null 2>&1 &
    SERVER_PID=$!
    STARTED_SERVER=true
    URL="http://localhost:8080"
    sleep 1

    if ! curl -sf "$URL/health" >/dev/null 2>&1; then
        echo "ERROR: Server failed to start"
        exit 1
    fi
fi

echo "=== Running auto-coordination demo..."
STREAM0_URL="$URL" python3 examples/auto_coordination_demo.py
