//! MCP (Model Context Protocol) server for EchoVault.
//!
//! Phase 5 of the EchoVault pipeline:
//! Exposes vault data via MCP tools for AI assistants.
//!
//! Tools (2-tool pattern, minimizing token usage):
//! - `vault` - Unified tool: list, search, read, semantic_search
//! - `help`  - On-demand documentation for the `vault` tool
//!
//! Runs on stdio transport for integration with Claude Desktop, Copilot, etc.

use crate::config::Config;
use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, ServerHandler},
    model::*,
    service::{RequestContext, RoleServer, ServiceExt},
    tool, tool_router,
    transport::io::stdio,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Type alias for MCP error data.
type McpError = rmcp::model::ErrorData;

/// Lazily-loaded config singleton.
static CONFIG: OnceLock<Config> = OnceLock::new();

fn get_config() -> &'static Config {
    CONFIG.get_or_init(|| Config::load_default().unwrap_or_default())
}

/// EchoVault MCP Server.
#[derive(Clone)]
pub struct EchoVaultServer {
    vault_dir: PathBuf,
    tool_router: ToolRouter<Self>,
}

// ============ TOOL PARAMETERS ============

#[derive(Debug, Deserialize, JsonSchema)]
struct VaultParams {
    /// Action to perform: "list", "search", "read", "semantic_search"
    action: String,
    /// Search query (required for "search" and "semantic_search" actions)
    query: Option<String>,
    /// Session ID (required for "read" action)
    session_id: Option<String>,
    /// Source filter (e.g., "vscode-copilot", "cursor"). Used by "list" and "read".
    source: Option<String>,
    /// Maximum number of results (default varies by action)
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HelpParams {
    /// Tool name to get help for (default: "vault")
    tool_name: Option<String>,
}

// ============ TIER 2 DOCUMENTATION ============

const VAULT_HELP: &str = r#"# EchoVault `vault` Tool

Unified interface to AI chat session history from 12+ sources.

## Actions

### `list` - Browse sessions
| Param    | Required | Default | Description |
|----------|----------|---------|-------------|
| source   | no       | all     | Filter by source name |
| limit    | no       | 50      | Max results |

Returns: `[source] title | ws: workspace | date: YYYY-MM-DD HH:MM | id: session_id`

### `search` - Full-text search (FTS5)
| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| query | yes      | -       | FTS5 query (titles + workspace names) |
| limit | no       | 20      | Max results |

Returns: Matching sessions sorted by relevance.

### `read` - Read full session content
| Param      | Required | Description |
|------------|----------|-------------|
| source     | yes      | Source name (from list/search output) |
| session_id | yes      | Session ID (from list/search output) |

Returns: Full Markdown with YAML frontmatter.

### `semantic_search` - Hybrid search (vector + keyword)
| Param | Required | Default | Description |
|-------|----------|---------|-------------|
| query | yes      | -       | Natural language query |
| limit | no       | 10      | Max results |

Returns: Ranked results with relevance score and content snippet.
Requires embeddings to be generated first (Settings > Build Index).

## Sources
copilot, cursor, cline, continue-dev, jetbrains, zed, antigravity,
gemini-cli, claude-code, codex, opencode

## Workflow
1. `vault(action="list")` to browse available sessions
2. `vault(action="search", query="keyword")` to find by title
3. `vault(action="read", source="...", session_id="...")` to read content
4. `vault(action="semantic_search", query="natural language")` for semantic match
"#;

// ============ TOOL IMPLEMENTATIONS ============

#[tool_router]
impl EchoVaultServer {
    /// Create a new MCP server with the given vault directory.
    pub fn new(vault_dir: PathBuf) -> Self {
        Self {
            vault_dir,
            tool_router: Self::tool_router(),
        }
    }

    /// Create from default config (lazy loaded).
    pub fn from_config() -> Result<Self, McpError> {
        let config = get_config();
        Ok(Self::new(config.vault_path.clone()))
    }

