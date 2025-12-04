# ECHOVAULT - DEVELOPER HANDBOOK

**Phiên bản:** 2.0.0
**Ngày cập nhật:** 04/12/2025
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

Hiện tại, GitHub Copilot, Cursor, Cline và các công cụ AI khác đều lưu trữ lịch sử chat **cục bộ** (locally), gắn liền với từng workspace hoặc IDE cụ thể. EchoVault trích xuất, chuẩn hóa và lưu trữ tất cả thành **Markdown** - format có thể đọc được ở bất kỳ đâu, bất kỳ lúc nào.

### 1.2. Vấn Đề Cần Giải Quyết

- **Phân mảnh dữ liệu**: Chat history nằm rải rác ở nhiều máy (Windows, WSL, Mac), nhiều IDE (VS Code, Cursor, JetBrains).
- **Không thể đồng bộ**: Các IDE không hỗ trợ sync chat history qua cloud.
- **Mất tri thức**: Những insight, code snippets, debugging sessions quý giá bị quên lãng.
- **Khó tái sử dụng**: Không thể inject context từ cuộc chat cũ vào cuộc chat mới.

### 1.3. Giải Pháp

```text
IDE Databases (SQLite/JSON) -> EchoVault CLI -> Markdown Files -> Git Sync -> Context for AI
```

### 1.4. Người Dùng Mục Tiêu

- **Developers**: Sử dụng nhiều IDE và muốn lưu trữ lịch sử chat tập trung.
- **AI Power Users**: Muốn biến lịch sử chat thành "Knowledge Bank" có thể tái sử dụng.
- **Teams**: Muốn chia sẻ context và patterns giữa các thành viên.

---

## 2. VISION VÀ SCOPE

### 2.1. Core Philosophy

- **Universal Compatibility**: Hoạt động với mọi IDE và công cụ AI phổ biến.
- **Privacy First**: Dữ liệu được trích xuất và xử lý hoàn toàn cục bộ. Không gửi lên cloud của bên thứ ba.
- **Git-Native**: Git là cơ chế sync chính. Đơn giản, đáng tin cậy, miễn phí.
- **Markdown-Centric**: Output là Markdown thuần - đọc được ở mọi nơi, import được vào mọi AI.

### 2.2. Key Features

1. **Universal Extraction**:
    - Tự động phát hiện và trích xuất từ SQLite databases (`state.vscdb`), JSON files, JSONL logs.
    - Plugin architecture cho từng IDE/Tool.

2. **Format Standardization**:
    - Chuyển đổi mọi format proprietary thành Markdown với frontmatter metadata.
    - Giữ nguyên code blocks, artifacts, và formatting.

3. **Git Synchronization**:
    - Tự động commit vào local Git repository.
    - Push lên private remote (GitHub, GitLab, self-hosted).
    - Conflict resolution thông minh.

4. **Context Injection** (Future):
    - Sử dụng file Markdown như `@context` cho Cursor, Cline.
    - Semantic search để tìm context liên quan.

---

## 3. KIẾN TRÚC HỆ THỐNG

### 3.1. High-Level Architecture

```text
+------------------+     +------------------+     +------------------+     +------------------+
|   IDE Databases  | --> |   EchoVault CLI  | --> |   AES-256-GCM    | --> |     GitHub       |
|  (SQLite, JSON)  |     |     (Rust)       |     |   Encryption     |     |  (Remote Repo)   |
+------------------+     +------------------+     +------------------+     +------------------+
        |                        |                        |                        |
        v                        v                        v                        v
+------------------+     +------------------+     +------------------+     +------------------+
| - VS Code        |     | - Extractors     |     | - Passphrase     |     | - HTTPS/SSH      |
| - Cursor         |     | - Formatters     |     | - Argon2id KDF   |     | - Auto Push      |
| - Cline          |     | - Sync Engine    |     | - .md.enc files  |     | - Private/Public |
| - Aider          |     | - TUI Viewer     |     | - Fast (<10ms)   |     | - Version Ctrl   |
+------------------+     +------------------+     +------------------+     +------------------+
```

### 3.2. Components

#### 3.2.1. CLI Tool (Core)

- **Language**: Rust (single binary, cross-platform)
- **Framework**: clap (derive macro)
- **Commands**:
  - `echovault init` - Khởi tạo vault mới với encryption key và GitHub remote.
  - `echovault scan` - Quét và liệt kê tất cả chat sessions có sẵn.
  - `echovault extract` - Trích xuất và chuyển đổi thành Markdown.
  - `echovault sync` - Commit và push lên GitHub (với encryption).
  - `echovault view` - Mở TUI để xem và tìm kiếm lịch sử chat.

