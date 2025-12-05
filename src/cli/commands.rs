//! Command implementations cho EchoVault CLI.
//!
//! Các commands chính:
//! - init: Khởi tạo vault với GitHub OAuth và encryption key
//! - scan: Quét và liệt kê tất cả chat sessions có sẵn
//! - extract: Copy raw JSON files vào vault (KHÔNG format)
//! - sync: Encrypt và push lên GitHub

use crate::config::{default_config_path, Config};
use crate::crypto::key_derivation::{derive_key_new, SALT_LEN};
use crate::crypto::Encryptor;
use crate::extractors::{vscode_copilot::VSCodeCopilotExtractor, Extractor};
use crate::storage::SessionIndex;
use crate::sync::oauth::{
    load_credentials_from_file, save_credentials_to_file, OAuthCredentials, OAuthDeviceFlow,
};
use crate::sync::GitSync;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::io::{self, Write};
use std::path::PathBuf;

/// Khởi tạo vault mới
pub fn init(remote: Option<String>) -> Result<()> {
    println!("{}", "Initializing EchoVault...".cyan().bold());

    // Load hoặc tạo config mới
    let config_path = default_config_path();
    let mut config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        Config::new()
    };

    // Tạo vault directory
    let vault_dir = config.vault_dir().to_path_buf();
    std::fs::create_dir_all(&vault_dir)?;
    println!("  {} {}", "Vault directory:".green(), vault_dir.display());

    // Init git repository
    let _git = GitSync::open_or_init(&vault_dir)?;
    println!("  {} Initialized git repository", "✓".green());

    // Setup remote nếu được cung cấp
    if let Some(remote_url) = remote {
        config.set_remote(remote_url.clone());
        let git = GitSync::open(&vault_dir)?;
        git.add_remote("origin", &remote_url)?;
        println!("  {} Remote: {}", "✓".green(), remote_url);

        // Hỏi user có muốn authenticate với GitHub không
        println!("\n{}", "GitHub Authentication".cyan().bold());
        print!("Authenticate with GitHub now? [Y/n]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let should_auth = input.trim().is_empty() || input.trim().to_lowercase() == "y";

        if should_auth {
            let credentials = authenticate_github()?;
            let creds_path = vault_dir.join(".credentials.json");
            save_credentials_to_file(&credentials, &creds_path)?;
            println!("  {} Saved GitHub credentials", "✓".green());
        }
    }

    // Setup encryption
    println!("\n{}", "Encryption Setup".cyan().bold());
    println!("Enter a passphrase to encrypt your vault.");
    println!(
        "{}",
        "WARNING: If you lose this passphrase, you cannot recover your data!".yellow()
    );

    let passphrase = prompt_passphrase("Passphrase: ")?;
    let confirm = prompt_passphrase("Confirm passphrase: ")?;

    if passphrase != confirm {
        bail!("Passphrases do not match");
    }

    // Derive key và lưu salt
    let (key, salt) = derive_key_new(&passphrase)?;
    let salt_path = vault_dir.join(".salt");
    std::fs::write(&salt_path, salt)?;

    // Test encryption
    let test_data = b"EchoVault encryption test";
    let encryptor = Encryptor::new(&key);
    let encrypted = encryptor.encrypt(test_data)?;
    let decrypted = encryptor.decrypt(&encrypted)?;
    if decrypted != test_data {
        bail!("Encryption test failed");
    }
    println!("  {} Encryption configured", "✓".green());

    // Lưu config
    config.save(&config_path)?;
    println!(
        "  {} Config saved to {}",
        "✓".green(),
        config_path.display()
    );

    println!("\n{}", "Initialization complete!".green().bold());
    println!("Next steps:");
    println!("  1. Run 'echovault scan' to see available chat sessions");
    println!("  2. Run 'echovault extract' to copy sessions to vault");
    println!("  3. Run 'echovault sync' to encrypt and push to GitHub");

    Ok(())
}