    #[tool(
        name = "vault",
        description = "Access AI chat sessions. Actions: list (browse), search (FTS5), read (full content), semantic_search (hybrid vector+keyword). Use `help` tool for full documentation.",
        annotations(read_only_hint = true)
    )]
    async fn vault(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<VaultParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = params.0;
        let vault_dir = self.vault_dir.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
            match p.action.as_str() {
                "list" => vault_list(&vault_dir, p.source.as_deref(), p.limit.unwrap_or(50)),
                "search" => {
                    let query = p.query.as_deref().unwrap_or_default();
                    if query.is_empty() {
                        return Ok("Error: 'query' parameter is required for 'search' action".to_string());
                    }
                    vault_search(&vault_dir, query, p.limit.unwrap_or(20))
                }
                "read" => {
                    let source = match p.source.as_deref() {
                        Some(s) => s,
                        None => return Ok("Error: 'source' parameter is required for 'read' action".to_string()),
                    };
                    let session_id = match p.session_id.as_deref() {
                        Some(s) => s,
                        None => return Ok("Error: 'session_id' parameter is required for 'read' action".to_string()),
                    };
                    vault_read(&vault_dir, source, session_id)
                }
                "semantic_search" => {
                    let query = p.query.as_deref().unwrap_or_default();
                    if query.is_empty() {
                        return Ok("Error: 'query' parameter is required for 'semantic_search' action".to_string());
                    }
                    vault_semantic_search(&vault_dir, query, p.limit.unwrap_or(10))
                }
                other => Ok(format!(
                    "Error: Unknown action '{}'. Valid actions: list, search, read, semantic_search",
                    other
                )),
            }
        })
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Always return Ok with text content — errors as friendly messages
        let text = result.unwrap_or_else(|e| format!("Error: {}", e));
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        name = "help",
        description = "Get full documentation for EchoVault tools. Returns detailed usage, parameters, examples.",
        annotations(read_only_hint = true)
    )]
    async fn help(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<HelpParams>,
    ) -> Result<CallToolResult, McpError> {
        let tool_name = params.0.tool_name.unwrap_or_else(|| "vault".to_string());
        let doc = match tool_name.as_str() {
            "vault" => VAULT_HELP.to_string(),
            "help" => "The `help` tool returns documentation for EchoVault tools.\n\nUsage: help(tool_name=\"vault\")".to_string(),
            other => format!("Documentation not found for '{}'. Available: vault, help", other),
        };
        Ok(CallToolResult::success(vec![Content::text(doc)]))
    }
}

// ============ ACTION IMPLEMENTATIONS ============

fn vault_list(vault_dir: &Path, source: Option<&str>, limit: usize) -> Result<String, String> {
    let index = crate::storage::SessionIndex::open(vault_dir).map_err(|e| e.to_string())?;

    let sessions = if let Some(src) = source {
        index.filter_by_source(src, limit)
    } else {
        index.list(limit, 0)
    }
    .map_err(|e| e.to_string())?;

    if sessions.is_empty() {
        return Ok("No sessions found.".to_string());
    }

    let mut output = format!("Found {} sessions:\n\n", sessions.len());
    for s in &sessions {
        let title = s.title.as_deref().unwrap_or("(untitled)");
        let ws = s.workspace_name.as_deref().unwrap_or("-");
        let date = s
            .created_at
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "-".to_string());
        output.push_str(&format!(
            "- [{}] {} | ws: {} | date: {} | id: {}\n",
            s.source, title, ws, date, s.id
        ));
    }
    Ok(output)
}