#### 3.2.2. TUI Viewer (ratatui)

Giao diện Terminal UI để xem lại lịch sử chat:

```text
┌─────────────────────────────────────────────────────────────┐
│ EchoVault - Chat History Viewer                    [q]uit  │
├─────────────────────────────────────────────────────────────┤
│ Sessions                    │ Content                       │
│ ─────────────────────────── │ ───────────────────────────── │
│ > 2025-12-04 FastAPI Mid... │ ## User                       │
│   2025-12-03 Rust CLI ar... │ Tôi cần tối ưu middleware...  │
│   2025-12-02 TypeScript...  │                               │
│   2025-12-01 Database op... │ ## Assistant                  │
│                             │ Dưới đây là code đã tối ưu... │
│                             │                               │
│ [/] Search  [↑↓] Navigate   │ [Enter] Open  [c] Copy        │
└─────────────────────────────────────────────────────────────┘
```

**Features:**

- Browse sessions theo ngày, project, source
- Tìm kiếm full-text trong tất cả sessions
- Xem nội dung đã decrypt (tự động pull từ GitHub)
- Copy code blocks vào clipboard
- Keyboard-driven, không cần chuột

#### 3.2.3. Extractors (Plugin Architecture)

Mỗi IDE/Tool có một Extractor riêng:

```text
src/
  extractors/
    mod.rs           # Trait definition
    vscode_copilot.rs
    cursor.rs
    cline.rs
    aider.rs
    claude_code.rs
```

#### 3.2.4. Formatters

- **MarkdownFormatter**: Chuyển đổi raw data thành Markdown với frontmatter.
- **JSONFormatter**: Export raw JSON cho advanced use cases.

#### 3.2.5. Sync Engine

- Git-based synchronization với `git2` crate.
- **GitHub as primary storage**: Không lưu local lâu dài, chỉ cache tạm.
- **Auto-push**: Tự động đẩy lên GitHub sau mỗi extract.
- **Encryption by default**: AES-256-GCM trước khi push.
- **Auth methods**: HTTPS (Personal Access Token) hoặc SSH key.

#### 3.2.6. Encryption Layer

- **AES-256-GCM**: Military-grade encryption cho tất cả files.
- **Passphrase-based**: User cung cấp passphrase khi init.
- **Key Derivation**: Argon2id để derive encryption key từ passphrase.

---

## 4. CÔNG NGHỆ STACK

### 4.1. Core Stack (Rust)

| Layer | Crate | Purpose |
| :--- | :--- | :--- |
| **Language** | Rust 1.75+ | Core Logic, single binary |
| **CLI Framework** | clap (derive) | Modern CLI with auto-completion |
| **TUI Framework** | ratatui, crossterm | Terminal UI for viewing history |
| **SQLite Reader** | rusqlite | Read IDE databases (40M+ downloads) |
| **JSON** | serde, serde_json | Serialization/Deserialization |
| **Encryption** | aes-gcm | AES-256-GCM encryption |
| **Key Derivation** | argon2 | Passphrase to key derivation |
| **Git Operations** | git2 | libgit2 bindings |
| **Async Runtime** | tokio | Async I/O (optional) |
| **Terminal UI** | indicatif, colored | Progress bars, colors |
| **Error Handling** | anyhow, thiserror | Ergonomic errors |

### 4.2. Output Format

| Format | Use Case |
| :--- | :--- |
| **Markdown** | Primary output, human-readable, AI-injectable |
| **JSON** | Raw data export, programmatic access |
| **Encrypted Markdown** | Secure sync to remote Git |

### 4.3. Build & Distribution

| Platform | Method |
| :--- | :--- |
| **Linux** | cargo build --release, AppImage |
| **macOS** | cargo build --release, Homebrew |
| **Windows** | cargo build --release, MSI/exe |
| **Cross-compile** | cross (Docker-based) |

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

```text
echovault init --passphrase "your-secure-passphrase"
echovault sync  # Tự động encrypt -> commit -> push
```

**Đặc điểm:**

- **AES-256-GCM**: Military-grade encryption
- **Hardware accelerated**: < 10% overhead trên CPU hiện đại (AES-NI)
- **Performance**: Encrypt/decrypt file KB-MB trong **milliseconds**
- **Passphrase-based**: Sử dụng Argon2id để derive key từ passphrase

