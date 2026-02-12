# EchoVault

> [!CAUTION]
> **PROJECT DISCONTINUED** — The replacement solution is being developed at [mnemo-mcp](https://github.com/n24q02m/mnemo-mcp).

**Black box for all your AI conversations.**

EchoVault extracts, indexes, and searches chat history from 12+ AI coding tools — ensuring you never lose valuable insights. Works as a desktop app, CLI, or MCP server for AI assistants.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## Features

- **11 Source Extractors**: VS Code Copilot, Cursor, Cline, Continue.dev, JetBrains AI, Zed, Antigravity, Gemini CLI, Claude Code, Codex, OpenCode
- **Hybrid Search**: Vector semantic search + FTS5 keyword search with RRF fusion
- **MCP Server**: Expose your vault to Claude Desktop, Copilot, Cursor, and other AI assistants
- **Cloud Sync**: Auto-sync with Google Drive via Rclone
- **Desktop App**: Mini window with system tray, background sync, auto-update
- **Embedding Presets**: Built-in Ollama/OpenAI support — no external proxy needed
- **Cross-platform**: Windows, Linux, macOS
- **Future-proof**: Stores raw files without transformation

## Architecture

```
IDE Sources (11)          EchoVault Pipeline              AI Assistants
+-----------------+     +-------------------------+     +----------------+
| VS Code Copilot |     |                         |     | Claude Desktop |
| Cursor          | --> | Extract --> Parse -->    |     | VS Code Copilot|
| Cline           |     |   Embed --> Index -->    | --> | Cursor         |
| Continue.dev    |     |     Search (Hybrid)      |     | Any MCP Client |
| JetBrains AI    |     |                         |     +----------------+
| Zed             |     +----+----+----+----------+
| Gemini CLI      |          |    |    |
| Claude Code     |       vault.db  index.db  embeddings.db
| Codex           |          |
| OpenCode        |       Google Drive (Rclone sync)
| Antigravity     |
+-----------------+
```

## Privacy Notice

> [!WARNING]
> EchoVault accesses your AI chat history. This data may contain:
>
> - Code snippets and file paths
> - API keys or secrets mentioned in conversations
> - Personal information
>
> Cloud sync is optional. All data stays local unless you explicitly connect Google Drive.

---

## MCP Server

The MCP server exposes your vault to AI assistants via 2 tools:

| Tool | Description |
|------|-------------|
| `vault` | Unified interface: `list`, `search` (FTS5), `read`, `semantic_search` (hybrid) |
| `help` | On-demand documentation (saves tokens — only called when needed) |

### Setup

#### 1. Install CLI

**Quick install (recommended):**

```bash
# Linux/macOS
curl -fsSL https://raw.githubusercontent.com/n24q02m/EchoVault/main/install-cli.sh | bash
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/n24q02m/EchoVault/main/install-cli.ps1 | iex
```

> [!TIP]
> The [full install script](#quick-install-recommended) (Desktop App) also includes the CLI.
> Only use `install-cli` if you need CLI/MCP only (e.g., servers, CI, headless).

**Build from source:**

```bash
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault
cargo build -p echovault-cli --release
# Binary at: target/release/echovault-cli
```

#### 2. Prepare Vault Data

```bash
# Extract sessions from all detected IDEs
echovault-cli extract

# Parse raw files into clean Markdown
echovault-cli parse

# (Optional) Build embedding index for semantic search
# Requires Ollama running locally, or set OpenAI API key in config
echovault-cli embed
```

#### 3. Configure MCP Client

**Claude Desktop** (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "echovault": {
      "command": "echovault-cli",
      "args": ["mcp"]
    }
  }
}
```

**VS Code Copilot** (`.vscode/mcp.json`):

```json
{
  "servers": {
    "echovault": {
      "type": "stdio",
      "command": "echovault-cli",
      "args": ["mcp"]
    }
  }
}
```

**Cursor** (`~/.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "echovault": {
      "command": "echovault-cli",
      "args": ["mcp"]
    }
  }
}
```

> [!NOTE]
> The install scripts add `echovault-cli` to your PATH automatically.
> If you built from source, use the full path: `"command": "/path/to/target/release/echovault-cli"`

### MCP Usage Examples

Once configured, your AI assistant can:

```
"List my recent AI chat sessions"
→ vault(action="list", limit=20)

"Find sessions about authentication"
→ vault(action="search", query="authentication")

"Show me the full conversation from that Cursor session"
→ vault(action="read", source="cursor", session_id="abc123")

"Find code related to database migrations"
→ vault(action="semantic_search", query="database migration setup")
```

---

## Desktop App

### Download & Install

#### Quick Install (Recommended)

Installs both the Desktop App and CLI (including MCP server).

**Linux/macOS:**

```bash
curl -fsSL https://raw.githubusercontent.com/n24q02m/EchoVault/main/install.sh | bash
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/n24q02m/EchoVault/main/install.ps1 | iex
```

> [!TIP]
> Need CLI only (no desktop app)? See [MCP Server > Install CLI](#1-install-cli).

#### Manual Download

Download from [Releases](https://github.com/n24q02m/EchoVault/releases):

| Platform | File |
|----------|------|
| **Windows (x64)** | `EchoVault_x.x.x_x64-setup.exe` |
| **macOS (Intel)** | `EchoVault_x.x.x_x64.dmg` |
| **macOS (Apple Silicon)** | `EchoVault_x.x.x_aarch64.dmg` |
| **Linux (Debian/Ubuntu)** | `EchoVault_x.x.x_amd64.deb` |
| **Linux (Fedora/RHEL)** | `EchoVault-x.x.x-1.x86_64.rpm` |
| **Linux (Universal)** | `EchoVault_x.x.x_amd64.AppImage` |

The desktop app provides a GUI for extraction, parsing, embedding, search, cloud sync, and settings.

#### CLI-only Release Binaries

Pre-built CLI binaries are also attached to each release:

| Platform | File |
|----------|------|
| **Linux (x64)** | `echovault-cli-linux-x64` |
| **macOS (Intel)** | `echovault-cli-macos-x64` |
| **macOS (Apple Silicon)** | `echovault-cli-macos-arm64` |
| **Windows (x64)** | `echovault-cli-windows-x64.exe` |

---

## CLI Reference

For servers, CI environments, or headless usage. Install via [quick install](#1-install-cli) or download from [Releases](https://github.com/n24q02m/EchoVault/releases).

```bash
echovault-cli <COMMAND>

