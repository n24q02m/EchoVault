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
use crate::utils::open_browser;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Authenticate với GitHub qua OAuth Device Flow
fn authenticate_github() -> Result<OAuthCredentials> {
    println!("\n{}", "GitHub OAuth Device Flow".cyan().bold());

    let oauth = OAuthDeviceFlow::new();

    let credentials = oauth.authenticate(|device_code| {
        println!("\nĐể xác thực với GitHub:");
        println!("  1. Nhập mã: {}", device_code.user_code.yellow().bold());
        println!(
            "  2. Truy cập: {}",
            device_code.verification_uri.cyan().underline()
        );

        // Tự động mở browser
        println!("\nĐang thử mở trình duyệt...");
        if open_browser(&device_code.verification_uri) {
            println!("  {} Đã mở trình duyệt!", "✓".green());
        } else {
            println!(
                "  {} Không thể mở tự động. Vui lòng mở link trên thủ công.",
                "!".yellow()
            );
        }
        println!(); // Blank line trước spinner
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

    // === ENCRYPT: Compress + Encrypt (parallel + incremental) ===
    println!(
        "\n{}",
        "Encrypting files (parallel + incremental)...".cyan()
    );
    let source_dirs = ["vscode-copilot", "cursor", "cline", "antigravity"];
    let encrypted_dir = config.encrypted_dir();
    std::fs::create_dir_all(&encrypted_dir)?;

    // Thu thập tất cả files cần encrypt
    let mut files_to_encrypt: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    let mut skipped_count = 0;

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
                let file_stem = path.file_stem().unwrap().to_string_lossy();
                let enc_path = enc_source_dir.join(format!("{}.json.gz.enc", file_stem));

                // Incremental: chỉ encrypt nếu file mới hoặc đã thay đổi
                if enc_path.exists() {
                    let src_modified = path.metadata()?.modified()?;
                    let enc_modified = enc_path.metadata()?.modified()?;
                    if src_modified <= enc_modified {
                        skipped_count += 1;
                        continue; // Skip - đã encrypt và không thay đổi
                    }
                }

                files_to_encrypt.push((path, enc_source_dir.clone()));
            }
        }
    }

    let total_files = files_to_encrypt.len();
    if total_files == 0 {
        if skipped_count > 0 {
            println!(
                "  {} All {} files already encrypted (skipped)",
                "✓".green(),
                skipped_count.to_string().cyan()
            );
        }
    } else {
        // Progress bar
        let pb = ProgressBar::new(total_files as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );

        // Parallel encryption với rayon
        let encrypted_count = AtomicUsize::new(0);
        let error_count = AtomicUsize::new(0);

        files_to_encrypt
            .par_iter()
            .progress_with(pb.clone())
            .for_each(|(path, enc_dir)| {
                match crate::storage::compress_encrypt_chunk(path, enc_dir, &encryptor) {
                    Ok(()) => {
                        encrypted_count.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_e) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });

        pb.finish_and_clear();

        let encrypted = encrypted_count.load(Ordering::Relaxed);
        let errors = error_count.load(Ordering::Relaxed);

        println!(
            "  {} Encrypted {} files ({} skipped, {} errors)",
            "✓".green(),
            encrypted.to_string().cyan(),
            skipped_count.to_string().yellow(),
            errors.to_string().red()
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

    // Push (với retry logic cho trường hợp remote có commits)
    println!("\n{}", "Pushing to GitHub...".cyan());
    let push_result = push_with_retry(&git, &credentials.access_token, &config)?;

    if push_result {
        println!("  {} Pushed successfully!", "✓".green());
    }

    println!("\n{}", "Sync complete!".green().bold());

    Ok(())
}

/// Push với retry logic - handle các trường hợp:
/// 1. Repo chưa tồn tại → tạo mới
/// 2. Remote có commits khác → pull-merge trước, rồi push
/// 3. Chỉ force push nếu merge fail
fn push_with_retry(git: &GitSync, access_token: &str, config: &Config) -> Result<bool> {
    let vault_dir = git.workdir()?;
    let remote_url = git.get_remote_url("origin")?;
    let auth_url = remote_url.replace(
        "https://github.com/",
        &format!("https://x-access-token:{}@github.com/", access_token),
    );

    // Thử push lần đầu
    match git.push("origin", "main", access_token) {
        Ok(true) => return Ok(true), // Push thành công
        Ok(false) => {
            // Repo chưa tồn tại, tự động tạo
            let repo_name = config
                .sync
                .remote
                .as_deref()
                .unwrap_or("")
                .trim_end_matches(".git")
                .rsplit('/')
                .next()
                .unwrap_or("echovault-backup");

            println!(
                "  {} Repository chưa tồn tại, đang tạo '{}'...",
                "→".cyan(),
                repo_name
            );

            match create_github_repo(repo_name, access_token, true) {
                Ok(clone_url) => {
                    println!("  {} Created: {}", "✓".green(), clone_url);
                    println!("  {} Pushing...", "→".cyan());
                    git.push("origin", "main", access_token)?;
                    return Ok(true);
                }
                Err(e) if e.to_string().contains("already exists") => {
                    // Race condition - repo đã được tạo, tiếp tục force push
                    println!("  {} Repository đã tồn tại", "→".cyan());
                }
                Err(e) => bail!("Cannot create repository: {}", e),
            }
        }
        Err(e) => {
            let err_msg = e.to_string();
            if !err_msg.contains("rejected")
                && !err_msg.contains("fetch first")
                && !err_msg.contains("failed to push")
            {
                // Lỗi khác, không phải conflict
                bail!("{}", e);
            }
            // Conflict - tiếp tục force push
            println!("  {} Remote có thay đổi, đang pull-merge...", "→".yellow());
        }
    }

    // Pull và merge từ remote trước (encrypted files có unique names nên không conflict)
    println!("  {} Pulling from remote...", "→".cyan());
    let pull_output = std::process::Command::new("git")
        .current_dir(&vault_dir)
        .args(["pull", "--no-rebase", &auth_url, "main"])
        .output()
        .context("Cannot execute git pull")?;

    if pull_output.status.success() {
        // Pull thành công, thử push lại
        println!("  {} Merged successfully, pushing...", "✓".green());
        let push_output = std::process::Command::new("git")
            .current_dir(&vault_dir)
            .args(["push", &auth_url, "main"])
            .output()
            .context("Cannot execute git push")?;

        if push_output.status.success() {
            return Ok(true);
        }
    }

    // Pull/merge failed - có conflict thực sự
    // Trong trường hợp này, vẫn cần force push
    println!(
        "  {} Merge failed, force pushing (có thể mất data từ máy khác)...",
        "!".red()
    );

    let output = std::process::Command::new("git")
        .current_dir(&vault_dir)
        .args(["push", "--force", &auth_url, "main"])
        .output()
        .context("Cannot execute git push --force")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Force push failed: {}", stderr);
    }

    Ok(true)
}

/// Extract sessions từ các IDE sources vào vault (parallel)
fn extract_sessions(vault_dir: &std::path::Path) -> Result<usize> {
    let extractor = VSCodeCopilotExtractor::new();
    let locations = extractor.find_storage_locations()?;

    if locations.is_empty() {
        return Ok(0);
    }

    // Thu thập tất cả session files từ tất cả locations (parallel)
    let all_sessions: Vec<_> = locations
        .par_iter()
        .flat_map(|location| {
            extractor
                .list_session_files(location)
                .unwrap_or_default()
                .into_par_iter()
        })
        .collect();

    if all_sessions.is_empty() {
        return Ok(0);
    }

    // Progress bar
    let total = all_sessions.len();
    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=>-"),
    );

    // Copy files vào vault (parallel)
    let copied_count = AtomicUsize::new(0);
    let skipped_count = AtomicUsize::new(0);
    let metadata_results: Vec<_> = all_sessions
        .par_iter()
        .progress_with(pb.clone())
        .filter_map(|session| {
            match extractor.copy_to_vault(session, vault_dir) {
                Ok(Some(vault_path)) => {
                    // File đã được copy (mới hoặc thay đổi)
                    copied_count.fetch_add(1, Ordering::Relaxed);
                    let mut metadata = session.metadata.clone();
                    metadata.vault_path = vault_path;
                    Some(metadata)
                }
                Ok(None) => {
                    // File không thay đổi, skip
                    skipped_count.fetch_add(1, Ordering::Relaxed);
                    None
                }
                Err(_) => None,
            }
        })
        .collect();

    pb.finish_and_clear();

    // Batch upsert vào SQLite index (chỉ files đã copy)
    if !metadata_results.is_empty() {
        let mut index = SessionIndex::open(vault_dir)?;
        index.upsert_batch(&metadata_results)?;
    }

    let copied = copied_count.load(Ordering::Relaxed);
    let skipped = skipped_count.load(Ordering::Relaxed);

    if skipped > 0 {
        println!(
            "  {} {} copied, {} unchanged",
            "→".cyan(),
            copied.to_string().cyan(),
            skipped.to_string().dimmed()
        );
    }

    Ok(copied)
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
