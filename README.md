# EchoVault

**Black box for all your AI conversations.**

EchoVault extracts and syncs chat history from GitHub Copilot, Cursor, Antigravity, and other AI tools - ensuring you never lose valuable insights.

## Features

- **Multi-source Extraction**: Supports VS Code Copilot, Cursor AI, Cline, Antigravity
- **Cloud Sync via Rclone**: Auto-sync with Google Drive
- **Desktop App**: Mini window with system tray background sync
- **Cross-platform**: Windows, Linux, macOS
- **Future-proof**: Stores raw JSON files without transformation

## Privacy Notice

> [!WARNING]
> EchoVault syncs your AI chat history to cloud storage. This data may contain:
>
> - Code snippets and file paths
> - API keys or secrets mentioned in conversations
> - Personal information

## Download & Install

### Quick Install (Recommended)

One command to download and install the latest version:

**Linux/macOS:**

```bash
curl -fsSL https://raw.githubusercontent.com/n24q02m/EchoVault/main/install.sh | bash
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/n24q02m/EchoVault/main/install.ps1 | iex
```

### Manual Download

Download installers directly from [Releases](https://github.com/n24q02m/EchoVault/releases):

| Platform                  | File                             |
| ------------------------- | -------------------------------- |
| **Windows (x64)**         | `EchoVault_x.x.x_x64-setup.exe`  |
| **macOS (Intel)**         | `EchoVault_x.x.x_x64.dmg`        |
| **macOS (Apple Silicon)** | `EchoVault_x.x.x_aarch64.dmg`    |
| **Linux (Debian/Ubuntu)** | `EchoVault_x.x.x_amd64.deb`      |
| **Linux (Fedora/RHEL)**   | `EchoVault-x.x.x-1.x86_64.rpm`   |
| **Linux (Universal)**     | `EchoVault_x.x.x_amd64.AppImage` |

#### Manual Installation Steps

**Windows:**

1. Download and run `EchoVault_x.x.x_x64-setup.exe`
2. Follow the installer prompts
3. Launch from Start Menu

**macOS:**

1. Download the `.dmg` matching your chip (Intel = `x64`, M-series = `aarch64`)
2. Open DMG and drag EchoVault to Applications
3. First launch: Right-click > Open (to bypass Gatekeeper)

**Linux:**

```bash
# Debian/Ubuntu
sudo dpkg -i EchoVault_x.x.x_amd64.deb

# Fedora/RHEL
sudo rpm -i EchoVault-x.x.x-1.x86_64.rpm

# AppImage (Universal)
chmod +x EchoVault_x.x.x_amd64.AppImage
./EchoVault_x.x.x_amd64.AppImage
```

### CLI for Unsupported OS

For older Linux distributions (e.g., Ubuntu 20.04) where the desktop app doesn't work, use the CLI:

```bash
# Build CLI from source
cargo build -p echovault-cli --release

# First time: authenticate with Google Drive
./target/release/echovault-cli auth

# Sync your AI chat history
./target/release/echovault-cli sync

# Other commands
./target/release/echovault-cli status   # Show status
./target/release/echovault-cli extract  # Extract only (no cloud sync)
```

## Quick Setup

**Prerequisites:** [mise](https://mise.jdx.dev/) only.

```bash
# Clone repository
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault

# Setup (auto-install tools + dependencies)
mise run setup
```

The setup will **AUTOMATICALLY** install:

1. **mise tools** - Rust, Node.js, pnpm, uv
2. **Node dependencies** - All required packages
3. **Cargo build** - Rust compilation
4. **Rclone binary** - Sync engine for Google Drive
5. **Pre-commit hooks** - Automatic quality checks

> **Note for Linux**: Install [Tauri dependencies](#linux-tauri-dependencies) first.

## Running the App

```bash
# Development mode (full app)
cargo tauri dev

# Development mode (web only)
pnpm dev

# Production build
cargo tauri build

# Reset app (delete config to re-setup)
pnpm reset
pnpm reset --all
```

## Development

### Rust

```bash
cargo build                # Debug build
cargo test --workspace     # Run tests
cargo clippy --workspace   # Lint
cargo fmt --all            # Format code
```

### TypeScript (Frontend)

```bash
cd apps/web
pnpm dev                   # Dev server with HMR
pnpm build                 # Production build
pnpm lint                  # Biome lint
pnpm format                # Biome format
```

### Pre-commit hooks

Pre-commit hooks are automatically installed via setup script. To run manually:

```bash
uv run pre-commit run --all-files
```

## Sync Provider

EchoVault uses **Rclone** as sync engine to sync with **Google Drive**:

- **No complex OAuth setup**: Rclone comes with verified credentials
- **User-friendly**: Just click Connect and login in browser
- **Reliable**: Rclone is a widely-used sync tool with 40k+ stars on GitHub

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

MIT - See [LICENSE](LICENSE) for details.
