# EchoVault

**Black Box cho moi cuoc hoi thoai AI cua ban.**

EchoVault trich xuat va dong bo lich su chat tu GitHub Copilot, Antigravity va cac cong cu AI khac - giup ban khong bao gio mat nhung insight quy gia.

## Tinh nang

- **Universal Extraction**: Ho tro VS Code Copilot, Antigravity
- **Cloud Sync via Rclone**: Dong bo voi Google Drive
- **Desktop App**: Mini window voi system tray background sync
- **Cross-Platform**: Windows, Linux, macOS
- **Future-Proof**: Luu tru raw JSON goc, khong transform/format

## Yeu cau

- **Rust**: 1.83+ (via mise hoac rustup)
- **Node.js**: 20+ (cho frontend)
- **pnpm**: Package manager

## Cai dat

```bash
# Clone repository
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault

# Cai dat Tauri dependencies (Linux)
sudo apt update && sudo apt install -y pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev

# Cai dat Tauri dependencies (Windows)
# Can Visual Studio Build Tools voi C++ workload
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# Cai dat rust, node, pnpm
mise install

# Cai dat Tauri CLI
cargo install tauri-cli

# Cai dat dependencies
pnpm install
```

## Chay ung dung

```bash
# Development mode (tu dong download Rclone binary)
pnpm dev
cargo tauri dev

# Production build
cargo tauri build
```

## Phat trien

```bash
# Cai dat pre-commit
uv venv
uv pip install pre-commit
uv run pre-commit install
uv run pre-commit run --all-files

# Rust
cargo build                # Debug build
cargo test --workspace     # Run tests
cargo clippy --workspace   # Lint
cargo fmt --all            # Format

# TypeScript (apps/web/)
cd apps/web
pnpm dev                   # Dev server
pnpm build                 # Production build
pnpm lint                  # Biome lint
pnpm format                # Biome format
```

## Cau truc du an

```text
EchoVault/
├── apps/
│   ├── core/              # Core library (extractors, sync, watcher)
│   ├── tauri/             # Tauri backend
│   │   ├── binaries/      # Rclone sidecar binaries (auto-downloaded)
│   │   └── src/           # Rust source
│   └── web/               # React frontend
└── scripts/               # Build scripts
```

## Sync Provider

EchoVault su dung **Rclone** lam sync engine de dong bo voi **Google Drive**:

- **Khong can setup OAuth phuc tap**: Rclone da co san verified credentials
- **User-friendly**: Chi can click Connect va dang nhap trong browser

## License

MIT
