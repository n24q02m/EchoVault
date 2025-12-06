# ECHOVAULT - DEVELOPER HANDBOOK

**Phiên bản:** 2.3.0
**Ngày cập nhật:** 17/07/2025
**Dành cho:** Solo Developer

---

## MỤC LỤC

1. [TỔNG QUAN DỰ ÁN](#1-tổng-quan-dự-án)
2. [VISION VÀ SCOPE](#2-vision-và-scope)
3. [KIẾN TRÚC HỆ THỐNG](#3-kiến-trúc-hệ-thống)
4. [CÔNG NGHỆ STACK](#4-công-nghệ-stack)
5. [BẢO MẬT VÀ MÃ HÓA](#5-bảo-mật-và-mã-hóa)
6. [HỖ TRỢ IDE VÀ CÔNG CỤ](#6-hỗ-trợ-ide-và-công-cụ)
7. [QUY TRÌNH PHÁT TRIỂN](#7-quy-trình-phát-triển)
8. [CHUẨN MỰC CODE](#8-chuẩn-mực-code)
9. [LỊCH TRÌNH VÀ MILESTONES](#9-lịch-trình-và-milestones)
10. [DEPLOYMENT](#10-deployment)
11. [TROUBLESHOOTING](#11-troubleshooting)

---

## 1. TỔNG QUAN DỰ ÁN

### 1.1. Giới Thiệu

**EchoVault** là "Hộp đen" (Black Box) cho mọi cuộc hội thoại AI của bạn. Dự án giải quyết vấn đề **không thể đồng bộ lịch sử chat AI** giữa nhiều máy tính, nhiều IDE, và nhiều workspace.

Hiện tại, GitHub Copilot, Cursor, Cline và các công cụ AI khác đều lưu trữ lịch sử chat **cục bộ** (locally), gắn liền với từng workspace hoặc IDE cụ thể. EchoVault trích xuất và lưu trữ **nguyên vẹn dữ liệu gốc** - đảm bảo không mất thông tin khi format thay đổi.

### 1.2. Vấn Đề Cần Giải Quyết

- **Phân mảnh dữ liệu**: Chat history nằm rải rác ở nhiều máy (Windows, WSL, Mac), nhiều IDE (VS Code, Cursor, JetBrains).
- **Không thể đồng bộ**: Các IDE không hỗ trợ sync chat history qua cloud.
- **Mất tri thức**: Những insight, code snippets, debugging sessions quý giá bị quên lãng.
- **Format thay đổi liên tục**: VS Code Copilot đã có 3+ versions format khác nhau.

### 1.3. Giải Pháp

```text
IDE Files (JSON/SQLite) -> EchoVault CLI -> Raw JSON Storage -> Encryption -> GitHub Sync
                                |                                               |
                                +-> Copy nguyên vẹn file gốc                    +-> OAuth Device Flow
                                +-> Không format/transform                      +-> Auto push
                                +-> Index metadata để search                    +-> Multi-device sync
```

**Nguyên tắc quan trọng**: Không format lại dữ liệu! Lưu trữ nguyên vẹn JSON gốc để:

- Không mất thông tin khi IDE/model thay đổi format
- Có thể re-parse bất kỳ lúc nào với parser mới
- Future-proof cho các tính năng chưa biết

### 1.4. Người Dùng Mục Tiêu

- **Developers**: Sử dụng nhiều IDE và muốn lưu trữ lịch sử chat tập trung.
- **AI Power Users**: Muốn biến lịch sử chat thành "Knowledge Bank" có thể tái sử dụng.
- **Teams**: Muốn chia sẻ context và patterns giữa các thành viên.

---

## 2. VISION VÀ SCOPE

### 2.1. Core Philosophy

- **Universal Compatibility**: Hoạt động với mọi IDE và công cụ AI phổ biến.
- **Privacy First**: Dữ liệu được mã hóa trước khi rời khỏi máy.
- **Git-Native**: Git + GitHub là cơ chế sync chính.
- **Future-Proof**: Lưu trữ raw data gốc, không transform/format.

### 2.2. Key Features

1. **Universal Extraction**:
    - Copy nguyên vẹn JSON/SQLite files từ các IDE.
    - Plugin architecture cho từng IDE/Tool.
    - Hỗ trợ nhiều schema versions (không cần parse chi tiết).

2. **Raw Storage** (Future-Proof):
    - **Chỉ lưu JSON gốc**: Copy nguyên vẹn, không transform.
    - **Index metadata**: Extract metadata cơ bản (date, title) để search.
    - **Không format Markdown**: Việc render để đọc sẽ làm on-demand trong Desktop app.

3. **Git Synchronization**:
    - Tự động commit vào local Git repository.
    - Push lên GitHub với OAuth Device Flow.
    - Encryption trước khi push.

4. **Desktop App** (Phase 2):
    - Tauri 2.x cross-platform app.
    - Mini window style (như Google Drive Desktop).
    - System tray với background sync.
    - Full-text search và filtering.

---

## 3. KIẾN TRÚC HỆ THỐNG

### 3.1. High-Level Architecture

```text
+------------------+     +------------------+     +------------------+     +------------------+
|   IDE Files      | --> |   EchoVault CLI  | --> |   AES-256-GCM    | --> |     GitHub       |
|  (JSON, SQLite)  |     |     (Rust)       |     |   Encryption     |     |  (Remote Repo)   |
+------------------+     +------------------+     +------------------+     +------------------+
        |                        |                        |                        |
        v                        v                        v                        v
+------------------+     +------------------+     +------------------+     +------------------+
| - VS Code        |     | - Copy raw files |     | - Passphrase     |     | - OAuth Device   |
| - Cursor         |     | - Index metadata |     | - Argon2id KDF   |     |   Flow           |
| - Cline          |     | - Desktop App    |     | - .json.enc      |     | - Auto Push      |
| - Antigravity    |     | - Sync Engine    |     | - Fast (<10ms)   |     | - Version Ctrl   |
+------------------+     +------------------+     +------------------+     +------------------+
```

### 3.2. Components

#### 3.2.1. CLI Tool (Temporary - Phase 1)

> **Lưu ý**: CLI `ev` là giải pháp tạm thời trong Phase 1. Sẽ được thay thế hoàn toàn bởi Tauri Desktop App trong Phase 2.

- **Language**: Rust (single binary, cross-platform)
- **Framework**: clap (derive macro)
- **Binary**: `ev` (viết tắt của EchoVault - có thể trùng với tools khác)
- **Commands**:
  - `ev scan` - Quét và liệt kê tất cả chat sessions có sẵn.
  - `ev sync` - Extract, encrypt và push lên GitHub (all-in-one, tự động setup nếu lần đầu).

#### 3.2.2. Desktop App (Tauri - Phase 2)

Ứng dụng Desktop chính thức thay thế CLI:

**UI Style:**

- Mini window (như Google Drive Desktop, không full-screen)
- Có thể resize nhưng mặc định nhỏ gọn
- System tray icon để chạy nền

**Core Features:**

- Browse sessions theo ngày, project, source
- Tìm kiếm full-text trong tất cả sessions
- Parse và render JSON on-demand
- Xem nội dung đã decrypt (tự động pull từ GitHub)
- Copy code blocks vào clipboard

**Background Sync:**

- Chạy trong system tray khi đóng window (minimize to tray)
- Auto-sync định kỳ (configurable: 30 phút, 1 giờ, etc.)
- Notifications khi sync thành công/thất bại
- Auto-start khi login (optional)

**Tauri Plugins:**

- `tauri-plugin-autostart` - Tự động khởi động khi login
- `tauri-plugin-notification` - Hiển thị notifications
- System tray - Built-in trong Tauri 2.x

**Packaging Formats:**

| Platform    | Formats                     | Mô tả                                            |
| :---------- | :-------------------------- | :----------------------------------------------- |
| **Windows** | `.exe`, `.msi`              | Portable executable hoặc MSI installer           |
| **macOS**   | `.dmg`, `.app`              | DMG disk image hoặc App bundle                   |
| **Linux**   | `.AppImage`, `.deb`, `.rpm` | Universal AppImage hoặc distro-specific packages |

#### 3.2.3. Extractors (Plugin Architecture)

Mỗi IDE/Tool có một Extractor riêng, nhưng chỉ copy files, không parse chi tiết:

```text
src/
  extractors/
    mod.rs           # Trait definition
    vscode_copilot.rs
    cursor.rs
    cline.rs
    antigravity.rs
```

**Extractor responsibilities:**

- Tìm đường dẫn lưu trữ của IDE
- Copy raw JSON files vào vault
- Extract metadata cơ bản (filename, date, size) cho index

#### 3.2.4. Sync Engine

- Git-based synchronization với `git2` crate.
- **GitHub as primary storage**: Không lưu local lâu dài, chỉ cache tạm.
- **Auto-push**: Tự động đẩy lên GitHub sau mỗi extract.
- **Encryption by default**: AES-256-GCM trước khi push.
- **OAuth Device Flow**: Authentication chính (không cần PAT/SSH key).

#### 3.2.5. Encryption Layer

- **AES-256-GCM**: Military-grade encryption cho tất cả files.
- **Passphrase-based**: User cung cấp passphrase khi sync lần đầu.
- **Key Derivation**: Argon2id để derive encryption key từ passphrase.
- **Only encrypted files synced**: Chỉ push files đã mã hóa (.enc) lên GitHub.

---

## 4. CÔNG NGHỆ STACK

### 4.1. Core Stack (Rust)

| Layer              | Crate              | Purpose                           |
| :----------------- | :----------------- | :-------------------------------- |
| **Language**       | Rust 1.83+         | Core Logic, single binary         |
| **CLI Framework**  | clap (derive)      | Modern CLI with auto-completion   |
| **SQLite Reader**  | rusqlite           | Read IDE databases (Cursor, etc.) |
| **JSON**           | serde, serde_json  | Serialization/Deserialization     |
| **Encryption**     | aes-gcm            | AES-256-GCM encryption            |
| **Key Derivation** | argon2             | Passphrase to key derivation      |
| **Git Operations** | git2               | libgit2 bindings                  |
| **HTTP Client**    | reqwest            | OAuth Device Flow                 |
| **Terminal UI**    | indicatif, colored | Progress bars, colors             |
| **Error Handling** | anyhow, thiserror  | Ergonomic errors                  |

### 4.2. Desktop App Stack (Tauri)

| Layer         | Technology       | Purpose                    |
| :------------ | :--------------- | :------------------------- |
| **Framework** | Tauri 2.x        | Cross-platform desktop app |
| **Frontend**  | React/TypeScript | UI components              |
| **Styling**   | Tailwind CSS     | Responsive design          |
| **State**     | Zustand          | Simple state management    |

### 4.3. Storage Format

| Layer       | Format           | Use Case               |
| :---------- | :--------------- | :--------------------- |
| **Primary** | Raw JSON gốc     | Copy nguyên vẹn từ IDE |
| **Index**   | SQLite           | Fast search, metadata  |
| **Sync**    | Encrypted (.enc) | Secure sync to GitHub  |

**Tại sao không format lại?**

- VS Code Copilot đã thay đổi format 3+ lần (v1 → v2 → v3)
- Model khác nhau (GPT-4, Claude, GPT-5-Codex) có response structure khác
- Lưu raw JSON giúp không mất data, có thể re-parse bất kỳ lúc nào

### 4.4. Build & Distribution

| Platform          | Method                          |
| :---------------- | :------------------------------ |
| **Linux**         | cargo build --release, AppImage |
| **macOS**         | cargo build --release, Homebrew |
| **Windows**       | cargo build --release, MSI/exe  |
| **Cross-compile** | cross (Docker-based)            |

---

## 5. BẢO MẬT VÀ MÃ HÓA

### 5.1. Vấn Đề

Chat history có thể chứa thông tin nhạy cảm:

- API keys, tokens, passwords
- Database credentials
- Private keys, certificates
- Sensitive business logic

**Push lên Git mà không mã hóa = Rủi ro bảo mật nghiêm trọng!**

### 5.2. Giải Pháp: Encryption (AES-256-GCM)

EchoVault sử dụng **một layer bảo mật duy nhất**: Mã hóa toàn bộ nội dung trước khi đẩy lên GitHub.

```bash
# Lần đầu chạy sync sẽ tự động init
ev sync
# Browser mở -> Đăng nhập GitHub -> Nhập passphrase
# Tự động extract -> encrypt -> commit -> push
```

**Đặc điểm:**

- **AES-256-GCM**: Military-grade encryption
- **Hardware accelerated**: < 10% overhead trên CPU hiện đại (AES-NI)
- **Performance**: Encrypt/decrypt file KB-MB trong **milliseconds**
- **Passphrase-based**: Sử dụng Argon2id để derive key từ passphrase

### 5.3. GitHub Synchronization với OAuth Device Flow

EchoVault sử dụng **OAuth Device Flow** để authentication với GitHub - không cần copy/paste token!

#### OAuth Device Flow

```text
+------------------+                              +------------------+
|  EchoVault CLI   |                              |     GitHub       |
+------------------+                              +------------------+
        |                                                 |
        |  1. POST /login/device/code                     |
        |  (client_id, scope)                             |
        |------------------------------------------------>|
        |                                                 |
        |  2. device_code, user_code, verification_uri    |
        |<------------------------------------------------|
        |                                                 |
        |  3. Display: "Go to github.com/login/device     |
        |     and enter code: ABCD-1234"                  |
        |                                                 |
        |                    [User opens browser,         |
        |                     enters code, authorizes]    |
        |                                                 |
        |  4. Poll POST /login/oauth/access_token         |
        |     (device_code, client_id)                    |
        |------------------------------------------------>|
        |                                                 |
        |  5. access_token (when authorized)              |
        |<------------------------------------------------|
        |                                                 |
```

**Ưu điểm của OAuth Device Flow:**

- Không cần copy/paste Personal Access Token
- Không cần SSH key setup
- Hoạt động tốt cho CLI applications
- Token có thể revoke từ GitHub settings
- Scope hạn chế (chỉ cần repo access)
- **Auto-create repository**: Tự động tạo repo nếu chưa tồn tại

### 5.4. Encryption Workflow

```text
                    +-----------------+
                    |   User Input    |
                    |  (Passphrase)   |
                    +-----------------+
                           |
                           v
                    +-----------------+
                    |  Key Derivation |
                    |  (Argon2id)     |
                    +-----------------+
                           |
                           v
+------------------+  +-----------------+  +------------------+
|  Raw JSON Files  |->|  AES-256-GCM    |->| Encrypted File   |
|  (with secrets)  |  |   Encryption    |  |  (.json.enc)     |
+------------------+  +-----------------+  +------------------+
                                                    |
                                                    v
                                           +------------------+
                                           |   Git Commit     |
                                           |   & Auto Push    |
                                           |   to GitHub      |
                                           +------------------+
```

### 5.5. Cấu Hình

```toml
# echovault.toml (auto-generated by sync)

[sync]
# Remote repository (GitHub)
remote = "https://github.com/username/my-vault.git"
# OAuth Device Flow is the only authentication method
# Token is stored in .credentials.json file

[encryption]
# Encryption is always enabled, cannot be disabled
# Passphrase is stored securely in system keychain
```

### 5.6. Lưu Ý Bảo Mật

1. **Passphrase là chìa khóa duy nhất**: Mất passphrase = mất dữ liệu
2. **Backup passphrase**: Lưu passphrase ở nơi an toàn (password manager)
3. **Repo visibility**: Private repo khuyến nghị, nhưng Public cũng OK vì đã encrypt
4. **Đồng bộ passphrase**: Sử dụng cùng passphrase trên tất cả máy

---

## 6. HỖ TRỢ IDE VÀ CÔNG CỤ

### 6.1. Phase 1: VS Code Copilot

#### Storage Location

| Platform | Storage Path                                                                                |
| :------- | :------------------------------------------------------------------------------------------ |
| Windows  | `%APPDATA%\Code\User\workspaceStorage\<hash>\chatSessions\*.json`                           |
| macOS    | `~/Library/Application Support/Code/User/workspaceStorage/<hash>/chatSessions/*.json`       |
| Linux    | `~/.config/Code/User/workspaceStorage/<hash>/chatSessions/*.json`                           |
| WSL      | `/mnt/c/Users/<user>/AppData/Roaming/Code/User/workspaceStorage/<hash>/chatSessions/*.json` |

- **Format**: JSON files (từ VS Code 1.96+)
- **Versions**: v1 (cũ), v2, v3 (hiện tại) - format thay đổi thường xuyên
- **Data**: Structured JSON với messages, tool calls, thinking steps

### 6.2. Phase 2: Antigravity + Others

#### Google Antigravity

- VS Code fork với AI-first design
- Storage location: Cần research (likely similar to VS Code)
- Có "Knowledge Items" feature

#### Các IDE khác (Phase 3)

| IDE              | Storage                                 | Format   |
| :--------------- | :-------------------------------------- | :------- |
| **Cursor**       | `workspaceStorage/<hash>/state.vscdb`   | SQLite   |
| **Cline**        | `globalStorage/saoudrizwan.claude-dev/` | JSON     |
| **Aider**        | `~/.aider.chat.history.md`              | Markdown |
| **Claude Code**  | `~/.claude/projects/`                   | JSONL    |
| **JetBrains AI** | Cần research                            | Unknown  |

---

## 7. QUY TRÌNH PHÁT TRIỂN

### 7.1. Prerequisites

- [mise](https://mise.jdx.dev/) (quản lý toolchain)
- Git

### 7.2. Setup

```bash
# Clone
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault

# Cài đặt Tauri dependencies (Linux)
sudo apt update && sudo apt install -y pkg-config libgtk-3-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev

# Cài đặt toolchain qua mise
mise trust && mise install

# Cài đặt frontend dependencies
cd src-web && pnpm install && cd ..

# Build
cargo build --release
```

### 7.3. Development Commands

**Rust (Backend):**

```bash
cargo build                     # Debug build
cargo build --release           # Release build
cargo test --workspace          # Chạy tất cả tests
cargo clippy -- -D warnings     # Lint
cargo fmt                       # Format
cargo fmt --check               # Check format
```

**TypeScript (Frontend):**

```bash
cd src-web
pnpm dev              # Dev server
pnpm build            # Production build
pnpm lint             # ESLint
pnpm format           # Prettier
pnpm typecheck        # TypeScript check
```

**Tauri App:**

```bash
cargo tauri dev       # Development mode
cargo tauri build     # Production build
```

### 7.4. Project Structure

```text
EchoVault/
  Cargo.toml                      # Workspace root
  mise.toml                       # Toolchain (rust, node)
  crates/
    echovault-core/               # Core library
      src/
        lib.rs                    # Library entry
        config.rs                 # Configuration
        crypto/                   # AES-256-GCM, Argon2id
        extractors/               # IDE extractors
        storage/                  # SQLite index
        sync/                     # Git sync, OAuth
  src-tauri/                      # Tauri backend
    src/
      lib.rs                      # Tauri commands
      commands.rs                 # API handlers
    tauri.conf.json               # Tauri config
  src-web/                        # React frontend
    src/
      App.tsx                     # Main component
      index.css                   # Tailwind styles
    package.json
  docs/
    HANDBOOK.md
```

---

## 8. CHUẨN MỰC CODE

### 8.1. Language

- **Code**: 100% Tiếng Anh
- **Docs/Comments**: Tiếng Việt (có dấu)
- **Commit Messages**: Tiếng Anh, Conventional Commits

### 8.2. Style

- **Formatter**: rustfmt (default settings)
- **Linter**: clippy (pedantic level)
- **Documentation**: rustdoc cho public APIs
- **Error Handling**: anyhow cho applications, thiserror cho libraries

### 8.3. Architecture Principles

- **Plugin Architecture**: Mỗi IDE là một module độc lập.
- **No Data Transformation**: Extractors chỉ copy, không format.
- **Single Responsibility**: Mỗi module làm một việc.
- **Fail Gracefully**: Lỗi một IDE không ảnh hưởng IDE khác.

---

## 9. LỊCH TRÌNH VÀ MILESTONES

### Phase 1: MVP (VS Code Copilot + Vault Core + Desktop App)

**Extractor:**

- [x] Core CLI với clap (scan, extract commands)
- [x] VS Code Copilot Extractor (tìm files)
- [x] Cross-platform path handling (Windows, macOS, Linux, WSL)
- [ ] Raw JSON storage (copy files, không format)
- [ ] Metadata index (SQLite)

**Vault Core:**

- [x] Basic configuration (`echovault.toml`)
- [x] AES-256-GCM Encryption với Argon2id
- [x] Git Sync Engine (auto-commit, push)
- [x] GitHub OAuth Device Flow
- [x] Auto-create GitHub repository

**Desktop App (Tauri):**

- [ ] Tauri 2.x setup
- [ ] React/TypeScript frontend
- [ ] Session list và navigation
- [ ] JSON parsing on-demand để render
- [ ] Full-text search

### Phase 2: Antigravity + More Features

**More Features:**

- [ ] Google Antigravity Extractor
- [ ] Clipboard integration
- [ ] Export to Markdown (on-demand)
- [ ] Advanced search và filtering

### Phase 3: More Extractors + Advanced

**Extractors:**

- [ ] Cursor Extractor (SQLite)
- [ ] Cline (Claude Dev) Extractor
- [ ] GitHub Copilot for JetBrains
- [ ] Aider Extractor
- [ ] Claude Code Extractor

**Advanced:**

- [ ] Semantic search với embeddings
- [ ] Context injection helpers
- [ ] Team sharing features

---

## 10. DEPLOYMENT

### 10.1. CLI Distribution

- **Cargo**: `cargo install --path .` hoặc `mise run install`
- **Binary**: `ev` - Single executable cho Windows/macOS/Linux
- **Homebrew**: Formula cho macOS (planned)
- **AUR**: Package cho Arch Linux (planned)

### 10.2. Desktop App Distribution

- **Windows**: MSI installer
- **macOS**: DMG
- **Linux**: AppImage, deb, rpm

---

## 11. TROUBLESHOOTING

### 11.1. Database Locked

**Vấn đề**: `sqlite3.OperationalError: database is locked`

**Nguyên nhân**: IDE đang mở và sử dụng database.

**Giải pháp**:

1. EchoVault tự động copy database ra temp folder trước khi đọc.
2. Sử dụng `PRAGMA journal_mode=WAL` để đọc trong khi IDE đang ghi.

### 11.2. Workspace Hash Không Tìm Thấy

**Vấn đề**: Không biết hash nào ứng với workspace nào.

**Giải pháp**:

1. Đọc file `workspace.json` trong mỗi hash folder.
2. Match với project path hiện tại.

### 11.3. OAuth Device Flow Fails

**Vấn đề**: Không thể authenticate với GitHub.

**Giải pháp**:

1. Kiểm tra kết nối internet
2. Xóa file credentials và chạy lại:
   - Linux: `rm ~/.config/echovault/.credentials.json && ev sync`
   - macOS: `rm ~/Library/Application\ Support/echovault/.credentials.json && ev sync`
   - Windows: `del %APPDATA%\echovault\.credentials.json && ev sync`
3. Kiểm tra GitHub status page
4. Revoke token cũ trong GitHub Settings > Applications > Authorized OAuth Apps

### 11.4. WSL Path Issues

**Vấn đề**: Windows và WSL có paths khác nhau.

**Giải pháp**:

1. EchoVault auto-detect WSL và đọc từ `/mnt/c/Users/...`
2. Mỗi môi trường có thể chạy instance riêng của EchoVault

### 11.5. Large Chat History

**Vấn đề**: Database quá lớn, extract chậm.

**Giải pháp**:

1. Incremental extraction (chỉ extract sessions mới).
2. Compression với gzip.
3. Configurable time range filter.
