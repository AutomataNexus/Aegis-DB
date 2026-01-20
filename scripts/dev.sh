#!/bin/bash
# Aegis DB Development Script
# Manages both the API server and the dashboard

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
DASHBOARD_DIR="$ROOT_DIR/crates/aegis-dashboard"

SERVER_PID_FILE="/tmp/aegis-server.pid"
DASHBOARD_PID_FILE="/tmp/aegis-dashboard.pid"

SERVER_PORT=9090
DASHBOARD_PORT=8000

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

start_server() {
    if [ -f "$SERVER_PID_FILE" ] && kill -0 "$(cat "$SERVER_PID_FILE")" 2>/dev/null; then
        log_warn "Server already running (PID: $(cat "$SERVER_PID_FILE"))"
        return 0
    fi

    log_info "Starting Aegis Server on port $SERVER_PORT..."

    # Check if binary exists, build if not
    SERVER_BIN="$ROOT_DIR/target/release/aegis-server"
    if [ ! -f "$SERVER_BIN" ]; then
        log_info "Building server binary..."
        cd "$ROOT_DIR"
        cargo build -p aegis-server --release
    fi

    "$SERVER_BIN" --port "$SERVER_PORT" > /tmp/aegis-server.log 2>&1 &
    echo $! > "$SERVER_PID_FILE"

    # Wait for server to be ready
    for i in {1..30}; do
        if curl -s "http://127.0.0.1:$SERVER_PORT/health" > /dev/null 2>&1; then
            log_success "Server running at http://127.0.0.1:$SERVER_PORT (PID: $(cat "$SERVER_PID_FILE"))"
            return 0
        fi
        sleep 1
    done

    log_error "Server failed to start. Check /tmp/aegis-server.log"
    return 1
}

stop_server() {
    if [ -f "$SERVER_PID_FILE" ]; then
        PID=$(cat "$SERVER_PID_FILE")
        if kill -0 "$PID" 2>/dev/null; then
            log_info "Stopping server (PID: $PID)..."
            kill "$PID" 2>/dev/null || true
            rm -f "$SERVER_PID_FILE"
            log_success "Server stopped"
        else
            log_warn "Server not running"
            rm -f "$SERVER_PID_FILE"
        fi
    else
        log_warn "No server PID file found"
    fi
}

start_dashboard() {
    if [ -f "$DASHBOARD_PID_FILE" ] && kill -0 "$(cat "$DASHBOARD_PID_FILE")" 2>/dev/null; then
        log_warn "Dashboard already running (PID: $(cat "$DASHBOARD_PID_FILE"))"
        return 0
    fi

    log_info "Starting Dashboard on port $DASHBOARD_PORT..."
    cd "$DASHBOARD_DIR"
    trunk serve --release > /tmp/aegis-dashboard.log 2>&1 &
    echo $! > "$DASHBOARD_PID_FILE"

    # Wait for dashboard to be ready
    for i in {1..30}; do
        if curl -s "http://127.0.0.1:$DASHBOARD_PORT" > /dev/null 2>&1; then
            log_success "Dashboard running at http://127.0.0.1:$DASHBOARD_PORT (PID: $(cat "$DASHBOARD_PID_FILE"))"
            return 0
        fi
        sleep 1
    done

    log_error "Dashboard failed to start. Check /tmp/aegis-dashboard.log"
    return 1
}

stop_dashboard() {
    if [ -f "$DASHBOARD_PID_FILE" ]; then
        PID=$(cat "$DASHBOARD_PID_FILE")
        if kill -0 "$PID" 2>/dev/null; then
            log_info "Stopping dashboard (PID: $PID)..."
            kill "$PID" 2>/dev/null || true
            rm -f "$DASHBOARD_PID_FILE"
            log_success "Dashboard stopped"
        else
            log_warn "Dashboard not running"
            rm -f "$DASHBOARD_PID_FILE"
        fi
    else
        log_warn "No dashboard PID file found"
    fi
}

