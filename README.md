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
>
> **Please review your chat history for sensitive data before enabling sync.**
> See [SECURITY.md](SECURITY.md) for details.

## Download

Download the latest installer from [Releases](https://github.com/n24q02m/EchoVault/releases):

| Platform              | File                                  | Description                |
| --------------------- | ------------------------------------- | -------------------------- |
| **Windows**           | `EchoVault_x.x.x_x64-setup.exe`       | NSIS installer (recommended) |
|                       | `EchoVault_x.x.x_x64_en-US.msi`       | MSI installer              |
| **macOS (Intel)**     | `EchoVault_x.x.x_x64.dmg`             | DMG installer              |
|                       | `EchoVault_x64.app.tar.gz`            | App bundle                 |
| **macOS (Apple Silicon)** | `EchoVault_x.x.x_aarch64.dmg`     | DMG installer              |
|                       | `EchoVault_aarch64.app.tar.gz`        | App bundle                 |
| **Linux (Debian/Ubuntu)** | `EchoVault_x.x.x_amd64.deb`       | DEB package                |
| **Linux (Fedora/RHEL)**   | `EchoVault-x.x.x-1.x86_64.rpm`    | RPM package                |
| **Linux (Universal)** | `EchoVault_x.x.x_amd64.AppImage`      | Portable AppImage          |

### Installation

**Windows:**

1. Download `EchoVault_x.x.x_x64-setup.exe` (recommended) or `.msi`
2. Run the installer and follow the prompts
3. Launch from Start Menu or Desktop shortcut

**macOS:**

1. Download the `.dmg` file matching your chip:
   - Intel Mac: `EchoVault_x.x.x_x64.dmg`
   - Apple Silicon (M1/M2/M3): `EchoVault_x.x.x_aarch64.dmg`
2. Open the DMG and drag EchoVault to Applications
3. First launch: Right-click > Open (to bypass Gatekeeper)

**Linux (Debian/Ubuntu):**

```bash
sudo dpkg -i EchoVault_x.x.x_amd64.deb
```

**Linux (Fedora/RHEL):**

```bash
sudo rpm -i EchoVault-x.x.x-1.x86_64.rpm
```

**Linux (AppImage):**

```bash
chmod +x EchoVault_x.x.x_amd64.AppImage
./EchoVault_x.x.x_amd64.AppImage
```

## Data Storage

EchoVault stores data in standard OS locations:

| Type   | Windows                            | macOS                                      | Linux                          |
| ------ | ---------------------------------- | ------------------------------------------ | ------------------------------ |
| Config | `%APPDATA%\echovault\`             | `~/Library/Application Support/echovault/` | `~/.config/echovault/`         |
| Data   | `%LOCALAPPDATA%\echovault\vault\`  | `~/Library/Application Support/echovault/` | `~/.local/share/echovault/`    |

> [!NOTE]
> Uninstalling the app does NOT remove your config and data. This is intentional so you can reinstall without losing your chat history.

### Complete Removal

To fully remove EchoVault including all data:

**Windows:**

1. Uninstall via Settings > Apps
2. Delete `%APPDATA%\echovault\` and `%LOCALAPPDATA%\echovault\`

**macOS:**

1. Delete EchoVault from Applications
2. Delete `~/Library/Application Support/echovault/`

**Linux:**

```bash
# Uninstall (choose based on your package manager)
sudo dpkg -r echo-vault        # Debian/Ubuntu
sudo rpm -e echo-vault         # Fedora/RHEL

# Remove data
rm -rf ~/.config/echovault ~/.local/share/echovault
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

## Project Structure

```text
EchoVault/
├── apps/
│   ├── core/              # Core library (extractors, sync, watcher)
│   │   ├── extractors/    # Chat extractors for platforms
│   │   ├── storage/       # Storage layer
│   │   ├── sync/          # Rclone sync engine
│   │   └── utils/         # Utilities
│   ├── tauri/             # Tauri backend
│   │   ├── binaries/      # Rclone sidecar binaries (auto-downloaded)
│   │   ├── icons/         # App icons
│   │   └── src/           # Rust commands
│   └── web/               # React frontend
│       └── src/           # React components
└── scripts/               # Development scripts
    ├── setup-dev.mjs      # One-command setup script
    └── download-rclone.mjs # Download Rclone binary
```

## Sync Provider

EchoVault uses **Rclone** as sync engine to sync with **Google Drive**:

- **No complex OAuth setup**: Rclone comes with verified credentials
- **User-friendly**: Just click Connect and login in browser
- **Reliable**: Rclone is a widely-used sync tool with 40k+ stars on GitHub

## Tech Stack

- **Backend**: Rust (Tauri, tokio, serde)
- **Frontend**: React + TypeScript (Vite, TailwindCSS)
- **Sync**: Rclone (Google Drive)
- **Extractors**: VS Code SQLite, Cursor, Cline, Antigravity
- **Build Tools**: Cargo, pnpm, mise
- **Dev Tools**: uv, pre-commit, biome

## Troubleshooting

### Setup script fails

If setup script fails, try:

1. **Restart terminal** and run `pnpm setup` again
2. Check detailed log in output
3. Manually install missing components (see below)

### Manual installation (if setup script fails)

#### Linux: Tauri dependencies

```bash
# Ubuntu/Debian
sudo apt update && sudo apt install -y \
  pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev

# Fedora
sudo dnf install -y pkg-config gtk3-devel webkit2gtk4.1-devel \
  libayatana-appindicator-gtk3-devel librsvg2-devel

# Arch
sudo pacman -S --noconfirm pkg-config gtk3 webkit2gtk-4.1 \
  libayatana-appindicator librsvg
```

#### macOS: Homebrew

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

#### Windows: Visual Studio Build Tools

```bash
winget install Microsoft.VisualStudio.2022.BuildTools \
  --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

#### mise

```bash
curl https://mise.run | sh
```

#### Tools from mise

```bash
mise install
```

### "command not found" error after installation

Restart terminal to load new PATH, or:

```bash
# Linux/macOS
source ~/.bashrc  # or ~/.zshrc

# Windows
# Close and reopen terminal
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

MIT - See [LICENSE](LICENSE) for details.
