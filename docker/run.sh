#!/bin/bash
# EchoVault Docker Runner Script
# Run EchoVault in Docker with X11 forwarding on unsupported Linux distributions
#
# Usage:
#   ./docker/run.sh [command]
#
# Commands:
#   run     - Run EchoVault (default)
#   build   - Build Docker image
#   shell   - Open shell in container
#   logs    - Show container logs
#   stop    - Stop container
#   clean   - Remove container and volumes

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
IMAGE_NAME="echovault"
CONTAINER_NAME="echovault"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running on Linux
check_linux() {
    if [[ "$(uname)" != "Linux" ]]; then
        log_error "This script is intended for Linux systems only."
        log_info "For other platforms, please use the native installer."
        exit 1
    fi
}

# Setup X11 access for Docker
setup_x11() {
    log_info "Setting up X11 access..."

    # Allow local connections to X server
    xhost +local:docker 2>/dev/null || true

    # Export DISPLAY if not set
    export DISPLAY="${DISPLAY:-:0}"

    log_info "DISPLAY=$DISPLAY"
}

# Build Docker image
build_image() {
    log_info "Building Docker image..."

    cd "$PROJECT_DIR"

    # Check if AppImage exists
    APPIMAGE=$(find target/release/bundle/appimage -name "*.AppImage" 2>/dev/null | head -n1)

    if [[ -z "$APPIMAGE" ]]; then
        log_error "AppImage not found. Please build the app first:"
        log_info "  cargo tauri build --target x86_64-unknown-linux-gnu"
        exit 1
    fi

    log_info "Found AppImage: $APPIMAGE"

    docker build -t "$IMAGE_NAME" .

    log_info "Docker image built successfully!"
}

# Run EchoVault in Docker
run_app() {
    check_linux
    setup_x11

    log_info "Running EchoVault in Docker..."

    cd "$PROJECT_DIR"
    docker compose up -d

    log_info "EchoVault is running!"
    log_info "To view logs: $0 logs"
    log_info "To stop: $0 stop"
}

# Open shell in container
open_shell() {
    log_info "Opening shell in container..."
    docker exec -it "$CONTAINER_NAME" /bin/bash
}

# Show container logs
show_logs() {
    docker compose logs -f
}

# Stop container
stop_app() {
    log_info "Stopping EchoVault..."
    cd "$PROJECT_DIR"
    docker compose down
    log_info "EchoVault stopped."
}

# Clean up everything
clean_all() {
    log_warn "This will remove the container and all data volumes!"
    read -p "Are you sure? (y/N) " -n 1 -r
    echo

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "Cleaning up..."
        cd "$PROJECT_DIR"
        docker compose down -v
        docker rmi "$IMAGE_NAME" 2>/dev/null || true
        log_info "Cleanup complete."
    else
        log_info "Cancelled."
    fi
}

# Show usage
show_usage() {
    echo "EchoVault Docker Runner"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  run     Run EchoVault (default)"
    echo "  build   Build Docker image"
    echo "  shell   Open shell in container"
    echo "  logs    Show container logs"
    echo "  stop    Stop container"
    echo "  clean   Remove container and volumes"
    echo ""
    echo "Examples:"
    echo "  $0 build   # Build the Docker image"
    echo "  $0 run     # Start EchoVault"
    echo "  $0 stop    # Stop EchoVault"
}

# Main entry point
case "${1:-run}" in
    run)
        run_app
        ;;
    build)
        build_image
        ;;
    shell)
        open_shell
        ;;
    logs)
        show_logs
        ;;
    stop)
        stop_app
        ;;
    clean)
        clean_all
        ;;
    help|--help|-h)
        show_usage
        ;;
    *)
        log_error "Unknown command: $1"
        show_usage
        exit 1
        ;;
esac
