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
        antigravity::AntigravityExtractor, cline::ClineExtractor, cursor::CursorExtractor,
        vscode_copilot::VSCodeCopilotExtractor, Extractor, SessionFile,
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

    /// Sync vault with cloud (pull → extract → push)
    Sync,

    /// Extract sessions from IDE into vault (without syncing to cloud)
    Extract,

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
                    if !matches!(extension, Some("json") | Some("pb") | Some("md")) {
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

                    let (title, workspace_name, created_at) = if extension == Some("json") {
                        match fs::read_to_string(&file_path) {
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
