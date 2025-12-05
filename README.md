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
# Clone
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault

# Tải dependencies
sudo apt update && sudo apt upgrade -y
mise trust && mise install

# Build
mise run release

# Cài đặt
mise run install
```

## Sử dụng

```bash
# Quét chat sessions có sẵn
ev scan

# Extract, mã hóa và đồng bộ lên GitHub (tự động setup nếu lần đầu)
ev sync

# Hoặc chỉ định remote URL trực tiếp
ev sync --remote https://github.com/username/my-vault.git
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

1. Chạy `ev sync`
2. Mở browser theo hướng dẫn: <https://github.com/login/device>
3. Nhập code hiển thị trên terminal
4. Authorize ứng dụng EchoVault

Token được lưu trong vault và tự động sử dụng cho các lần sync tiếp theo.

## Yêu cầu

- Rust 1.80+
- Git

## Phát triển

Sử dụng [mise](https://mise.jdx.dev/) (khuyến nghị):

```bash
# Setup
mise install

# Build & Test
mise run install      # Install ev to ~/.cargo/bin
mise run build        # Debug build
mise run release      # Release build
mise run test         # Run tests
mise run lint         # Run clippy
mise run fmt          # Format code
mise run ci           # Run all checks

# Development
mise run dev scan
mise run dev sync
```

## Tài liệu

Xem [HANDBOOK](docs/HANDBOOK.md) để biết chi tiết về kiến trúc, bảo mật và roadmap.

## License

MIT
