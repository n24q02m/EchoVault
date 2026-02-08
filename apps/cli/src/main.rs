//! EchoVault CLI - Sync AI chat history for unsupported OS
//!
//! This CLI provides the same functionality as the desktop app but without
//! Tauri dependencies, making it compatible with older Linux distributions
//! like Ubuntu 20.04.
//!
//! Usage:
//!   echovault-cli auth     - Authenticate with Google Drive
//!   echovault-cli sync     - Sync vault (pull → extract → push)
//!   echovault-cli extract  - Extract sessions from IDE only
//!   echovault-cli status   - Show auth and sync status

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use echovault_core::{
    extractors::{
        aider::AiderExtractor, antigravity::AntigravityExtractor, claude_code::ClaudeCodeExtractor,
        cline::ClineExtractor, codex::CodexExtractor, continue_dev::ContinueDevExtractor,
        cursor::CursorExtractor, gemini_cli::GeminiCliExtractor, jetbrains::JetBrainsExtractor,
        opencode::OpenCodeExtractor, vscode_copilot::VSCodeCopilotExtractor, zed::ZedExtractor,
        Extractor, SessionFile,
    },
    storage::{SessionEntry, VaultDb},
    sync::{AuthStatus, RcloneProvider, SyncOptions, SyncProvider},
    Config,
};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// EchoVault CLI - Black box for your AI conversations
#[derive(Parser)]
#[command(name = "echovault-cli")]
#[command(about = "Sync AI chat history for unsupported OS", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with Google Drive (required before first sync)
    Auth,

    /// Sync vault with cloud (pull -> extract -> push)
    Sync,

    /// Extract sessions from IDE into vault (without syncing to cloud)
    Extract,

    /// Parse raw sessions into clean Markdown
    Parse,

    /// Start interceptor proxy for capturing API traffic
    Intercept {
        /// Port to listen on (default: 18080)
        #[arg(short, long, default_value = "18080")]
        port: u16,
    },

    /// Embed parsed conversations for semantic search
    Embed,

    /// Semantic search across embedded conversations
    Search {
        /// Search query text
        query: String,

        /// Maximum number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Start MCP (Model Context Protocol) server on stdio
    Mcp,

    /// Show current status (auth, last sync, etc.)
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("echovault_cli={}", log_level).parse().unwrap())
                .add_directive(format!("echovault_core={}", log_level).parse().unwrap()),
        )
        .with_target(false)
        .init();

    match cli.command {
        Commands::Auth => cmd_auth(),
        Commands::Sync => cmd_sync(),
        Commands::Extract => cmd_extract(),
        Commands::Parse => cmd_parse(),
        Commands::Intercept { port } => cmd_intercept(port),
        Commands::Embed => cmd_embed(),
        Commands::Search { query, limit } => cmd_search(&query, limit),
        Commands::Mcp => cmd_mcp(),
        Commands::Status => cmd_status(),
    }
}

// ============ AUTH COMMAND ============

fn cmd_auth() -> Result<()> {
    println!("{}", "🔐 EchoVault Authentication".bold().cyan());
    println!();

    let mut provider = RcloneProvider::new();

    // Check if already authenticated
    if provider.is_authenticated() {
        println!("{}", "✓ Already authenticated with Google Drive".green());
        return Ok(());
    }

    println!("Starting Google Drive authentication...");
    println!();

    // Start auth - this will open browser
    match provider.start_auth()? {
        AuthStatus::Authenticated => {
            println!("{}", "✓ Authentication successful!".green());
        }
        AuthStatus::Pending { verify_url, .. } => {
            println!("Please open the following URL in your browser:");
            println!("{}", verify_url.blue().underline());
            println!();
            println!(
                "After completing authentication in browser, the CLI will detect it automatically."
            );

            // Poll for completion
            loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                match provider.complete_auth()? {
                    AuthStatus::Authenticated => {
                        println!();
                        println!("{}", "✓ Authentication successful!".green());
                        break;
                    }
                    AuthStatus::Pending { .. } => {
                        print!(".");
                        std::io::Write::flush(&mut std::io::stdout())?;
                    }
                    AuthStatus::Error(e) => {
                        return Err(anyhow::anyhow!("Authentication failed: {}", e));
                    }
                    AuthStatus::NotAuthenticated => {
                        // Continue polling
                    }
                }
            }
        }
        AuthStatus::Error(e) => {
            return Err(anyhow::anyhow!("Authentication failed: {}", e));
        }
        AuthStatus::NotAuthenticated => {
            return Err(anyhow::anyhow!("Authentication was not started"));
        }
    }

    println!();
    println!(
        "You can now run {} to sync your vault.",
        "echovault-cli sync".cyan()
    );

    Ok(())
}

