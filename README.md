# EchoVault

**Black Box cho mọi cuộc hội thoại AI của bạn.**

EchoVault trích xuất, mã hóa và đồng bộ lịch sử chat từ GitHub Copilot, Cursor, Antigravity và các công cụ AI khác - giúp bạn không bao giờ mất những insight quý giá.

## Tính năng

- **Universal Extraction**: Hỗ trợ VS Code Copilot, Antigravity, Cursor, Cline
- **Privacy First**: Mã hóa AES-256-GCM trước khi rời khỏi máy
- **Cloud Sync via Rclone**: Hỗ trợ Google Drive, Dropbox, OneDrive, S3 và 40+ services khác
- **Desktop App**: Mini window với system tray background sync
- **Future-Proof**: Lưu trữ raw JSON gốc, không transform/format

## Yêu cầu

- **Rust**: 1.83+ (via mise hoặc rustup)
- **Node.js**: 20+ (cho frontend)
- **pnpm**: Package manager
- **Rclone**: Sẽ được bundle vào app (hoặc cài đặt từ [rclone.org](https://rclone.org/downloads/))

## Cài đặt

```bash
# Clone repository
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault

# Cài đặt Tauri dependencies (Linux)
sudo apt update && sudo apt install -y pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev

# Cài đặt Tauri dependencies (Windows)
# Cần Visual Studio Build Tools với C++ workload
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# Cài đặt rust, node, pnpm, uv
mise install

# Cài đặt Tauri CLI
cargo install tauri-cli

# Cài đặt frontend dependencies
cd src-web && pnpm install && cd ..

# Download Rclone binaries (xem src-tauri/binaries/README.md)
```

## Chạy ứng dụng

```bash
# Development mode
cargo tauri dev

# Production build
cargo tauri build
```

## Phát triển

```bash
# Cài đặt pre-commit
uv venv
uv pip install pre-commit
uv run pre-commit install
uv run pre-commit run --all-file

# Rust
cargo build                # Debug build
cargo test --workspace     # Run tests
cargo clippy --workspace   # Lint
cargo fmt --all            # Format

# TypeScript (src-web/)
cd src-web
pnpm dev                   # Dev server
pnpm build                 # Production build
pnpm lint                  # Biome lint
pnpm format                # Biome format
```

## Cấu trúc dự án

```text
EchoVault/
├── crates/
│   └── echovault-core/    # Core library (extractors, encryption, sync)
├── src-tauri/             # Tauri backend
│   ├── binaries/          # Rclone sidecar binaries
│   └── src/               # Rust source
├── src-web/               # React frontend
└── docs/                  # Documentation
```

## Sync Providers

EchoVault sử dụng **Rclone** làm sync engine chính, mang lại lợi ích:

- **Không cần setup OAuth phức tạp**: Rclone đã có sẵn verified credentials cho các cloud services
- **Hỗ trợ 40+ providers**: Google Drive, Dropbox, OneDrive, S3, B2, và nhiều hơn nữa
- **User-friendly**: Chỉ cần click Connect và đăng nhập trong browser

## Tài liệu

Xem [HANDBOOK](docs/HANDBOOK.md) để biết chi tiết về kiến trúc, bảo mật và roadmap.

## License

MIT