fn vault_search(vault_dir: &Path, query: &str, limit: usize) -> Result<String, String> {
    let index = crate::storage::SessionIndex::open(vault_dir).map_err(|e| e.to_string())?;
    let sessions = index.search(query, limit).map_err(|e| e.to_string())?;

    if sessions.is_empty() {
        return Ok(format!("No results for '{}'.", query));
    }

    let mut output = format!("Search '{}': {} results\n\n", query, sessions.len());
    for s in &sessions {
        let title = s.title.as_deref().unwrap_or("(untitled)");
        let ws = s.workspace_name.as_deref().unwrap_or("-");
        output.push_str(&format!(
            "- [{}] {} | ws: {} | id: {}\n",
            s.source, title, ws, s.id
        ));
    }
    Ok(output)
}

fn vault_read(vault_dir: &Path, source: &str, session_id: &str) -> Result<String, String> {
    let parsed_path = vault_dir
        .join("parsed")
        .join(source)
        .join(format!("{}.md", session_id));

    if !parsed_path.exists() {
        return Ok(format!("Session not found: {}/{}", source, session_id));
    }

    std::fs::read_to_string(&parsed_path).map_err(|e| e.to_string())
}

fn vault_semantic_search(vault_dir: &Path, query: &str, limit: usize) -> Result<String, String> {
    #[cfg(feature = "embedding")]
    {
        let config = get_config();
        let embedding_config = crate::embedding::EmbeddingConfig {
            api_base: config.embedding.api_base.clone(),
            api_key: config.embedding.api_key.clone(),
            model: config.embedding.model.clone(),
            chunk_size: config.embedding.chunk_size,
            chunk_overlap: config.embedding.chunk_overlap,
            batch_size: config.embedding.batch_size,
        };

        let results = crate::embedding::search_similar(&embedding_config, vault_dir, query, limit)
            .map_err(|e| e.to_string())?;

        if results.is_empty() {
            return Ok(format!(
                "No semantic results for '{}'. Ensure embeddings are built (echovault embed).",
                query
            ));
        }

        let mut output = format!("Semantic search '{}': {} results\n\n", query, results.len());
        for (i, r) in results.iter().enumerate() {
            let title = r.title.as_deref().unwrap_or("(untitled)");
            output.push_str(&format!(
                "{}. [{}] {} (score: {:.3})\n",
                i + 1,
                r.source,
                title,
                r.score
            ));
            let snippet: String = r.chunk_content.chars().take(300).collect();
            output.push_str(&format!("   {}\n\n", snippet));
        }
        Ok(output)
    }

    #[cfg(not(feature = "embedding"))]
    {
        let _ = (query, limit, vault_dir);
        Ok("Embedding feature not enabled. Rebuild with --features embedding".to_string())
    }
}

// ============ SERVER HANDLER ============

impl ServerHandler for EchoVaultServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: None }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "echovault".to_string(),
                title: Some("EchoVault MCP Server".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "EchoVault: AI chat history vault. Use vault(action=\"list\") to browse, \
                 vault(action=\"search\", query=\"...\") for FTS, \
                 vault(action=\"read\", source=\"...\", session_id=\"...\") for content, \
                 vault(action=\"semantic_search\", query=\"...\") for semantic search. \
                 Call help() for full documentation."
                    .to_string(),
            ),
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let tool_context = ToolCallContext::new(self, request, context);
        async move { self.tool_router.call(tool_context).await }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let tools = self.tool_router.list_all();
        std::future::ready(Ok(ListToolsResult {
            tools,
            ..Default::default()
        }))
    }
}

/// Run the MCP server on stdio transport.
///
/// This is the main entry point for the MCP server binary/CLI command.
pub async fn run_server() -> anyhow::Result<()> {
    let server = EchoVaultServer::from_config()
        .map_err(|e| anyhow::anyhow!("Failed to create MCP server: {:?}", e))?;

    let (stdin, stdout) = stdio();
    let service = server
        .serve((stdin, stdout))
        .await
        .map_err(|e| anyhow::anyhow!("MCP server failed to start: {:?}", e))?;

    service
        .waiting()
        .await
        .map_err(|e| anyhow::anyhow!("MCP server error: {:?}", e))?;

    Ok(())
}