// ============ SYNC COMMAND ============

fn cmd_sync() -> Result<()> {
    println!("{}", "🔄 EchoVault Sync".bold().cyan());
    println!();

    let provider = RcloneProvider::new();

    // Check auth
    if !provider.is_authenticated() {
        println!(
            "{}",
            "✗ Not authenticated. Please run 'echovault-cli auth' first.".red()
        );
        return Ok(());
    }

    // Ensure config exists
    let config = ensure_config()?;

    let vault_dir = &config.vault_path;
    println!("Vault: {}", vault_dir.display().to_string().dimmed());
    println!();

    // Step 1: Pull from remote
    println!("{}", "Step 1/3: Pulling from Google Drive...".bold());
    let options = SyncOptions::default();
    match provider.pull(vault_dir, &options) {
        Ok(result) => {
            if result.has_changes {
                println!(
                    "  {} new files, {} updated",
                    result.new_files.to_string().green(),
                    result.updated_files.to_string().yellow()
                );
            } else {
                println!("  {}", "No new changes from remote".dimmed());
            }
        }
        Err(e) => {
            println!(
                "  {} (continuing anyway)",
                format!("Warning: {}", e).yellow()
            );
        }
    }

    // Step 1.5: Import pulled sessions into vault.db
    let import_count = import_vault_sessions(vault_dir)?;
    if import_count > 0 {
        println!(
            "  Imported {} sessions from other machines",
            import_count.to_string().green()
        );
    }

    println!();

    // Step 2: Extract from local IDEs
    println!("{}", "Step 2/3: Extracting from local IDEs...".bold());
    let extracted = ingest_sessions(vault_dir)?;
    if extracted {
        println!("  {}", "Sessions extracted successfully".green());
    } else {
        println!("  {}", "All sessions already up-to-date".dimmed());
    }

    println!();

    // Step 3: Push to remote
    println!("{}", "Step 3/3: Pushing to Google Drive...".bold());
    match provider.push(vault_dir, &options) {
        Ok(result) => {
            println!("  {} files pushed", result.files_pushed.to_string().green());
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Push failed: {}", e));
        }
    }

    println!();
    println!("{}", "✓ Sync complete!".green().bold());

    Ok(())
}

// ============ EXTRACT COMMAND ============

fn cmd_extract() -> Result<()> {
    println!("{}", "📁 EchoVault Extract".bold().cyan());
    println!();

    // Ensure config exists
    let config = ensure_config()?;

    let vault_dir = &config.vault_path;
    println!("Vault: {}", vault_dir.display().to_string().dimmed());
    println!();

    let extracted = ingest_sessions(vault_dir)?;
    if extracted {
        println!();
        println!("{}", "✓ Extraction complete!".green().bold());
    } else {
        println!("{}", "✓ All sessions already up-to-date".green());
    }

    Ok(())
}

// ============ PARSE COMMAND ============

fn cmd_parse() -> Result<()> {
    println!("{}", "📝 EchoVault Parse".bold().cyan());
    println!();

    let config = ensure_config()?;
    let vault_dir = &config.vault_path;
    let sessions_dir = vault_dir.join("sessions");

    if !sessions_dir.exists() {
        println!(
            "{}",
            "No sessions found. Run 'echovault-cli extract' first.".yellow()
        );
        return Ok(());
    }

    println!("Vault: {}", vault_dir.display().to_string().dimmed());
    println!();

    use echovault_core::parsers::{all_parsers, markdown_writer, parse_vault_source};

    let parsers = all_parsers();
    let parsed_dir = vault_dir.join("parsed");

    let mut total_parsed = 0usize;
    let mut total_errors = 0usize;
    let mut total_skipped = 0usize;

    for parser in &parsers {
        let source_dir = sessions_dir.join(parser.source_name());
        if !source_dir.exists() {
            continue;
        }

        print!("  Parsing {}...", parser.source_name());

        let (conversations, errors) = parse_vault_source(parser.as_ref(), &sessions_dir);

        let mut source_parsed = 0;
        for (path, err) in &errors {
            tracing::warn!("Error parsing {:?}: {}", path, err);
        }

        for conv in &conversations {
            let output_path = parsed_dir
                .join(&conv.source)
                .join(format!("{}.md", conv.id));

            // Skip if already parsed and source hasn't changed
            if output_path.exists() {
                total_skipped += 1;
                continue;
            }

            match markdown_writer::write_markdown(conv, &output_path) {
                Ok(()) => {
                    source_parsed += 1;
                    total_parsed += 1;
                }
                Err(e) => {
                    tracing::warn!("Error writing {:?}: {}", output_path, e);
                    total_errors += 1;
                }
            }
        }

        total_errors += errors.len();

        println!(
            " {} parsed, {} errors",
            source_parsed.to_string().green(),
            errors.len().to_string().red()
        );
    }

    println!();
    println!(
        "{}",
        format!(
            "Complete: {} parsed, {} skipped, {} errors",
            total_parsed, total_skipped, total_errors
        )
        .green()
        .bold()
    );

    Ok(())
}