status() {
    echo ""
    echo "=== Aegis DB Status ==="
    echo ""

    if [ -f "$SERVER_PID_FILE" ] && kill -0 "$(cat "$SERVER_PID_FILE")" 2>/dev/null; then
        log_success "Server:    RUNNING (PID: $(cat "$SERVER_PID_FILE")) - http://127.0.0.1:$SERVER_PORT"
    else
        log_error "Server:    STOPPED"
    fi

    if [ -f "$DASHBOARD_PID_FILE" ] && kill -0 "$(cat "$DASHBOARD_PID_FILE")" 2>/dev/null; then
        log_success "Dashboard: RUNNING (PID: $(cat "$DASHBOARD_PID_FILE")) - http://127.0.0.1:$DASHBOARD_PORT"
    else
        log_error "Dashboard: STOPPED"
    fi
    echo ""
}

build() {
    log_info "Building server..."
    cd "$ROOT_DIR"
    cargo build -p aegis-server --release

    log_info "Building dashboard..."
    cd "$DASHBOARD_DIR"
    trunk build --release

    log_success "Build complete!"
}

case "$1" in
    start)
        echo ""
        echo "=== Starting Aegis DB ==="
        echo ""
        start_server
        start_dashboard
        echo ""
        log_success "Aegis DB is ready!"
        echo ""
        echo "  Server:    http://127.0.0.1:$SERVER_PORT"
        echo "  Dashboard: http://127.0.0.1:$DASHBOARD_PORT"
        echo ""
        echo "  Login with: demo / demo"
        echo ""
        ;;
    stop)
        echo ""
        echo "=== Stopping Aegis DB ==="
        echo ""
        stop_dashboard
        stop_server
        echo ""
        ;;
    restart)
        $0 stop
        sleep 2
        $0 start
        ;;
    server)
        case "$2" in
            start) start_server ;;
            stop) stop_server ;;
            restart) stop_server; sleep 1; start_server ;;
            *) log_error "Usage: $0 server {start|stop|restart}" ;;
        esac
        ;;
    dashboard)
        case "$2" in
            start) start_dashboard ;;
            stop) stop_dashboard ;;
            restart) stop_dashboard; sleep 1; start_dashboard ;;
            *) log_error "Usage: $0 dashboard {start|stop|restart}" ;;
        esac
        ;;
    status)
        status
        ;;
    build)
        build
        ;;
    logs)
        case "$2" in
            server) tail -f /tmp/aegis-server.log ;;
            dashboard) tail -f /tmp/aegis-dashboard.log ;;
            *)
                echo "=== Server Logs (last 20 lines) ==="
                tail -20 /tmp/aegis-server.log 2>/dev/null || echo "No server logs"
                echo ""
                echo "=== Dashboard Logs (last 20 lines) ==="
                tail -20 /tmp/aegis-dashboard.log 2>/dev/null || echo "No dashboard logs"
                ;;
        esac
        ;;
    *)
        echo ""
        echo "Aegis DB Development Script"
        echo ""
        echo "Usage: $0 <command>"
        echo ""
        echo "Commands:"
        echo "  start              Start both server and dashboard"
        echo "  stop               Stop both server and dashboard"
        echo "  restart            Restart both"
        echo "  status             Show status of services"
        echo "  build              Build both server and dashboard"
        echo ""
        echo "  server start       Start only the server"
        echo "  server stop        Stop only the server"
        echo "  server restart     Restart only the server"
        echo ""
        echo "  dashboard start    Start only the dashboard"
        echo "  dashboard stop     Stop only the dashboard"
        echo "  dashboard restart  Restart only the dashboard"
        echo ""
        echo "  logs               Show recent logs from both"
        echo "  logs server        Follow server logs"
        echo "  logs dashboard     Follow dashboard logs"
        echo ""
        exit 1
        ;;
esac
