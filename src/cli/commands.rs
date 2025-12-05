//! Command implementations cho EchoVault CLI.
//!
//! Các commands chính:
//! - scan: Quét và liệt kê tất cả chat sessions có sẵn
//! - sync: Extract, encrypt và push lên GitHub (all-in-one)

use crate::config::{default_config_path, default_credentials_path, Config};
use crate::crypto::key_derivation::{derive_key, derive_key_new, SALT_LEN};
use crate::crypto::Encryptor;
use crate::extractors::{vscode_copilot::VSCodeCopilotExtractor, Extractor};
use crate::storage::SessionIndex;
use crate::sync::oauth::{
    check_repo_exists, create_github_repo, load_credentials_from_file, save_credentials_to_file,
    OAuthCredentials, OAuthDeviceFlow,
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

    // Xác định remote URL trước (cần để quyết định clone hay init)
    let remote_url = if let Some(url) = remote {
        Some(url)
    } else {
        config.sync.remote.clone()
    };

    // Setup OAuth credentials trước (cần để check/clone repo)
    // Lưu credentials vào config directory (không phải vault) để không ảnh hưởng việc clone
    let creds_path = default_credentials_path();

    // Tạo config directory nếu chưa có
    if let Some(parent) = creds_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let credentials = if creds_path.exists() {
        load_credentials_from_file(&creds_path)?
    } else {
        println!("\n{}", "GitHub Authentication".cyan().bold());
        let creds = authenticate_github()?;
        save_credentials_to_file(&creds, &creds_path)?;
        println!("  {} Saved credentials", "✓".green());
        creds
    };

    // Setup vault repository với logic mới:
    // 1. Nếu local .git tồn tại -> mở
    // 2. Nếu không, và remote có -> clone
    // 3. Nếu không, và remote không có -> init local + tạo remote
    let git = setup_vault_repo(&vault_dir, &remote_url, &credentials)?;

    // Cập nhật remote URL vào config nếu cần
    if let Ok(url) = git.get_remote_url("origin") {
        config.set_remote(url);
    }

    // Setup/update .gitignore để chỉ push encrypted files
    let gitignore_path = vault_dir.join(".gitignore");
    if should_update_gitignore(&gitignore_path)? {
        create_vault_gitignore(&gitignore_path)?;
        println!("  {} Updated .gitignore", "✓".green());
    }

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

    // === PULL: Đồng bộ từ remote trước để có dữ liệu mới nhất ===
    if git.has_remote("origin")? {
        println!("\n{}", "Pulling from remote...".cyan());
        match git.pull("origin", "main", &credentials.access_token) {
            Ok(true) => println!("  {} Pulled latest changes", "✓".green()),
            Ok(false) => println!("  {} Already up to date", "✓".green()),
            Err(e) => {
                // Nếu lỗi do remote không có branch (repo mới/empty), tiếp tục
                let err_str = e.to_string();
                if err_str.contains("couldn't find remote ref") {
                    println!("  {} Remote is empty, will push first commit", "→".cyan());
                } else {
                    return Err(e);
                }
            }
        }
    }

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

    // Push (không force - an toàn cho dữ liệu)
    println!("\n{}", "Pushing to GitHub...".cyan());
    push_safe(&git, &credentials.access_token)?;
    println!("  {} Pushed successfully!", "✓".green());

    println!("\n{}", "Sync complete!".green().bold());

    Ok(())
}