// ============ INTERCEPT COMMAND ============

fn cmd_intercept(port: u16) -> Result<()> {
    use echovault_core::interceptor::{self, InterceptorConfig, InterceptorState};

    println!("{}", "Interceptor Proxy".bold().cyan());
    println!();

    let config = InterceptorConfig {
        port,
        ..Default::default()
    };

    // Show setup instructions
    let instructions = interceptor::proxy_setup_instructions(&config);
    println!("{}", instructions);

    println!("Target domains:");
    for domain in &config.target_domains {
        println!("  - {}", domain.yellow());
    }
    println!();
    println!("Starting proxy on port {}...", port.to_string().cyan());
    println!("{}", "Press Ctrl+C to stop.".dimmed());
    println!();

    // Run the async proxy in a tokio runtime
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let handle = interceptor::start(config).await?;

        match handle.state() {
            InterceptorState::Running { port } => {
                println!(
                    "{}",
                    format!("Proxy running on http://127.0.0.1:{}", port)
                        .green()
                        .bold()
                );
            }
            InterceptorState::Error(e) => {
                println!("{}", format!("Error: {}", e).red());
                return Ok(());
            }
            InterceptorState::Stopped => {
                println!("{}", "Proxy stopped unexpectedly".red());
                return Ok(());
            }
        }

        // Wait for Ctrl+C
        tokio::signal::ctrl_c().await?;
        println!();
        println!("Shutting down...");
        handle.stop();
        // Give proxy a moment to clean up
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        println!("{}", "Proxy stopped.".green());

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

// ============ EMBED COMMAND ============

fn cmd_embed() -> Result<()> {
    println!("{}", "Embedding Sessions".bold().cyan());
    println!();

    let config = ensure_config()?;
    let vault_dir = &config.vault_path;

    println!("Vault: {}", vault_dir.display().to_string().dimmed());
    println!(
        "API:   {} ({})",
        config.embedding.api_base.dimmed(),
        config.embedding.model.yellow()
    );
    println!();

    let embedding_config = echovault_core::embedding::EmbeddingConfig {
        api_base: config.embedding.api_base,
        api_key: config.embedding.api_key,
        model: config.embedding.model,
        chunk_size: config.embedding.chunk_size,
        chunk_overlap: config.embedding.chunk_overlap,
        batch_size: config.embedding.batch_size,
    };

    println!("Processing conversations...");
    match echovault_core::embedding::embed_vault(&embedding_config, vault_dir) {
        Ok(result) => {
            println!();
            println!(
                "{}",
                format!(
                    "Complete: {} processed, {} chunks, {} skipped, {} errors",
                    result.sessions_processed,
                    result.chunks_created,
                    result.sessions_skipped,
                    result.errors.len()
                )
                .green()
                .bold()
            );

            if !result.errors.is_empty() {
                println!();
                println!("{}", "Errors:".red());
                for (id, err) in &result.errors {
                    println!("  {} - {}", id.dimmed(), err);
                }
            }
        }
        Err(e) => {
            println!("{}", format!("Embedding failed: {}", e).red());
        }
    }

    Ok(())
}

// ============ SEARCH COMMAND ============

fn cmd_search(query: &str, limit: usize) -> Result<()> {
    println!("{}", "Semantic Search".bold().cyan());
    println!();

    let config = ensure_config()?;
    let vault_dir = &config.vault_path;

    let embedding_config = echovault_core::embedding::EmbeddingConfig {
        api_base: config.embedding.api_base,
        api_key: config.embedding.api_key,
        model: config.embedding.model,
        chunk_size: config.embedding.chunk_size,
        chunk_overlap: config.embedding.chunk_overlap,
        batch_size: config.embedding.batch_size,
    };

    println!("Query: {}", query.yellow());
    println!();

    match echovault_core::embedding::search_similar(&embedding_config, vault_dir, query, limit) {
        Ok(results) => {
            if results.is_empty() {
                println!(
                    "{}",
                    "No results found. Run 'echovault-cli embed' first.".yellow()
                );
                return Ok(());
            }

            for (i, r) in results.iter().enumerate() {
                let title = r.title.as_deref().unwrap_or("(untitled)");
                println!(
                    "{}. {} [{}] (score: {:.3})",
                    (i + 1).to_string().bold(),
                    title.green(),
                    r.source.dimmed(),
                    r.score
                );
                // Show snippet (first 200 chars)
                let snippet: String = r.chunk_content.chars().take(200).collect();
                println!("   {}", snippet.dimmed());
                println!("   ID: {}", r.session_id.dimmed());
                println!();
            }
        }
        Err(e) => {
            println!("{}", format!("Search failed: {}", e).red());
        }
    }

    Ok(())
}

// ============ MCP COMMAND ============

fn cmd_mcp() -> Result<()> {
    // MCP server runs on stdio - no user output to stdout (it's protocol data)
    // Log to stderr only
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { echovault_core::mcp::run_server().await })?;
    Ok(())
}

