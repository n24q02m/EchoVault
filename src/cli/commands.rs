//! Command implementations cho EchoVault CLI.
//!
//! Các commands chính:
//! - scan: Quét và liệt kê tất cả chat sessions có sẵn
//! - sync: Extract, encrypt và push lên GitHub (all-in-one)

use crate::config::{default_config_path, Config};
use crate::crypto::key_derivation::{derive_key, derive_key_new, SALT_LEN};
use crate::crypto::Encryptor;
use crate::extractors::{vscode_copilot::VSCodeCopilotExtractor, Extractor};
use crate::storage::SessionIndex;
use crate::sync::oauth::{
    create_github_repo, load_credentials_from_file, save_credentials_to_file, OAuthCredentials,
    OAuthDeviceFlow,
};
use crate::sync::GitSync;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::io::{self, Write};

/// Authenticate với GitHub qua OAuth Device Flow
fn authenticate_github() -> Result<OAuthCredentials> {
    println!("\n{}", "GitHub OAuth Device Flow".cyan().bold());

    let oauth = OAuthDeviceFlow::new();

    let credentials = oauth.authenticate(|device_code| {
        println!("\nĐể xác thực với GitHub:");
        println!(
            "  1. Mở trình duyệt: {}",
            device_code.verification_uri.cyan().bold()
        );
        println!("  2. Nhập mã: {}", device_code.user_code.yellow().bold());
        println!(
            "\nĐang chờ xác thực (hết hạn sau {} giây)...",
            device_code.expires_in
        );
    })?;

    println!("  {} Xác thực thành công!", "✓".green());
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

/// Extract, encrypt và sync vault lên GitHub
/// Tự động setup (remote, OAuth, encryption) nếu lần đầu chạy
pub fn sync_vault(remote: Option<String>) -> Result<()> {
    println!("{}", "Syncing vault to GitHub...".cyan().bold());

    // Load hoặc tạo config mới
    let config_path = default_config_path();
    let mut config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        Config::new()
    };
    let vault_dir = config.vault_dir().to_path_buf();

    // Tạo vault directory nếu chưa có
    std::fs::create_dir_all(&vault_dir)?;

    // Init git repository nếu chưa có
    let git = GitSync::open_or_init(&vault_dir)?;

    // Setup/update .gitignore để chỉ push encrypted files
    let gitignore_path = vault_dir.join(".gitignore");
    if should_update_gitignore(&gitignore_path)? {
        create_vault_gitignore(&gitignore_path)?;
        println!("  {} Updated .gitignore", "✓".green());
    }

    // Setup remote nếu cần (kiểm tra cả config và git remote)
    let has_git_remote = git.has_remote("origin")?;
    let need_remote_setup = config.sync.remote.is_none() || remote.is_some() || !has_git_remote;
    if need_remote_setup {
        let remote_url = if let Some(url) = remote {
            url
        } else if let Some(url) = config.sync.remote.clone() {
            // Có trong config nhưng chưa add vào git
            url
        } else {
            // Hỏi user nhập remote URL
            println!("\n{}", "GitHub Repository Setup".cyan().bold());
            print!("Remote URL (e.g., https://github.com/user/vault.git): ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let url = input.trim().to_string();
            if url.is_empty() {
                bail!("Remote URL is required");
            }
            url
        };

        config.set_remote(remote_url.clone());

        // Add/update remote
        if has_git_remote {
            git.set_remote_url("origin", &remote_url)?;
        } else {
            git.add_remote("origin", &remote_url)?;
        }
        println!("  {} Remote: {}", "✓".green(), remote_url);
    }

    // Setup OAuth credentials nếu chưa có
    let creds_path = vault_dir.join(".credentials.json");
    let credentials = if creds_path.exists() {
        load_credentials_from_file(&creds_path)?
    } else {
        println!("\n{}", "GitHub Authentication".cyan().bold());
        let creds = authenticate_github()?;
        save_credentials_to_file(&creds, &creds_path)?;
        println!("  {} Saved credentials", "✓".green());
        creds
    };

    // Setup encryption nếu chưa có
    let salt_path = vault_dir.join(".salt");
    let (encryptor, is_new_encryption) = if salt_path.exists() {
        // Load existing salt
        let salt_bytes = std::fs::read(&salt_path)?;
        if salt_bytes.len() != SALT_LEN {
            bail!("Invalid salt file");
        }
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&salt_bytes);

        let passphrase = prompt_passphrase("Enter passphrase: ")?;
        let key = derive_key(&passphrase, &salt)?;
        (Encryptor::new(&key), false)
    } else {
        // Setup new encryption
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

        let (key, salt) = derive_key_new(&passphrase)?;
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
        (encryptor, true)
    };

    // Tạo README trong vault nếu chưa có
    let readme_path = vault_dir.join("README.md");
    if !readme_path.exists() {
        create_vault_readme(&readme_path)?;
        println!("  {} Created README.md", "✓".green());
    }

    // Lưu config
    config.save(&config_path)?;

    // === EXTRACT: Copy raw JSON files từ các IDE ===
    println!("\n{}", "Extracting chat sessions...".cyan());
    let extracted_count = extract_sessions(&vault_dir)?;
    if extracted_count > 0 {
        println!(
            "  {} Extracted {} sessions",
            "✓".green(),
            extracted_count.to_string().cyan()
        );
    }

    // === ENCRYPT: Mã hóa tất cả raw JSON files ===
    println!("\n{}", "Encrypting files...".cyan());
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
                let filename = path.file_name().unwrap().to_string_lossy();
                let enc_path = enc_source_dir.join(format!("{}.enc", filename));

                encryptor.encrypt_file(&path, &enc_path)?;
                println!("  {} {}", "Encrypted:".green(), filename);
                encrypted_count += 1;
            }
        }
    }

    if encrypted_count > 0 {
        println!(
            "\n{} {} files",
            "Encrypted".green(),
            encrypted_count.to_string().cyan()
        );
    }

    // Git operations
    git.stage_all()?;

    // Check for changes
    if !git.has_changes()? && !is_new_encryption {
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
    let push_success = git.push("origin", "main", &credentials.access_token)?;

    if !push_success {
        // Repo chưa tồn tại, tự động tạo
        let remote_url = config.sync.remote.as_deref().unwrap_or("");
        let repo_name = remote_url
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or("echovault-backup");

        println!(
            "  {} Repository chưa tồn tại, đang tạo '{}'...",
            "→".cyan(),
            repo_name
        );

        match create_github_repo(repo_name, &credentials.access_token, true) {
            Ok(clone_url) => {
                println!("  {} Created: {}", "✓".green(), clone_url);

                // Thử push lại
                println!("  {} Pushing...", "→".cyan());
                let retry_success = git.push("origin", "main", &credentials.access_token)?;
                if !retry_success {
                    bail!("Push failed after creating repository");
                }
            }
            Err(create_err) => {
                // Nếu repo đã tồn tại (race condition), thử push lại
                if create_err.to_string().contains("already exists") {
                    println!("  {} Repository đã tồn tại, pushing...", "→".cyan());
                    let retry_success = git.push("origin", "main", &credentials.access_token)?;
                    if !retry_success {
                        bail!("Push failed");
                    }
                } else {
                    bail!("Cannot create repository: {}", create_err);
                }
            }
        }
    }

    println!("  {} Pushed successfully!", "✓".green());
    println!("\n{}", "Sync complete!".green().bold());

    Ok(())
}