/// Authenticate với GitHub qua OAuth Device Flow
fn authenticate_github() -> Result<OAuthCredentials> {
    let oauth = OAuthDeviceFlow::new();

    let credentials = oauth.authenticate(|device_code| {
        println!("\nTo authenticate with GitHub:");
        println!("  1. Open: {}", device_code.verification_uri.cyan().bold());
        println!("  2. Enter code: {}", device_code.user_code.yellow().bold());
        println!(
            "\nWaiting for authorization (expires in {} seconds)...",
            device_code.expires_in
        );
    })?;

    println!("  {} Authenticated successfully!", "✓".green());
    Ok(credentials)
}

/// Prompt cho passphrase (không hiển thị input)
fn prompt_passphrase(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    // Sử dụng rpassword để ẩn input nếu có
    // Fallback: đọc từ stdin
    let passphrase = rpassword::read_password().context("Cannot read passphrase")?;

    if passphrase.is_empty() {
        bail!("Passphrase cannot be empty");
    }

    Ok(passphrase)
}

/// Quét tất cả sources để tìm chat sessions
pub fn scan(source: Option<String>) -> Result<()> {
    println!("{}", "Scanning for chat sessions...".cyan());

    // Hiện tại chỉ hỗ trợ VS Code Copilot
    let _source_filter = source.as_deref();

    let extractor = VSCodeCopilotExtractor::new();
    let locations = extractor.find_storage_locations()?;

    if locations.is_empty() {
        println!("{}", "No VS Code Copilot chat sessions found.".yellow());
        return Ok(());
    }

    println!(
        "\n{} {} workspace(s) with chat sessions:\n",
        "Found".green(),
        locations.len().to_string().green().bold()
    );

    for (idx, location) in locations.iter().enumerate() {
        let workspace_name = extractor.get_workspace_name(location);

        // Đếm số sessions
        let session_count = match extractor.count_sessions(location) {
            Ok(count) => count.to_string(),
            Err(_) => "?".to_string(),
        };

        println!(
            "  {}. {} [{}]",
            (idx + 1).to_string().cyan(),
            workspace_name.white().bold(),
            format!("{} sessions", session_count).dimmed()
        );
        println!("     {}", location.display().to_string().dimmed());
    }

    println!();
    Ok(())
}

/// Trích xuất chat history - CHỈ COPY raw JSON files, KHÔNG format
pub fn extract(source: Option<String>, output: Option<PathBuf>) -> Result<()> {
    // Load config để lấy vault path
    let config = Config::load_default()?;
    let output_dir = output.unwrap_or_else(|| config.vault_path.clone());

    println!(
        "{} to {}",
        "Extracting raw JSON files".cyan(),
        output_dir.display().to_string().yellow()
    );

    // Hiện tại chỉ hỗ trợ VS Code Copilot
    let _source_filter = source.as_deref();

    let extractor = VSCodeCopilotExtractor::new();
    let locations = extractor.find_storage_locations()?;

    if locations.is_empty() {
        println!("{}", "No VS Code Copilot chat sessions found.".yellow());
        return Ok(());
    }

    // Tạo output directory
    std::fs::create_dir_all(&output_dir)?;

    // Mở SQLite index
    let mut index = SessionIndex::open(&output_dir)?;

    let mut total_sessions = 0;
    let mut total_files = 0;
    let mut metadata_list: Vec<crate::extractors::SessionMetadata> = Vec::new();

    for location in &locations {
        let workspace_name = extractor.get_workspace_name(location);

        println!(
            "\n{} {} ({})",
            "Processing:".cyan(),
            workspace_name.white().bold(),
            location.display().to_string().dimmed()
        );

        // List session files
        match extractor.list_session_files(location) {
            Ok(sessions) => {
                for session in &sessions {
                    // Copy raw file vào vault
                    match extractor.copy_to_vault(session, &output_dir) {
                        Ok(vault_path) => {
                            // Cập nhật metadata với vault_path
                            let mut metadata = session.metadata.clone();
                            metadata.vault_path = vault_path.clone();
                            metadata_list.push(metadata);

                            let filename =
                                vault_path.file_name().unwrap_or_default().to_string_lossy();
                            println!("  {} {}", "Copied:".green(), filename);
                            total_files += 1;
                        }
                        Err(e) => {
                            println!("  {} {} - {}", "Error:".red(), session.session_id, e);
                        }
                    }
                }
                total_sessions += sessions.len();
            }
            Err(e) => {
                println!("  {} {}", "Error:".red(), e);
            }
        }
    }

    // Batch upsert vào SQLite index
    let indexed_count = index.upsert_batch(&metadata_list)?;

    println!(
        "\n{} Copied {} sessions ({} files), indexed {} entries",
        "Done!".green().bold(),
        total_sessions.to_string().cyan(),
        total_files.to_string().cyan(),
        indexed_count.to_string().cyan()
    );
    println!(
        "Index database: {}",
        output_dir.join("index.db").display().to_string().dimmed()
    );

    Ok(())
}