**Tại sao chỉ cần Encryption?**

- Nếu đã mã hóa toàn bộ file, secrets bên trong cũng đã được bảo vệ
- Không cần secret detection/redaction vì plaintext không bao giờ rời khỏi máy
- Đơn giản hóa kiến trúc, giảm dependencies

### 5.3. GitHub Synchronization

EchoVault **bắt buộc** sync lên GitHub (không hỗ trợ local-only storage):

| Feature | Description |
| :--- | :--- |
| **Primary Storage** | GitHub repository (không lưu local lâu dài) |
| **Local Cache** | Chỉ lưu tạm để xử lý, tự động xóa sau khi push |
| **Auto Push** | Tự động đẩy lên GitHub sau mỗi extract |
| **Repo Visibility** | Do người dùng tự chọn (Private recommended, Public OK vì đã encrypt) |

#### Authentication Methods

**HTTPS (Personal Access Token):**

```text
echovault init --remote https://github.com/username/vault.git --token ghp_xxxx
```

**SSH (SSH Key):**

```text
echovault init --remote git@github.com:username/vault.git
```

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
|  Raw Markdown    |->|  AES-256-GCM    |->| Encrypted File   |
|  (with secrets)  |  |   Encryption    |  |  (.md.enc)       |
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
# echovault.toml

[sync]
# Remote repository (GitHub)
remote = "git@github.com:username/my-vault.git"

# Authentication method: "ssh" or "https"
auth_method = "ssh"

# For HTTPS: Personal Access Token (stored securely in keychain)
# token = "ghp_xxxx"  # Or use environment variable ECHOVAULT_GITHUB_TOKEN

[encryption]
# Encryption is always enabled, cannot be disabled
# passphrase is stored securely in system keychain
```

### 5.6. Lưu Ý Bảo Mật

1. **Passphrase là chìa khóa duy nhất**: Mất passphrase = mất dữ liệu
2. **Backup passphrase**: Lưu passphrase ở nơi an toàn (password manager)
3. **Repo visibility**: Private repo khuyến nghị, nhưng Public cũng OK vì đã encrypt
4. **Đồng bộ passphrase**: Sử dụng cùng passphrase trên tất cả máy

---

## 6. HỖ TRỢ IDE VÀ CÔNG CỤ

### 6.1. Tier 1: Ưu Tiên Cao (Có Thông Tin Rõ Ràng)

#### VS Code Copilot

| Platform | Storage Path |
| :--- | :--- |
| Windows | `%APPDATA%\Code\User\workspaceStorage\<hash>\state.vscdb` |
| macOS | `~/Library/Application Support/Code/User/workspaceStorage/<hash>/state.vscdb` |
| Linux | `~/.config/Code/User/workspaceStorage/<hash>/state.vscdb` |
| WSL | `~/.vscode-server/data/User/workspaceStorage/<hash>/state.vscdb` |

- **Format**: SQLite database
- **Key**: `interactive.sessions` trong bảng `ItemTable`
- **Data**: JSON string chứa chat history

#### Cursor

| Platform | Storage Path |
| :--- | :--- |
| Windows | `%APPDATA%\Cursor\User\workspaceStorage\<hash>\state.vscdb` |
| macOS | `~/Library/Application Support/Cursor/User/workspaceStorage/<hash>/state.vscdb` |
| Linux | `~/.config/Cursor/User/workspaceStorage/<hash>/state.vscdb` |

- **Format**: SQLite database (tương tự VS Code)
- **Key**: Tìm keys chứa `cursor` hoặc `composer`
- **Data**: JSON string, có thể bao gồm cả artifacts

#### Cline (Claude Dev)

| Platform | Storage Path |
| :--- | :--- |
| Windows | `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\` |
| macOS | `~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/` |
| Linux | `~/.config/Code/User/globalStorage/saoudrizwan.claude-dev/` |

- **Format**: JSON files trong task directories
- **Data**: Structured JSON với messages và artifacts

#### Aider

| Platform | Storage Path |
| :--- | :--- |
| All | `~/.aider.chat.history.md` (global) |
| Per-project | `.aider.chat.history.md` (trong project root) |

- **Format**: Markdown (native!)
- **Data**: Đã sẵn sàng sử dụng, chỉ cần copy/sync

#### Claude Code

| Platform | Storage Path |
| :--- | :--- |
| All | `~/.claude/projects/<project-hash>/` |

- **Format**: JSONL (JSON Lines)
- **Data**: Session logs với timestamps

### 6.2. Tier 2: Cần Research Thêm

| Tool | Status | Notes |
| :--- | :--- | :--- |
| **Google Antigravity** | Research | VS Code fork, có Knowledge Items |
| **JetBrains AI** | Research | Có migration giữa phiên bản |
| **Windsurf** | Research | Codeium's IDE |
| **Codex CLI** | Known | `~/.codex/sessions` (JSONL) |
| **OpenCode** | Known | `.opencode` directories (SQLite) |

### 6.3. Markdown Output Format

```markdown
---
title: "Refactor FastAPI Middleware"
date: 2025-12-04T16:30:00
source: vscode-copilot
project: my-logistics-app
workspace_hash: a1b2c3d4
tags: [fastapi, middleware, refactor]
---

