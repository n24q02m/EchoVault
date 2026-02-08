#!/usr/bin/env bash
# EchoVault Full Install Script (Desktop App + CLI)
# Usage: curl -fsSL https://raw.githubusercontent.com/n24q02m/EchoVault/main/install.sh | bash
#
# Installs:
#   1. Desktop App (system package or AppImage)
#   2. CLI binary to ~/.local/bin (for MCP server, terminal usage)
#
# Environment variables:
#   VERSION     - Install specific version (e.g., "1.0.0"), default: latest
#   DRY_RUN     - Set to "1" to print commands without executing

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

REPO="n24q02m/EchoVault"
APP_NAME="EchoVault"
GITHUB_API="https://api.github.com/repos/${REPO}/releases"

# Helper functions
info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1" >&2; exit 1; }

run() {
    if [[ "${DRY_RUN:-0}" == "1" ]]; then
        echo -e "${YELLOW}[DRY-RUN]${NC} $*"
    else
        "$@"
    fi
}

# Show help
show_help() {
    cat << EOF
${APP_NAME} Install Script

Usage:
    curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | bash

    # Or with specific version
    curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | VERSION=1.0.0 bash

Options:
    --help      Show this help message
    --version   Show script version

Environment Variables:
    VERSION     Install specific version (default: latest)
    DRY_RUN     Set to "1" to preview commands without executing

Supported Platforms:
    - Linux (x86_64): .deb (Debian/Ubuntu), .rpm (Fedora/RHEL), .AppImage (Universal)
    - macOS (x86_64, arm64): .dmg

EOF
    exit 0
}

# Detect OS and architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)  PLATFORM="linux" ;;
        Darwin) PLATFORM="macos" ;;
        *)      error "Unsupported OS: $OS" ;;
    esac

    case "$ARCH" in
        x86_64|amd64)   ARCH="x64" ;;
        aarch64|arm64)  ARCH="arm64" ;;
        *)              error "Unsupported architecture: $ARCH" ;;
    esac

    info "Detected: $PLATFORM ($ARCH)"
}

# Detect Linux package manager
detect_linux_distro() {
    if [[ "$PLATFORM" != "linux" ]]; then
        return
    fi

    if command -v apt-get &>/dev/null; then
        PKG_MANAGER="apt"
        PKG_EXT="deb"
    elif command -v dnf &>/dev/null; then
        PKG_MANAGER="dnf"
        PKG_EXT="rpm"
    elif command -v yum &>/dev/null; then
        PKG_MANAGER="yum"
        PKG_EXT="rpm"
    else
        PKG_MANAGER="appimage"
        PKG_EXT="AppImage"
        warn "No supported package manager found, using AppImage"
    fi

    info "Package manager: $PKG_MANAGER"
}