/// Encrypt và sync vault lên GitHub
pub fn sync_vault() -> Result<()> {
    println!("{}", "Syncing vault to GitHub...".cyan().bold());

    // Load config
    let config = Config::load_default()?;
    let vault_dir = config.vault_dir();

    if !config.is_initialized() {
        bail!("Vault not initialized. Run 'echovault init' first.");
    }

    // Load encryption key
    let salt_path = vault_dir.join(".salt");
    if !salt_path.exists() {
        bail!("Encryption not configured. Run 'echovault init' first.");
    }

    let salt_bytes = std::fs::read(&salt_path)?;
    if salt_bytes.len() != SALT_LEN {
        bail!("Invalid salt file");
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&salt_bytes);

    let passphrase = prompt_passphrase("Enter passphrase: ")?;
    let key = crate::crypto::key_derivation::derive_key(&passphrase, &salt)?;
    let encryptor = Encryptor::new(&key);

    // Tìm tất cả raw JSON files cần encrypt
    let source_dirs = ["vscode-copilot", "cursor", "cline", "antigravity"];
    let encrypted_dir = config.encrypted_dir();
    std::fs::create_dir_all(&encrypted_dir)?;

    let mut encrypted_count = 0;
    for source in &source_dirs {
        let source_dir = vault_dir.join(source);
        if !source_dir.exists() {
            continue;
        }

        let enc_source_dir = encrypted_dir.join(source);
        std::fs::create_dir_all(&enc_source_dir)?;

        for entry in std::fs::read_dir(&source_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "json") {
                // Encrypt file
                let filename = path.file_name().unwrap().to_string_lossy();
                let enc_path = enc_source_dir.join(format!("{}.enc", filename));

                encryptor.encrypt_file(&path, &enc_path)?;
                println!("  {} {}", "Encrypted:".green(), filename);
                encrypted_count += 1;
            }
        }
    }

    println!(
        "\n{} {} files",
        "Encrypted".green(),
        encrypted_count.to_string().cyan()
    );

    // Load GitHub credentials
    let creds_path = vault_dir.join(".credentials.json");
    let credentials = if creds_path.exists() {
        load_credentials_from_file(&creds_path)?
    } else {
        println!("\n{}", "GitHub authentication required".yellow());
        let creds = authenticate_github()?;
        save_credentials_to_file(&creds, &creds_path)?;
        creds
    };

    // Git operations
    let git = GitSync::open(vault_dir)?;

    // Stage encrypted files
    git.stage_all()?;

    // Check for changes
    if !git.has_changes()? {
        println!("{}", "No changes to sync.".yellow());
        return Ok(());
    }

    // Commit
    let commit_id = git.commit("EchoVault: Auto-sync")?;
    println!(
        "  {} Committed: {}",
        "✓".green(),
        &commit_id.to_string()[..8]
    );

    // Push
    println!("\n{}", "Pushing to GitHub...".cyan());
    git.push("origin", "main", &credentials.access_token)?;
    println!("  {} Pushed successfully!", "✓".green());

    println!("\n{}", "Sync complete!".green().bold());

    Ok(())
}