/// Extract sessions từ các IDE sources vào vault
fn extract_sessions(vault_dir: &std::path::Path) -> Result<usize> {
    let extractor = VSCodeCopilotExtractor::new();
    let locations = extractor.find_storage_locations()?;

    if locations.is_empty() {
        return Ok(0);
    }

    // Mở SQLite index
    let mut index = SessionIndex::open(vault_dir)?;
    let mut total_files = 0;
    let mut metadata_list: Vec<crate::extractors::SessionMetadata> = Vec::new();

    for location in &locations {
        // List session files
        if let Ok(sessions) = extractor.list_session_files(location) {
            for session in &sessions {
                // Copy raw file vào vault
                if let Ok(vault_path) = extractor.copy_to_vault(session, vault_dir) {
                    let mut metadata = session.metadata.clone();
                    metadata.vault_path = vault_path;
                    metadata_list.push(metadata);
                    total_files += 1;
                }
            }
        }
    }

    // Batch upsert vào SQLite index
    index.upsert_batch(&metadata_list)?;

    Ok(total_files)
}

/// Tạo .gitignore cho vault để chỉ push encrypted files
fn create_vault_gitignore(path: &std::path::Path) -> Result<()> {
    // Sử dụng /folder/ để chỉ ignore folders ở root level
    // Không dùng folder/ vì nó sẽ match ở mọi depth (kể cả trong encrypted/)
    let content = r#"# EchoVault .gitignore
# Chỉ push encrypted files, không push raw files

# Raw session files (local only, not synced)
# Prefix / để chỉ match folders ở root level
/vscode-copilot/
/cursor/
/cline/
/antigravity/

# Index database (local only)
index.db

# Credentials (NEVER commit)
.credentials.json
"#;
    std::fs::write(path, content)?;
    Ok(())
}

/// Kiểm tra xem cần update .gitignore hay không
fn should_update_gitignore(path: &std::path::Path) -> Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    // Kiểm tra xem gitignore đã có rule ignore raw files chưa
    let content = std::fs::read_to_string(path)?;
    // Nếu chưa có rule ignore vscode-copilot/ thì cần update
    Ok(!content.contains("vscode-copilot/"))
}

/// Tạo README hướng dẫn trong vault repository
fn create_vault_readme(path: &std::path::Path) -> Result<()> {
    let content = r#"# EchoVault Backup

Kho lưu trữ được mã hóa của EchoVault - "Black Box" cho lịch sử chat AI.

## Nội dung

- `encrypted/` - Dữ liệu chat đã mã hóa AES-256-GCM
- `.salt` - Salt file cần thiết cho quá trình giải mã

## Khôi phục dữ liệu

1. Clone repository này về máy mới
2. Cài đặt EchoVault:
   ```bash
   cargo install echovault
   ```
3. Chạy sync để pull và decrypt:
   ```bash
   echovault sync --remote <URL_REPO_NÀY>
   ```
4. Nhập passphrase đã sử dụng khi tạo vault

## Lưu ý bảo mật

- Files `.enc` được mã hóa bằng AES-256-GCM
- **Passphrase là chìa khóa duy nhất** - mất passphrase = mất dữ liệu
- File `.salt` cần thiết cho việc giải mã, không xóa!
- Repository có thể public vì dữ liệu đã được mã hóa

## Thông tin thêm

Xem https://github.com/n24q02m/EchoVault để biết thêm chi tiết.
"#;
    std::fs::write(path, content)?;
    Ok(())
}