# Get latest or specific version
get_version() {
    if [[ -n "${VERSION:-}" ]]; then
        RELEASE_VERSION="$VERSION"
        info "Using specified version: v$RELEASE_VERSION"
    else
        info "Fetching latest version..."

        # Method 1: Try GitHub API first
        local response
        response=$(curl -fsSL -H "User-Agent: EchoVault-Installer" "${GITHUB_API}/latest" 2>/dev/null) && {
            RELEASE_VERSION=$(echo "$response" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
            if [[ -n "$RELEASE_VERSION" ]]; then
                info "Latest version: v$RELEASE_VERSION"
                return
            fi
        }

        # Method 2: Fallback - parse version from redirect URL (no rate limit)
        info "API rate limited, using fallback method..."
        local redirect_url
        redirect_url=$(curl -fsSI "https://github.com/${REPO}/releases/latest" 2>/dev/null | grep -i "^location:" | tr -d '\r' | awk '{print $2}')

        if [[ -n "$redirect_url" ]]; then
            # Extract version from URL like: .../releases/tag/v1.0.0
            RELEASE_VERSION=$(echo "$redirect_url" | sed -E 's|.*/tag/v||')
        fi

        if [[ -z "$RELEASE_VERSION" ]]; then
            error "Failed to fetch latest version. Try specifying VERSION=x.x.x"
        fi

        info "Latest version: v$RELEASE_VERSION"
    fi
}

# Build download URL
build_download_url() {
    local base_url="https://github.com/${REPO}/releases/download/v${RELEASE_VERSION}"

    case "$PLATFORM" in
        linux)
            case "$PKG_EXT" in
                deb)
                    DOWNLOAD_URL="${base_url}/${APP_NAME}_${RELEASE_VERSION}_amd64.deb"
                    FILENAME="${APP_NAME}_${RELEASE_VERSION}_amd64.deb"
                    ;;
                rpm)
                    DOWNLOAD_URL="${base_url}/${APP_NAME}-${RELEASE_VERSION}-1.x86_64.rpm"
                    FILENAME="${APP_NAME}-${RELEASE_VERSION}-1.x86_64.rpm"
                    ;;
                AppImage)
                    DOWNLOAD_URL="${base_url}/${APP_NAME}_${RELEASE_VERSION}_amd64.AppImage"
                    FILENAME="${APP_NAME}_${RELEASE_VERSION}_amd64.AppImage"
                    ;;
            esac
            ;;
        macos)
            if [[ "$ARCH" == "arm64" ]]; then
                DOWNLOAD_URL="${base_url}/${APP_NAME}_${RELEASE_VERSION}_aarch64.dmg"
                FILENAME="${APP_NAME}_${RELEASE_VERSION}_aarch64.dmg"
            else
                DOWNLOAD_URL="${base_url}/${APP_NAME}_${RELEASE_VERSION}_x64.dmg"
                FILENAME="${APP_NAME}_${RELEASE_VERSION}_x64.dmg"
            fi
            ;;
    esac

    info "Download URL: $DOWNLOAD_URL"
}

# Download installer
download_installer() {
    TEMP_DIR=$(mktemp -d)
    DOWNLOAD_PATH="${TEMP_DIR}/${FILENAME}"

    info "Downloading ${APP_NAME} v${RELEASE_VERSION}..."
    run curl -fSL --progress-bar -o "$DOWNLOAD_PATH" "$DOWNLOAD_URL"

    if [[ ! -f "$DOWNLOAD_PATH" ]] && [[ "${DRY_RUN:-0}" != "1" ]]; then
        error "Download failed"
    fi

    success "Downloaded to $DOWNLOAD_PATH"
}

# Download and install CLI binary
install_cli() {
    local cli_install_dir="${HOME}/.local/bin"
    local cli_name="echovault-cli"

    # Determine CLI artifact name
    local cli_arch_suffix
    case "$ARCH" in
        x64)    cli_arch_suffix="x64" ;;
        arm64)  cli_arch_suffix="arm64" ;;
    esac

    local cli_artifact="${cli_name}-${PLATFORM}-${cli_arch_suffix}"
    local cli_url="https://github.com/${REPO}/releases/download/v${RELEASE_VERSION}/${cli_artifact}"
    local cli_dest="${cli_install_dir}/${cli_name}"

    info "Installing CLI to ${cli_dest}..."
    run mkdir -p "$cli_install_dir"

    if [[ "${DRY_RUN:-0}" == "1" ]]; then
        run curl -fSL --progress-bar -o "$cli_dest" "$cli_url"
        run chmod +x "$cli_dest"
    else
        curl -fSL --progress-bar -o "$cli_dest" "$cli_url"
        if [[ ! -f "$cli_dest" ]]; then
            warn "CLI download failed (URL: $cli_url). Desktop app installed without CLI."
            return
        fi
        chmod +x "$cli_dest"
    fi

    success "CLI installed to $cli_dest"

    # Ensure ~/.local/bin is in PATH
    if [[ ":$PATH:" != *":${cli_install_dir}:"* ]]; then
        local shell_name rc_file export_line
        shell_name="$(basename "${SHELL:-/bin/bash}")"
        case "$shell_name" in
            zsh)  rc_file="$HOME/.zshrc" ;;
            fish) rc_file="$HOME/.config/fish/config.fish" ;;
            *)    rc_file="$HOME/.bashrc" ;;
        esac

        export_line="export PATH=\"${cli_install_dir}:\$PATH\""
        [[ "$shell_name" == "fish" ]] && export_line="fish_add_path ${cli_install_dir}"

        if [[ -f "$rc_file" ]] && grep -qF "$cli_install_dir" "$rc_file" 2>/dev/null; then
            info "PATH entry already in $rc_file"
        else
            run echo "$export_line" >> "$rc_file"
            info "Added ${cli_install_dir} to PATH in $rc_file"
        fi
        warn "Run: source $rc_file  (or restart terminal) to use 'echovault-cli'"
    fi
}