Commands:
  auth       Authenticate with Google Drive
  sync       Sync vault (pull -> extract -> push)
  extract    Extract sessions from all detected IDEs
  parse      Parse raw sessions into clean Markdown
  embed      Build embedding index for semantic search
  search     Semantic search across embedded conversations
  mcp        Start MCP server on stdio
  intercept  Start interceptor proxy for API traffic capture
  status     Show current status (auth, sync, vault info)
```

### Key Workflows

```bash
# Full pipeline: extract -> parse -> embed -> ready for search/MCP
echovault-cli extract
echovault-cli parse
echovault-cli embed

# Quick search
echovault-cli search "how to setup fastapi middleware" --limit 5

# Cloud sync (requires auth first)
echovault-cli auth
echovault-cli sync
```

---

## Supported Sources

### Extensions (plugins inside host IDEs)

| Source | IDE Support | Storage Format |
|--------|-------------|----------------|
| `vscode-copilot` | VS Code, VS Code Insiders | JSON/JSONL per workspace |
| `cline` | VS Code, Cursor | JSON tasks in globalStorage |
| `continue-dev` | VS Code, JetBrains | JSON sessions in `~/.continue/` |

### Standalone IDEs

| Source | Description | Storage Format |
|--------|-------------|----------------|
| `cursor` | Cursor AI Editor | SQLite database |
| `jetbrains` | IntelliJ, PyCharm, WebStorm, GoLand, etc. | XML workspace files |
| `zed` | Zed Editor | SQLite (zstd compressed) |
| `antigravity` | Google Antigravity IDE | Protobuf + Markdown |

### CLI Tools

| Source | Description | Storage Format |
|--------|-------------|----------------|
| `gemini-cli` | Google Gemini CLI | JSON sessions |
| `claude-code` | Claude Code (Anthropic) | JSONL sessions |
| `codex` | OpenAI Codex CLI | JSONL rollout |
| `opencode` | OpenCode terminal AI | JSON sessions |

---

## Embedding & Search

EchoVault supports hybrid search combining vector similarity and keyword matching.

### Embedding Providers

Configure via Desktop App (Settings) or `~/.config/echovault/echovault.toml`:

| Preset | API Base | Default Model | API Key Required |
|--------|----------|---------------|------------------|
| **Ollama** (default) | `http://localhost:11434/v1` | `nomic-embed-text` | No |
| **OpenAI** | `https://api.openai.com/v1` | `text-embedding-3-small` | Yes |
| **Custom** | Any OpenAI-compatible endpoint | User-defined | Depends |

```toml
# ~/.config/echovault/echovault.toml
[embedding]
preset = "ollama"
api_base = "http://localhost:11434/v1"
model = "nomic-embed-text"
# api_key = ""  # Only needed for OpenAI/custom
```

### Search Pipeline

1. **FTS5 keyword search** — SQLite full-text search on chunks (BM25 ranking)
2. **Vector similarity** — Cosine similarity on embeddings
3. **RRF fusion** — Reciprocal Rank Fusion merges both result sets (alpha=0.6 vector bias)

---

## Development

### Prerequisites

- [mise](https://mise.jdx.dev/) (auto-installs Rust, Node.js, pnpm, uv)

```bash
git clone https://github.com/n24q02m/EchoVault.git
cd EchoVault
mise run setup
```

### Build & Test

```bash
# Rust
cargo build --workspace          # Debug build (all crates)
cargo test --workspace           # Run tests
cargo clippy --workspace         # Lint
cargo fmt --all                  # Format

# Frontend (apps/web)
cd apps/web
pnpm dev                         # Dev server with HMR
pnpm build                       # Production build
pnpm lint                        # Biome lint

# Desktop app
cargo tauri dev                  # Development mode
cargo tauri build                # Production build
```

### Project Structure

```
EchoVault/
  apps/
    core/           # Core library (extractors, parsers, embedding, MCP, storage)
    cli/            # CLI binary (echovault-cli)
    tauri/          # Desktop app (Tauri + React)
    web/            # Frontend (React + TypeScript + Tailwind)
  scripts/          # Build scripts (download rclone, setup dev, etc.)
```

### Crate Features

| Feature | Description | Used By |
|---------|-------------|---------|
| `embedding` | Vector embeddings + hybrid search | CLI, Tauri |
| `mcp` | MCP server (rmcp + stdio transport) | CLI |
| `interceptor` | MITM proxy for API traffic capture | CLI, Tauri |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

MIT - See [LICENSE](LICENSE) for details.
