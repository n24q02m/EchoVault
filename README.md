# EchoVault

**Hộp đen cho mọi cuộc hội thoại AI của bạn.**

EchoVault trích xuất và đồng bộ lịch sử chat từ GitHub Copilot, Cursor, Antigravity và các công cụ AI khác - giúp bạn không bao giờ mất những insight quý giá.

## Tính năng

- **Trích xuất đa nguồn**: Hỗ trợ VS Code Copilot, Cursor AI, Cline, Antigravity
- **Đồng bộ Cloud qua Rclone**: Tự động sync với Google Drive
- **Desktop App**: Mini window với system tray background sync
- **Đa nền tảng**: Windows, Linux, macOS
- **Tương lai bền vững**: Lưu trữ raw JSON gốc, không transform/format

## Tải xuống

Tải bản cài đặt mới nhất từ [Releases](https://github.com/n24q02m/EchoVault/releases):

| Platform | File |
|----------|------|
| Windows | `EchoVault_x.x.x_x64-setup.exe` |
| macOS (Intel) | `EchoVault_x.x.x_x64.dmg` |
| macOS (Apple Silicon) | `EchoVault_x.x.x_aarch64.dmg` |
| Linux (Debian/Ubuntu) | `echovault_x.x.x_amd64.deb` |
| Linux (AppImage) | `EchoVault_x.x.x_amd64.AppImage` |

## Cài đặt nhanh

Chỉ cần 2 bước:

```bash
# Clone repository
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault

# Chạy setup tự động - cài đặt TẤT CẢ
pnpm setup
```

Script setup sẽ **TỰ ĐỘNG** cài đặt:

1. ✅ **OS packages** - Tauri dependencies (libgtk-3, webkit2gtk, etc.)
2. ✅ **mise** - Tool version manager
3. ✅ **Rust, Node.js, uv** - Via mise
4. ✅ **pnpm** - Package manager
5. ✅ **Node dependencies** - Tất cả packages cần thiết
6. ✅ **Rclone binary** - Sync engine cho Google Drive
7. ✅ **Pre-commit hooks** - Quality checks tự động

> **Lưu ý**: Script yêu cầu sudo password trên Linux để cài system packages.

## Chạy ứng dụng

```bash
# Development mode (full app)
cargo tauri dev

# Development mode (web only)
pnpm dev

# Production build
cargo tauri build

# Reset app (xóa config để setup lại)
pnpm reset
pnpm reset --all
```

## Phát triển

### Rust

```bash
cargo build                # Debug build
cargo test --workspace     # Chạy tests
cargo clippy --workspace   # Lint
cargo fmt --all            # Format code
```

### TypeScript (Frontend)

```bash
cd apps/web
pnpm dev                   # Dev server với HMR
pnpm build                 # Production build
pnpm lint                  # Biome lint
pnpm format                # Biome format
```

### Pre-commit hooks

Pre-commit hooks đã được tự động cài đặt qua setup script. Để chạy thủ công:

```bash
uv run pre-commit run --all-files
```

## Cấu trúc dự án

```text
EchoVault/
├── apps/
│   ├── core/              # Core library (extractors, sync, watcher)
│   │   ├── extractors/    # Chat extractors cho các platforms
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

EchoVault sử dụng **Rclone** làm sync engine để đồng bộ với **Google Drive**:

- **Không cần setup OAuth phức tạp**: Rclone đã có sẵn verified credentials
- **User-friendly**: Chỉ cần click Connect và đăng nhập trong browser
- **Tin cậy**: Rclone là công cụ sync được sử dụng rộng rãi với 40k+ stars trên GitHub

## Tech Stack

- **Backend**: Rust (Tauri, tokio, serde)
- **Frontend**: React + TypeScript (Vite, TailwindCSS)
- **Sync**: Rclone (Google Drive)
- **Extractors**: VS Code SQLite, Cursor, Cline, Antigravity
- **Build Tools**: Cargo, pnpm, mise
- **Dev Tools**: uv, pre-commit, biome

## Troubleshooting

### Setup script bị lỗi

Nếu setup script gặp lỗi, thử các bước sau:

1. **Restart terminal** và chạy lại `pnpm setup`
2. Kiểm tra log chi tiết trong output
3. Cài thủ công các components còn thiếu (xem bên dưới)

### Cài thủ công (nếu setup script thất bại)

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

#### Tools từ mise

```bash
mise install
```

### Lỗi "command not found" sau khi cài

Restart terminal để load PATH mới, hoặc:

```bash
# Linux/macOS
source ~/.bashrc  # hoặc ~/.zshrc

# Windows
# Đóng và mở lại terminal
```

## License

MIT - Xem [LICENSE](LICENSE) để biết thêm chi tiết.
