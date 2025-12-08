# EchoVault

**Black Box cho mọi cuộc hội thoại AI của bạn.**

EchoVault trích xuất, mã hóa và đồng bộ lịch sử chat từ GitHub Copilot, Cursor, Antigravity và các công cụ AI khác - giúp bạn không bao giờ mất những insight quý giá.

## Tính năng

- **Universal Extraction**: Hỗ trợ VS Code Copilot, Antigravity, Cursor, Cline
- **Privacy First**: Mã hóa AES-256-GCM trước khi rời khỏi máy
- **Multi-Provider Sync**: GitHub, Google Drive, S3 (sắp ra mắt)
- **Desktop App**: Mini window giống Google Drive Desktop
- **Future-Proof**: Lưu trữ raw JSON gốc, không transform/format

## Cài đặt

```bash
# Clone
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault

# Cài đặt Tauri dependencies (Linux)
sudo apt update && sudo apt install -y pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
cargo install tauri-cli

# Cài đặt pre-commit hooks
pre-commit install

# Cài đặt frontend
cd src-web && pnpm install && cd ..

# Run app
cargo build --release -p echovault-tauri
./target/release/echovault-tauri
```

## Phát triển

```bash
# Rust
cargo build              # Debug build
cargo test --workspace   # Run tests
cargo clippy             # Lint
cargo fmt                # Format

# TypeScript (src-web/)
pnpm dev                 # Dev server
pnpm build               # Production build
pnpm lint                # ESLint
pnpm format              # Prettier

# Tauri App
cargo tauri dev          # Development mode
cargo tauri build        # Production build
```

## Tài liệu

Xem [HANDBOOK](docs/HANDBOOK.md) để biết chi tiết về kiến trúc, bảo mật và roadmap.

## License

MIT