# Install on Linux
install_linux() {
    info "Installing ${APP_NAME}..."

    case "$PKG_MANAGER" in
        apt)
            run sudo dpkg -i "$DOWNLOAD_PATH"
            run sudo apt-get install -f -y  # Fix dependencies if needed
            ;;
        dnf)
            run sudo dnf install -y "$DOWNLOAD_PATH"
            ;;
        yum)
            run sudo yum install -y "$DOWNLOAD_PATH"
            ;;
        appimage)
            local install_dir="${HOME}/.local/bin"
            run mkdir -p "$install_dir"
            run chmod +x "$DOWNLOAD_PATH"
            run mv "$DOWNLOAD_PATH" "${install_dir}/${APP_NAME}"

            if [[ ":$PATH:" != *":${install_dir}:"* ]]; then
                warn "Add ${install_dir} to your PATH to run ${APP_NAME} from anywhere"
            fi
            ;;
    esac

    success "${APP_NAME} installed successfully!"
}

# Install on macOS
install_macos() {
    info "Installing ${APP_NAME}..."

    # Mount DMG
    local mount_point
    mount_point=$(run hdiutil attach "$DOWNLOAD_PATH" -nobrowse -noautoopen | grep -o '/Volumes/.*' | head -n1)

    if [[ "${DRY_RUN:-0}" != "1" ]]; then
        # Copy app to /Applications
        run cp -R "${mount_point}/${APP_NAME}.app" /Applications/

        # Unmount DMG
        run hdiutil detach "$mount_point" -quiet
    else
        echo -e "${YELLOW}[DRY-RUN]${NC} cp -R <mount>/${APP_NAME}.app /Applications/"
        echo -e "${YELLOW}[DRY-RUN]${NC} hdiutil detach <mount>"
    fi

    success "${APP_NAME} installed to /Applications!"
    warn "First launch: Right-click > Open (to bypass Gatekeeper)"
}

# Cleanup
cleanup() {
    if [[ -n "${TEMP_DIR:-}" ]] && [[ -d "$TEMP_DIR" ]]; then
        rm -rf "$TEMP_DIR"
    fi
}

# Main
main() {
    # Parse arguments
    for arg in "$@"; do
        case "$arg" in
            --help|-h)    show_help ;;
            --version|-v) echo "install.sh v1.0.0"; exit 0 ;;
        esac
    done

    echo ""
    echo -e "${BLUE}╔═══════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║      ${APP_NAME} Installer             ║${NC}"
    echo -e "${BLUE}╚═══════════════════════════════════════╝${NC}"
    echo ""

    trap cleanup EXIT

    detect_platform
    detect_linux_distro
    get_version
    build_download_url
    download_installer

    case "$PLATFORM" in
        linux) install_linux ;;
        macos) install_macos ;;
    esac

    # Also install CLI binary for terminal/MCP usage
    install_cli

    echo ""
    success "Installation complete!"
    echo ""
    info "Desktop: Run 'EchoVault' from your app launcher"
    info "CLI:     Run 'echovault-cli --help' in terminal"
    info "MCP:     Run 'echovault-cli mcp' for AI assistants"
}

main "$@"