// ============ STATUS COMMAND ============

fn cmd_status() -> Result<()> {
    println!("{}", "📊 EchoVault Status".bold().cyan());
    println!();

    // Auth status
    let provider = RcloneProvider::new();
    let auth_status = if provider.is_authenticated() {
        "Authenticated".green().to_string()
    } else {
        "Not authenticated".red().to_string()
    };
    println!("Auth:     {}", auth_status);

    // Config status
    match Config::load_default() {
        Ok(config) => {
            println!("Vault:    {}", config.vault_path.display());

            // Count sessions in vault
            if let Ok(vault_db) = VaultDb::open(&config.vault_path) {
                if let Ok(sessions) = vault_db.get_all_sessions() {
                    println!("Sessions: {}", sessions.len().to_string().cyan());

                    // Count by source
                    let mut by_source: std::collections::HashMap<String, usize> =
                        std::collections::HashMap::new();
                    for session in &sessions {
                        *by_source.entry(session.source.clone()).or_insert(0) += 1;
                    }
                    for (source, count) in by_source {
                        println!("  - {}: {}", source, count);
                    }
                }
            }
        }
        Err(_) => {
            println!("Config:   {}", "Not configured".yellow());
            println!();
            println!("Run {} to set up.", "echovault-cli auth".cyan());
        }
    }

    Ok(())
}

// ============ HELPER FUNCTIONS ============

/// Ensure config exists, create default if not
fn ensure_config() -> Result<Config> {
    match Config::load_default() {
        Ok(c) if c.setup_complete => Ok(c),
        _ => {
            // Create default config
            println!("Creating default configuration...");
            let vault_path = dirs::data_local_dir()
                .context("Cannot find local data directory")?
                .join("echovault")
                .join("vault");
            fs::create_dir_all(&vault_path)?;

            let config = Config {
                vault_path,
                setup_complete: true,
                ..Config::default()
            };
            config.save_default()?;
            Ok(config)
        }
    }
}

