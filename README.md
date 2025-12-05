# EchoVault

**Black Box cho mọi cuộc hội thoại AI của bạn.**

EchoVault trích xuất, mã hóa và đồng bộ lịch sử chat từ GitHub Copilot, Cursor, Cline và các công cụ AI khác lên GitHub - giúp bạn không bao giờ mất những insight quý giá.

## Tính năng

- **Universal Extraction**: Hỗ trợ VS Code Copilot, Cursor, Cline (sắp ra mắt)
- **Privacy First**: Mã hóa AES-256-GCM trước khi rời khỏi máy
- **Git-Native**: Đồng bộ qua GitHub với OAuth Device Flow
- **Future-Proof**: Lưu trữ raw JSON gốc, không transform/format

## Cài đặt

```bash
# Clone repository
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault

# Build
cargo build --release

# Binary sẽ ở target/release/echovault
```

## Sử dụng

```bash
# Khởi tạo vault với GitHub remote
echovault init --remote https://github.com/username/my-vault.git

# Quét chat sessions có sẵn
echovault scan

# Trích xuất vào vault (copy raw JSON)
echovault extract

# Mã hóa và đồng bộ lên GitHub
echovault sync
```

## Yêu cầu

- Rust 1.80+
- Git
- OpenSSL development libraries (`libssl-dev` trên Ubuntu/Debian)

## Phát triển

```bash
# Format code
cargo fmt

# Lint
cargo clippy

# Type check
cargo check

# Run tests
cargo test

# Build release
cargo build --release
```

## Tài liệu

Xem [HANDBOOK](docs/HANDBOOK.md) để biết chi tiết về kiến trúc, bảo mật và roadmap.

## License

MIT