/// Setup vault repository với logic an toàn:
/// 1. Nếu local .git tồn tại -> mở
/// 2. Nếu không, và remote có dữ liệu -> clone (dọn dẹp vault_dir nếu cần)
/// 3. Nếu không, và remote không có/empty -> init local + tạo remote nếu cần
fn setup_vault_repo(
    vault_dir: &std::path::Path,
    remote_url: &Option<String>,
    credentials: &OAuthCredentials,
) -> Result<GitSync> {
    let git_dir = vault_dir.join(".git");

    // Case 1: Local repo đã tồn tại
    if git_dir.exists() {
        println!("  {} Opened existing vault", "✓".green());
        let git = GitSync::open(vault_dir)?;

        // Cập nhật remote nếu được chỉ định
        if let Some(url) = remote_url {
            if git.has_remote("origin")? {
                git.set_remote_url("origin", url)?;
            } else {
                git.add_remote("origin", url)?;
            }
        }

        return Ok(git);
    }

    // Case 2 & 3: Local chưa có .git, cần kiểm tra remote
    let remote_url = match remote_url {
        Some(url) => url.clone(),
        None => {
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
        }
    };

    // Kiểm tra remote repo có tồn tại không
    println!("  {} Checking remote repository...", "→".cyan());
    let repo_exists = check_repo_exists(&remote_url, &credentials.access_token)?;

    if repo_exists {
        // Case 2: Remote có dữ liệu -> clone
        // Cần đảm bảo vault_dir trống trước khi clone
        if vault_dir.exists() {
            // Kiểm tra xem có files quan trọng không
            let has_important_files = has_local_data(vault_dir)?;
            if has_important_files {
                bail!(
                    "Vault directory '{}' contains local data but no .git folder.\n\
                     This could happen if the vault was partially initialized.\n\
                     Please backup and remove the directory, then run 'ev sync' again:\n\
                     rm -rf {}",
                    vault_dir.display(),
                    vault_dir.display()
                );
            }
            // Không có files quan trọng, xóa directory để clone
            println!(
                "  {} Cleaning up empty vault directory...",
                "→".cyan()
            );
            std::fs::remove_dir_all(vault_dir)?;
        }

        println!("  {} Found existing remote, cloning...", "→".cyan());
        let git = GitSync::clone(&remote_url, vault_dir, &credentials.access_token)?;
        println!("  {} Cloned from remote", "✓".green());
        Ok(git)
    } else {
        // Case 3: Remote không có -> init local + tạo remote
        println!("  {} Remote not found, creating new vault...", "→".cyan());

        // Tạo vault directory
        std::fs::create_dir_all(vault_dir)?;

        // Init local repo
        let git = GitSync::init(vault_dir)?;

        // Tạo remote repo trên GitHub
        let repo_name = remote_url
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or("echovault-backup");

        match create_github_repo(repo_name, &credentials.access_token, true) {
            Ok(clone_url) => {
                println!("  {} Created GitHub repo: {}", "✓".green(), clone_url);
                git.add_remote("origin", &clone_url)?;
            }
            Err(e) if e.to_string().contains("already exists") => {
                // Repo đã tồn tại (race condition hoặc tên trùng)
                println!(
                    "  {} Repository already exists, using provided URL",
                    "→".yellow()
                );
                git.add_remote("origin", &remote_url)?;
            }
            Err(e) => bail!("Cannot create GitHub repository: {}", e),
        }

        println!("  {} Initialized new vault", "✓".green());
        Ok(git)
    }
}

/// Push an toàn - KHÔNG force push
/// Nếu có conflict, yêu cầu user xử lý manual thay vì force push gây mất dữ liệu
fn push_safe(git: &GitSync, access_token: &str) -> Result<()> {
    match git.push("origin", "main", access_token) {
        Ok(true) => Ok(()), // Push thành công
        Ok(false) => {
            // Repo not found - điều này không nên xảy ra vì đã setup ở trên
            bail!(
                "Remote repository not found. This is unexpected.\n\
                 Please run 'ev sync' again to reinitialize."
            );
        }
        Err(e) => {
            let err_msg = e.to_string();

            // Kiểm tra các lỗi có thể retry
            if err_msg.contains("rejected")
                || err_msg.contains("fetch first")
                || err_msg.contains("failed to push")
                || err_msg.contains("non-fast-forward")
            {
                // Remote có commits mà local không có
                // Thử pull và push lại
                println!(
                    "  {} Remote has new commits, pulling...",
                    "→".yellow()
                );

                match git.pull("origin", "main", access_token) {
                    Ok(_) => {
                        // Pull thành công, thử push lại
                        println!("  {} Merged, retrying push...", "→".cyan());
                        match git.push("origin", "main", access_token) {
                            Ok(true) => return Ok(()),
                            Ok(false) => bail!("Repository not found after merge"),
                            Err(e2) => {
                                bail!(
                                    "Push failed after merge: {}\n\
                                     Please resolve manually:\n\
                                     1. cd {}\n\
                                     2. git pull origin main\n\
                                     3. Resolve any conflicts\n\
                                     4. git push origin main\n\
                                     5. Run 'ev sync' again",
                                    e2,
                                    git.workdir()?.display()
                                );
                            }
                        }
                    }
                    Err(pull_err) => {
                        bail!(
                            "Cannot sync with remote: {}\n\
                             Please resolve manually:\n\
                             1. cd {}\n\
                             2. git pull origin main --no-rebase\n\
                             3. Resolve any conflicts\n\
                             4. git push origin main\n\
                             5. Run 'ev sync' again",
                            pull_err,
                            git.workdir()?.display()
                        );
                    }
                }
            }

            // Lỗi khác
            Err(e)
        }
    }
}

/// Kiểm tra xem vault directory có chứa local data quan trọng không
/// Trả về true nếu có files cần được giữ lại (encrypted, sessions, salt, etc.)
fn has_local_data(vault_dir: &std::path::Path) -> Result<bool> {
    if !vault_dir.exists() {
        return Ok(false);
    }

    // Các files/folders quan trọng cần kiểm tra
    let important_items = [
        "encrypted",      // Encrypted sessions
        "vscode-copilot", // Raw sessions
        "cursor",
        "cline",
        "antigravity",
        ".salt",    // Encryption salt
        "index.db", // SQLite index
    ];

    for item in &important_items {
        let path = vault_dir.join(item);
        if path.exists() {
            return Ok(true);
        }
    }

    // Kiểm tra xem directory có files không (ngoại trừ hidden files tạm)
    let entries: Vec<_> = std::fs::read_dir(vault_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            // Bỏ qua các files tạm/hidden không quan trọng
            !name_str.starts_with('.') || name_str == ".salt"
        })
        .collect();

    Ok(!entries.is_empty())
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