/// Import sessions from vault/sessions folder into vault.db
fn import_vault_sessions(vault_dir: &Path) -> Result<usize> {
    let sessions_dir = vault_dir.join("sessions");
    if !sessions_dir.exists() {
        return Ok(0);
    }

    let mut vault_db = VaultDb::open(vault_dir)?;
    let existing_sessions = vault_db.get_all_sessions()?;
    let existing_mtimes: std::collections::HashMap<String, u64> = existing_sessions
        .iter()
        .map(|s| (s.id.clone(), s.mtime))
        .collect();

    let mut sessions_to_import: Vec<SessionEntry> = Vec::new();

    // Scan all subdirectories
    if let Ok(entries) = fs::read_dir(&sessions_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let source_dir = entry.path();
            if !source_dir.is_dir() {
                continue;
            }

            let source_name = source_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            if let Ok(files) = fs::read_dir(&source_dir) {
                for file in files.filter_map(|f| f.ok()) {
                    let file_path = file.path();

                    let extension = file_path.extension().and_then(|e| e.to_str());
                    if !matches!(
                        extension,
                        Some("json") | Some("jsonl") | Some("pb") | Some("md")
                    ) {
                        continue;
                    }

                    let session_id = file_path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    if session_id.is_empty() {
                        continue;
                    }

                    let metadata = match fs::metadata(&file_path) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    let file_mtime = metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    if let Some(&existing_mtime) = existing_mtimes.get(&session_id) {
                        if existing_mtime >= file_mtime {
                            continue;
                        }
                    }

                    let (title, workspace_name, created_at) = if extension == Some("json")
                        || extension == Some("jsonl")
                    {
                        match extension {
                            Some("jsonl") => {
                                // JSONL: read first line, parse v.customTitle/v.creationDate
                                use std::io::BufRead;
                                match std::fs::File::open(&file_path) {
                                    Ok(file) => {
                                        let reader = std::io::BufReader::new(file);
                                        if let Some(Ok(first_line)) = reader.lines().next() {
                                            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(
                                                &first_line,
                                            ) {
                                                let v = obj.get("v");
                                                let title = v
                                                    .and_then(|v| v.get("customTitle"))
                                                    .and_then(|v| v.as_str())
                                                    .map(|s| s.to_string());
                                                let created = v
                                                        .and_then(|v| v.get("creationDate"))
                                                        .and_then(|v| v.as_i64())
                                                        .map(|ts| {
                                                            chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ts)
                                                                .map(|d| d.to_rfc3339())
                                                                .unwrap_or_default()
                                                        });
                                                (title, None, created)
                                            } else {
                                                (None, None, None)
                                            }
                                        } else {
                                            (None, None, None)
                                        }
                                    }
                                    Err(_) => (None, None, None),
                                }
                            }
                            _ => match fs::read_to_string(&file_path) {
                                Ok(content) => {
                                    if let Ok(json) =
                                        serde_json::from_str::<serde_json::Value>(&content)
                                    {
                                        let title = json
                                            .get("title")
                                            .or_else(|| json.get("name"))
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());
                                        let workspace = json
                                            .get("workspace_name")
                                            .or_else(|| json.get("workspaceName"))
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());
                                        let created = json
                                            .get("created_at")
                                            .or_else(|| json.get("createdAt"))
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());
                                        (title, workspace, created)
                                    } else {
                                        (None, None, None)
                                    }
                                }
                                Err(_) => (None, None, None),
                            },
                        }
                    } else {
                        (None, None, None)
                    };

                    let ext = extension.unwrap_or("json");
                    let vault_path = format!("sessions/{}/{}.{}", source_name, session_id, ext);

                    sessions_to_import.push(SessionEntry {
                        id: session_id,
                        source: source_name.clone(),
                        mtime: file_mtime,
                        file_size: metadata.len(),
                        title,
                        workspace_name,
                        created_at,
                        vault_path,
                        original_path: file_path.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }

    let import_count = sessions_to_import.len();

    if import_count > 0 {
        vault_db.upsert_batch(&sessions_to_import)?;
    }

    Ok(import_count)
}

