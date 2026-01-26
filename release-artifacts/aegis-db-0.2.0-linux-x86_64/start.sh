#!/bin/bash
# Aegis DB Startup Script

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${AEGIS_DATA_DIR:-$SCRIPT_DIR/data}"

# Ensure data directory exists
mkdir -p "$DATA_DIR"

echo "Starting Aegis DB Server..."
echo "Data directory: $DATA_DIR"
echo "Dashboard available at: http://localhost:8000 (serve dashboard/ with any HTTP server)"
echo ""

# Start the server
exec "$SCRIPT_DIR/bin/aegis-server" \
    --host "${AEGIS_HOST:-127.0.0.1}" \
    --port "${AEGIS_PORT:-9090}" \
    --data-dir "$DATA_DIR"