## User

Tôi cần tối ưu lại middleware này để handle error tốt hơn...

## Assistant (Claude Opus 4.5)

Dưới đây là đoạn code đã tối ưu:

\`\`\`python
# Code here
\`\`\`

---

## User

Giải thích thêm về error handling...

## Assistant

...
```

---

## 7. QUY TRÌNH PHÁT TRIỂN

### 7.1. Prerequisites

- Rust 1.75+ (rustup recommended)
- Git

### 7.2. Setup

```bash
# Clone repository
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault

# Build
cargo build

# Run
cargo run -- --help
```

### 7.3. Development Commands

```bash
# Run CLI
cargo run -- scan

# Build release
cargo build --release

# Run tests
cargo test

# Lint
cargo clippy

# Format
cargo fmt

# Check for security vulnerabilities
cargo audit
```

### 7.4. Project Structure

```text
EchoVault/
  src/
    main.rs               # Entry point
    cli.rs                # clap CLI definitions
    config.rs             # Configuration management
    extractors/
      mod.rs              # Extractor trait
      vscode_copilot.rs
      cursor.rs
      cline.rs
      aider.rs
      claude_code.rs
    formatters/
      mod.rs
      markdown.rs
      json.rs
    sync/
      mod.rs
      git.rs
    crypto/
      mod.rs
      encryption.rs       # AES-256-GCM
      key_derivation.rs   # Argon2id
    tui/
      mod.rs
      app.rs              # TUI application state
      ui.rs               # ratatui widgets
      handlers.rs         # Keyboard event handlers
  tests/
  docs/
    HANDBOOK.md
  Cargo.toml
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
- **Dependency Injection**: Extractors và Formatters có thể swap được.
- **Single Responsibility**: Mỗi class làm một việc.
- **Fail Gracefully**: Lỗi một IDE không ảnh hưởng IDE khác.

---

## 9. LỊCH TRÌNH VÀ MILESTONES

### Phase 1: The Extractor (MVP)

- [ ] Core CLI với clap
- [ ] VS Code Copilot Extractor
- [ ] Cursor Extractor
- [ ] Cline Extractor
- [ ] Aider Extractor
- [ ] Markdown Formatter với frontmatter
- [ ] Basic configuration (`echovault.toml`)
- [ ] Cross-platform path handling (Windows, macOS, Linux, WSL)

### Phase 2: The Vault (Complete)

- [ ] AES-256-GCM Encryption với Argon2id
- [ ] Git Sync Engine (auto-commit, push)
- [ ] GitHub authentication (HTTPS/SSH)
- [ ] TUI Viewer với ratatui
- [ ] Full-text search trong TUI
- [ ] Clipboard integration

---

## 10. DEPLOYMENT

### 10.1. CLI Distribution

- **Cargo**: `cargo install echovault`
- **Binary**: Single executable cho Windows/macOS/Linux
- **Homebrew**: Formula cho macOS
- **AUR**: Package cho Arch Linux

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

### 11.3. Schema Thay Đổi

**Vấn đề**: IDE update làm thay đổi cấu trúc database.

**Giải pháp**:

1. Version detection trong Extractor.
2. Fallback logic cho nhiều schema versions.
3. Community reporting để update nhanh.

### 11.4. WSL Path Issues

**Vấn đề**: Windows và WSL có paths khác nhau.

**Giải pháp**:

1. Detect môi trường runtime.
2. Sử dụng native paths (không convert `\\wsl$`).
3. Mỗi môi trường chạy instance riêng của EchoVault.

### 11.5. Large Chat History

**Vấn đề**: Database quá lớn, extract chậm.

**Giải pháp**:

1. Incremental extraction (chỉ extract sessions mới).
2. Pagination cho output.
3. Configurable time range filter.
