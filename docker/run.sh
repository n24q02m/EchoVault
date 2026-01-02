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
#   reset   - Reset app config
#   clean   - Remove container and volumes
#
# Note on running with sudo:
#   When running docker with sudo, $HOME may resolve to /root instead of your home.
#   Option 1: Export RCLONE_CONFIG_DIR before running:
#     RCLONE_CONFIG_DIR=$HOME/.config/rclone sudo -E docker compose up -d
#   Option 2 (recommended): Add your user to the docker group:
#     sudo usermod -aG docker $USER
#     # Then log out and log back in

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

    # Multi-stage Dockerfile will build the app inside the container
    # Use --no-cache if FORCE_REBUILD is set
    if [[ "${FORCE_REBUILD:-}" == "1" ]]; then
        log_info "Force rebuilding without cache..."
        docker build --no-cache -t "$IMAGE_NAME" .
    else
        docker build -t "$IMAGE_NAME" .
    fi

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

# Setup rclone for Google Drive sync
setup_rclone() {
    log_info "Setting up rclone for Google Drive sync..."

    # Check if rclone is installed
    if ! command -v rclone &> /dev/null; then
        log_info "Installing rclone..."
        curl https://rclone.org/install.sh | sudo bash
    else
        log_info "rclone is already installed: $(rclone version | head -n1)"
    fi

    # Check if gdrive remote already exists and has valid token
    if rclone listremotes | grep -q "^gdrive:$"; then
        # Try to list remote to verify it's working
        if rclone lsd gdrive: --max-depth 0 &> /dev/null; then
            log_info "Google Drive remote 'gdrive' is already configured and working."
            read -p "Do you want to reconfigure? (y/N) " -n 1 -r
            echo
            if [[ ! $REPLY =~ ^[Yy]$ ]]; then
                log_info "Skipping rclone configuration."
                return
            fi
            # Delete existing remote before reconfiguring
            rclone config delete gdrive
        else
            log_warn "Remote 'gdrive' exists but is not working. Reconfiguring..."
            rclone config delete gdrive
        fi
    fi

    log_info "Configuring Google Drive remote..."
    log_info "A browser will open for you to login to Google Drive."
    log_info "After login, return here to continue."
    echo ""

    # Use rclone authorize to get token interactively
    # This opens browser and waits for authentication
    rclone config create gdrive drive config_is_local=true

    # Verify configuration worked
    if rclone listremotes | grep -q "^gdrive:$"; then
        if rclone lsd gdrive: --max-depth 0 &> /dev/null; then
            log_info "Rclone setup complete!"
            log_info "Your rclone config is saved at: ~/.config/rclone/rclone.conf"
        else
            log_error "Remote created but authentication may have failed."
            log_info "Try running: rclone config reconnect gdrive:"
        fi
    else
        log_error "Failed to create remote. Please run 'rclone config' manually."
    fi
}

# Show usage
show_usage() {
    echo "EchoVault Docker Runner"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  setup   Install rclone and configure Google Drive (run this first!)"
    echo "  build   Build Docker image"
    echo "  run     Run EchoVault (default)"
    echo "  stop    Stop container"
    echo "  logs    Show container logs"
    echo "  shell   Open shell in container"
    echo "  reset   Reset app config (re-run first time setup)"
    echo "  clean   Remove container and volumes"
    echo ""
    echo "First time setup:"
    echo "  $0 setup   # Install rclone and login to Google Drive"
    echo "  $0 build   # Build the Docker image (takes ~10 minutes)"
    echo "  $0 run     # Start EchoVault"
    echo ""
    echo "Daily usage:"
    echo "  $0 run     # Start EchoVault"
    echo "  $0 stop    # Stop EchoVault"
    echo ""
    echo "Note on running with sudo:"
    echo "  When using sudo, \$HOME may resolve to /root instead of your home."
    echo "  Option 1: Export RCLONE_CONFIG_DIR:"
    echo "    RCLONE_CONFIG_DIR=\$HOME/.config/rclone sudo -E docker compose up -d"
    echo "  Option 2 (recommended): Add user to docker group:"
    echo "    sudo usermod -aG docker \$USER"
    echo "    # Then log out and log back in"
}

# Reset app config (to re-run first time setup)
reset_app() {
    log_info "Resetting EchoVault app config..."

    # Stop container if running
    cd "$PROJECT_DIR"
    docker compose down 2>/dev/null || true

    # Remove config volume (keeps vault data)
    docker volume rm echovault-config 2>/dev/null || true

    log_info "Config reset complete."
    log_info "Run '$0 run' to start fresh setup."

    if [[ "${1:-}" == "--all" ]] || [[ "${1:-}" == "-a" ]]; then
        log_warn "Removing vault data as well..."
        docker volume rm echovault-data 2>/dev/null || true
        log_info "All data removed."
    else
        log_info "Note: Vault data still preserved. Use '$0 reset --all' to remove everything."
    fi
}

# Main entry point
case "${1:-run}" in
    setup)
        setup_rclone
        ;;
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
    reset)
        reset_app "${2:-}"
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