/// Ingest sessions from local extractors into vault
fn ingest_sessions(vault_dir: &Path) -> Result<bool> {
    let mut all_sessions: Vec<SessionFile> = Vec::new();

    // Collect sessions from all extractors
    println!("  Scanning VS Code Copilot...");
    let vscode_extractor = VSCodeCopilotExtractor::new();
    if let Ok(locations) = vscode_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = vscode_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning Antigravity...");
    let antigravity_extractor = AntigravityExtractor::new();
    if let Ok(locations) = antigravity_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = antigravity_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning Cursor...");
    let cursor_extractor = CursorExtractor::new();
    if let Ok(locations) = cursor_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = cursor_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning Cline...");
    let cline_extractor = ClineExtractor::new();
    if let Ok(locations) = cline_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = cline_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning Gemini CLI...");
    let gemini_extractor = GeminiCliExtractor::new();
    if let Ok(locations) = gemini_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = gemini_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning Claude Code...");
    let claude_extractor = ClaudeCodeExtractor::new();
    if let Ok(locations) = claude_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = claude_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning Aider...");
    let aider_extractor = AiderExtractor::new();
    if let Ok(locations) = aider_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = aider_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning Codex...");
    let codex_extractor = CodexExtractor::new();
    if let Ok(locations) = codex_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = codex_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning Continue.dev...");
    let continue_dev_extractor = ContinueDevExtractor::new();
    if let Ok(locations) = continue_dev_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = continue_dev_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning OpenCode...");
    let opencode_extractor = OpenCodeExtractor::new();
    if let Ok(locations) = opencode_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = opencode_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning Zed...");
    let zed_extractor = ZedExtractor::new();
    if let Ok(locations) = zed_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = zed_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    println!("  Scanning JetBrains AI...");
    let jetbrains_extractor = JetBrainsExtractor::new();
    if let Ok(locations) = jetbrains_extractor.find_storage_locations() {
        for location in &locations {
            if let Ok(files) = jetbrains_extractor.list_session_files(location) {
                all_sessions.extend(files);
            }
        }
    }

    let total_sessions = all_sessions.len();
    println!("  Found {} sessions total", total_sessions);

    if total_sessions == 0 {
        return Ok(false);
    }

    // Open vault database
    let vault_db = VaultDb::open(vault_dir)?;
    let sessions_dir = vault_dir.join("sessions");
    fs::create_dir_all(&sessions_dir)?;

    // Filter sessions that need processing
    let sessions_to_process: Vec<_> = all_sessions
        .into_iter()
        .filter_map(|session| {
            let source_path = &session.metadata.original_path;
            let file_size = session.metadata.file_size;

            let metadata = match fs::metadata(source_path) {
                Ok(m) => m,
                Err(_) => return None,
            };

            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let should_process = match vault_db.get_session_mtime(&session.metadata.id) {
                Ok(Some(cached_mtime)) => mtime > cached_mtime,
                Ok(None) => true,
                Err(_) => true,
            };

            if should_process {
                Some((session, mtime, file_size))
            } else {
                None
            }
        })
        .collect();

    let to_process = sessions_to_process.len();
    let skipped = total_sessions - to_process;

    if to_process == 0 {
        return Ok(false);
    }

    println!(
        "  Processing {} sessions ({} up-to-date)",
        to_process, skipped
    );

    // Progress bar
    let pb = ProgressBar::new(to_process as u64);
    pb.set_style(
        ProgressStyle::with_template("  [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")
            .unwrap()
            .progress_chars("█▓░"),
    );

    let processed = AtomicUsize::new(0);
    let errors = Mutex::new(Vec::<String>::new());
    let new_entries = Mutex::new(Vec::<SessionEntry>::new());

    // Process in parallel
    sessions_to_process
        .par_iter()
        .for_each(|(session, mtime, file_size)| {
            let source_path = &session.metadata.original_path;
            let dest_dir = sessions_dir.join(&session.metadata.source);

            if let Err(e) = fs::create_dir_all(&dest_dir) {
                errors
                    .lock()
                    .unwrap()
                    .push(format!("Failed to create dir {:?}: {}", dest_dir, e));
                return;
            }

            let extension = source_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("json");

            let dest_path = if session.metadata.id.ends_with(extension) {
                dest_dir.join(&session.metadata.id)
            } else {
                dest_dir.join(format!("{}.{}", session.metadata.id, extension))
            };

            if let Some(parent) = dest_path.parent() {
                if !parent.exists() {
                    let _ = fs::create_dir_all(parent);
                }
            }

            if let Err(e) = fs::copy(source_path, &dest_path) {
                errors
                    .lock()
                    .unwrap()
                    .push(format!("Failed to copy {}: {}", session.metadata.id, e));
                return;
            }

            new_entries.lock().unwrap().push(SessionEntry {
                id: session.metadata.id.clone(),
                source: session.metadata.source.clone(),
                mtime: *mtime,
                file_size: *file_size,
                title: session.metadata.title.clone(),
                workspace_name: session.metadata.workspace_name.clone(),
                created_at: session.metadata.created_at.map(|d| d.to_rfc3339()),
                vault_path: dest_path.to_string_lossy().to_string(),
                original_path: source_path.to_string_lossy().to_string(),
            });

            processed.fetch_add(1, Ordering::Relaxed);
            pb.inc(1);
        });

    pb.finish();

    // Update vault.db
    let entries = new_entries.into_inner().unwrap();
    if !entries.is_empty() {
        for entry in &entries {
            if let Err(e) = vault_db.upsert_session(entry) {
                tracing::warn!("Failed to upsert {}: {}", entry.id, e);
            }
        }
        if let Err(e) = vault_db.log_sync("ingest", Some(&format!("{} sessions", entries.len()))) {
            tracing::warn!("Failed to log sync: {}", e);
        }
    }

    Ok(true)
}
