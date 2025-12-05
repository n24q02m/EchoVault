# EchoVault

**Black Box cho mọi cuộc hội thoại AI của bạn.**

EchoVault trích xuất, mã hóa và đồng bộ lịch sử chat từ GitHub Copilot, Cursor, Cline và các công cụ AI khác lên GitHub - giúp bạn không bao giờ mất những insight quý giá.

## Tính năng

- **Universal Extraction**: Hỗ trợ VS Code Copilot, Cursor, Cline (sắp ra mắt)
- **Privacy First**: Mã hóa AES-256-GCM trước khi rời khỏi máy
- **Git-Native**: Đồng bộ qua GitHub với OAuth Device Flow
- **Future-Proof**: Lưu trữ raw JSON gốc, không transform/format
- **Auto-Setup**: Tự động thiết lập khi chạy lần đầu

## Cài đặt

```bash
# Clone và build
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault
cargo build --release

# Cài đặt vào ~/.cargo/bin (khuyến nghị)
cargo install --path .
```

## Sử dụng

```bash
# Quét chat sessions có sẵn
echovault scan

# Extract, mã hóa và đồng bộ lên GitHub (tự động setup nếu lần đầu)
echovault sync

# Hoặc chỉ định remote URL trực tiếp
echovault sync --remote https://github.com/username/my-vault.git
```

### Workflow đầy đủ

1. **Lần đầu chạy `sync`**:
   - Nhập GitHub remote URL
   - Xác thực OAuth qua browser (github.com/login/device)
   - Tạo passphrase mã hóa
   - Tự động tạo repository nếu chưa tồn tại

2. **Các lần sau**:
   - Chỉ cần nhập passphrase
   - Tự động extract, encrypt và push

### GitHub OAuth Authentication

EchoVault sử dụng OAuth Device Flow - không cần copy/paste token:

1. Chạy `echovault sync`
2. Mở browser theo hướng dẫn: <https://github.com/login/device>
3. Nhập code hiển thị trên terminal
4. Authorize ứng dụng EchoVault

Token được lưu trong vault và tự động sử dụng cho các lần sync tiếp theo.

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
