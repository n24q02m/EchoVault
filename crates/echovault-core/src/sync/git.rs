//! Git operations cho EchoVault.
//!
//! Sử dụng libgit2 (qua git2 crate) để:
//! - Init repository
//! - Stage và commit changes
//! - Push lên remote (GitHub)
//! - Pull changes từ remote

use anyhow::{bail, Context, Result};
use git2::{
    Commit, Cred, FetchOptions, ObjectType, RemoteCallbacks, Repository, RepositoryInitOptions,
    Signature,
};
use std::path::Path;

/// Chuyển đổi SSH URL sang HTTPS URL cho GitHub
/// Ví dụ: git@github.com:user/repo.git -> https://github.com/user/repo.git
fn ssh_to_https_url(url: &str) -> String {
    // SSH format: git@github.com:user/repo.git
    if url.starts_with("git@github.com:") {
        let path = url.trim_start_matches("git@github.com:");
        return format!("https://github.com/{}", path);
    }

    // SSH format: ssh://git@github.com/user/repo.git
    if url.starts_with("ssh://git@github.com/") {
        let path = url.trim_start_matches("ssh://git@github.com/");
        return format!("https://github.com/{}", path);
    }

    // Already HTTPS or other format, return as-is
    url.to_string()
}

/// Normalize GitHub URL - đảm bảo có .git suffix và là HTTPS
fn normalize_github_url(url: &str) -> String {
    let https_url = ssh_to_https_url(url);

    // Thêm .git nếu chưa có
    if https_url.ends_with(".git") {
        https_url
    } else {
        format!("{}.git", https_url.trim_end_matches('/'))
    }
}

/// Git sync engine cho EchoVault vault
pub struct GitSync {
    repo: Repository,
}

#[allow(dead_code)]
impl GitSync {
    /// Lấy workdir path
    pub fn workdir(&self) -> Result<std::path::PathBuf> {
        self.repo
            .workdir()
            .map(|p| p.to_path_buf())
            .context("Repository has no workdir")
    }

    /// Lấy remote URL
    pub fn get_remote_url(&self, name: &str) -> Result<String> {
        let remote = self.repo.find_remote(name)?;
        let url = remote.url().context("Remote has no URL")?;
        Ok(ssh_to_https_url(url))
    }

    /// Mở repository đã tồn tại
    pub fn open(vault_dir: &Path) -> Result<Self> {
        let repo = Repository::open(vault_dir)
            .with_context(|| format!("Cannot open git repository: {}", vault_dir.display()))?;
        Ok(Self { repo })
    }

    /// Khởi tạo repository mới
    pub fn init(vault_dir: &Path) -> Result<Self> {
        let mut opts = RepositoryInitOptions::new();
        opts.initial_head("main");

        let repo = Repository::init_opts(vault_dir, &opts)
            .with_context(|| format!("Cannot init git repository: {}", vault_dir.display()))?;

        // Tạo .gitignore để bỏ qua index.db (local-only)
        let gitignore_path = vault_dir.join(".gitignore");
        std::fs::write(
            &gitignore_path,
            "# Local-only files\nindex.db\nindex.db-journal\nindex.db-wal\n",
        )?;

        // Tạo README.md cho vault
        let readme_path = vault_dir.join("README.md");
        std::fs::write(
            &readme_path,
            r#"# EchoVault

This repository contains encrypted AI chat sessions from various IDEs.

## Structure

- `sessions/` - Encrypted session files organized by source
- `vault.json` - Vault metadata (encryption settings, compression settings)

## Security

- Sessions are encrypted with AES-256-GCM
- Key derived from your passphrase using Argon2id
- Passphrase is stored locally in OS keyring

## Usage

This vault is managed by [EchoVault](https://github.com/n24q02m/EchoVault) app.
DO NOT edit files manually - they may be encrypted and/or compressed.
"#,
        )?;

        Ok(Self { repo })
    }

    /// Mở hoặc khởi tạo repository
    pub fn open_or_init(vault_dir: &Path) -> Result<Self> {
        if vault_dir.join(".git").exists() {
            Self::open(vault_dir)
        } else {
            Self::init(vault_dir)
        }
    }

    /// Thêm remote repository
    pub fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        // Xóa remote cũ nếu tồn tại
        if self.repo.find_remote(name).is_ok() {
            self.repo.remote_delete(name)?;
        }

        self.repo
            .remote(name, url)
            .with_context(|| format!("Cannot add remote '{}': {}", name, url))?;
        Ok(())
    }

    /// Kiểm tra remote có tồn tại không
    pub fn has_remote(&self, name: &str) -> Result<bool> {
        Ok(self.repo.find_remote(name).is_ok())
    }

    /// Cập nhật URL của remote
    pub fn set_remote_url(&self, name: &str, url: &str) -> Result<()> {
        self.repo.remote_set_url(name, url)?;
        Ok(())
    }

    /// Stage tất cả changes (bao gồm cả subdirectories)
    /// Sử dụng git command thay vì libgit2 vì add_all() không recursive đúng cách
    pub fn stage_all(&self) -> Result<()> {
        let workdir = self.repo.workdir().context("No workdir")?;

        // Sử dụng git add -A để add tất cả files (bao gồm subdirectories)
        let output = std::process::Command::new("git")
            .current_dir(workdir)
            .args(["add", "-A"])
            .output()
            .context("Cannot execute git add command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("git add failed: {}", stderr);
        }

        Ok(())
    }

    /// Tạo commit
    pub fn commit(&self, message: &str) -> Result<git2::Oid> {
        // Lấy signature từ config hoặc dùng default
        let sig = self
            .repo
            .signature()
            .or_else(|_| Signature::now("EchoVault", "echovault@local"))
            .context("Cannot create git signature")?;

        // Lấy tree từ index
        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        // Lấy parent commit (nếu có)
        let parent_commit = self.get_head_commit();

        // Tạo commit
        let commit_id = match parent_commit {
            Some(parent) => {
                self.repo
                    .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?
            }
            None => self
                .repo
                .commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?,
        };

        Ok(commit_id)
    }

    /// Lấy HEAD commit (nếu có)
    fn get_head_commit(&self) -> Option<Commit<'_>> {
        self.repo
            .head()
            .ok()
            .and_then(|head| head.peel_to_commit().ok())
    }

    /// Push lên remote với access token
    /// Note: Sử dụng git command thay vì libgit2 để đảm bảo tương thích HTTPS
    /// Trả về Ok(true) nếu push thành công, Ok(false) nếu repo not found
    pub fn push(&self, remote_name: &str, branch: &str, access_token: &str) -> Result<bool> {
        let remote = self.repo.find_remote(remote_name)?;

        // Lấy URL và convert SSH -> HTTPS nếu cần
        let original_url = remote.url().context("Remote has no URL")?;
        let https_url = ssh_to_https_url(original_url);

        // Tạo URL với embedded token để authenticate
        // https://github.com/user/repo.git -> https://x-access-token:TOKEN@github.com/user/repo.git
        let auth_url = https_url.replace(
            "https://github.com/",
            &format!("https://x-access-token:{}@github.com/", access_token),
        );

        // Sử dụng git command để push với progress
        // --progress hiển thị tiến trình, -u set upstream
        let output = std::process::Command::new("git")
            .current_dir(self.repo.workdir().context("No workdir")?)
            .args(["push", "-u", "--progress", &auth_url, branch])
            .output()
            .context("Cannot execute git command")?;

        if output.status.success() {
            // In progress output
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                eprint!("{}", stderr);
            }
            return Ok(true);
        }

        // Kiểm tra lỗi "not found"
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") || stderr.contains("Repository not found") {
            return Ok(false); // Repo chưa tồn tại
        }

        // Lỗi khác
        bail!("Git push failed: {}", stderr);
    }

    /// Fetch từ remote với access token
    /// Note: Sử dụng HTTPS URL trực tiếp để tránh bị git config rewrite
    pub fn fetch(&self, remote_name: &str, access_token: &str) -> Result<()> {
        let remote = self.repo.find_remote(remote_name)?;

        // Lấy URL và convert SSH -> HTTPS nếu cần
        let original_url = remote.url().context("Remote has no URL")?;
        let https_url = ssh_to_https_url(original_url);

        // Tạo anonymous remote với HTTPS URL
        let mut remote = self
            .repo
            .remote_anonymous(&https_url)
            .with_context(|| format!("Cannot create anonymous remote for URL: {}", https_url))?;

        // Setup callbacks cho authentication
        let mut callbacks = RemoteCallbacks::new();
        let token = access_token.to_string();

        callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
            Cred::userpass_plaintext("x-access-token", &token)
        });

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        // Fetch all refs
        remote
            .fetch(&[] as &[&str], Some(&mut fetch_opts), None)
            .with_context(|| format!("Cannot fetch from remote '{}'", remote_name))?;

        Ok(())
    }

    /// Merge remote branch vào local
    pub fn merge_remote(&self, remote_name: &str, branch: &str) -> Result<()> {
        // Fetch trước
        let fetch_head = self
            .repo
            .find_reference(&format!("refs/remotes/{}/{}", remote_name, branch))?;

        let fetch_commit = fetch_head
            .peel(ObjectType::Commit)?
            .into_commit()
            .map_err(|_| anyhow::anyhow!("Cannot peel to commit"))?;

        // Merge analysis
        let (analysis, _) = self
            .repo
            .merge_analysis(&[&self.repo.find_annotated_commit(fetch_commit.id())?])?;

        if analysis.is_up_to_date() {
            // Already up to date
            return Ok(());
        }

        if analysis.is_fast_forward() {
            // Fast-forward merge
            let refname = format!("refs/heads/{}", branch);
            let mut reference = self.repo.find_reference(&refname)?;
            reference.set_target(fetch_commit.id(), "Fast-forward merge")?;

            self.repo.set_head(&refname)?;
            self.repo
                .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

            return Ok(());
        }

        // Normal merge cần user intervention
        bail!("Merge conflict detected. Please resolve manually.");
    }

    /// Kiểm tra có uncommitted changes không
    /// Note: Sử dụng git status command để đảm bảo sync với git operations khác
    pub fn has_changes(&self) -> Result<bool> {
        let workdir = self.repo.workdir().context("No workdir")?;

        // Sử dụng git status --porcelain để check changes
        // Cách này đảm bảo đồng bộ với git add command
        let output = std::process::Command::new("git")
            .current_dir(workdir)
            .args(["status", "--porcelain"])
            .output()
            .context("Cannot execute git status command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("git status failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    /// Lấy số lượng commits ahead/behind so với remote
    pub fn get_ahead_behind(&self, remote_name: &str, branch: &str) -> Result<(usize, usize)> {
        let local_ref = format!("refs/heads/{}", branch);
        let remote_ref = format!("refs/remotes/{}/{}", remote_name, branch);

        let local_oid = self
            .repo
            .find_reference(&local_ref)
            .ok()
            .and_then(|r| r.target());

        let remote_oid = self
            .repo
            .find_reference(&remote_ref)
            .ok()
            .and_then(|r| r.target());

        match (local_oid, remote_oid) {
            (Some(local), Some(remote)) => {
                let (ahead, behind) = self.repo.graph_ahead_behind(local, remote)?;
                Ok((ahead, behind))
            }
            (Some(_), None) => {
                // Remote không có branch này -> tất cả commits đều là ahead
                let count = self.count_commits()?;
                Ok((count, 0))
            }
            (None, _) => {
                // Local không có commits
                Ok((0, 0))
            }
        }
    }

    /// Đếm số commits trong repository
    fn count_commits(&self) -> Result<usize> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        Ok(revwalk.count())
    }

    /// Clone repository từ remote
    /// Sử dụng git command thay vì libgit2 để đảm bảo HTTPS support
    pub fn clone(url: &str, path: &Path, access_token: &str) -> Result<Self> {
        // Normalize URL - thêm .git nếu cần và chuyển sang HTTPS
        let normalized_url = normalize_github_url(url);

        // Tạo URL với embedded token để authenticate
        let auth_url = normalized_url.replace(
            "https://github.com/",
            &format!("https://x-access-token:{}@github.com/", access_token),
        );

        // Sử dụng git clone command
        let output = std::process::Command::new("git")
            .args(["clone", &auth_url, &path.to_string_lossy()])
            .output()
            .context("Cannot execute git clone command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Git clone failed: {}", stderr);
        }

        // Mở repository đã clone
        let repo = Repository::open(path)
            .with_context(|| format!("Cannot open cloned repository: {}", path.display()))?;

        // Cập nhật remote URL để không có token (bảo mật)
        repo.remote_set_url("origin", &normalized_url)?;

        Ok(Self { repo })
    }

    /// Pull từ remote với access token (fetch + merge)
    /// Sử dụng git command để đơn giản và đáng tin cậy hơn
    /// Trả về Ok(true) nếu có changes được pull, Ok(false) nếu already up-to-date
    pub fn pull(&self, remote_name: &str, branch: &str, access_token: &str) -> Result<bool> {
        let remote = self.repo.find_remote(remote_name)?;

        // Lấy URL và convert SSH -> HTTPS nếu cần
        let original_url = remote.url().context("Remote has no URL")?;
        let https_url = ssh_to_https_url(original_url);

        // Tạo URL với embedded token để authenticate
        let auth_url = https_url.replace(
            "https://github.com/",
            &format!("https://x-access-token:{}@github.com/", access_token),
        );

        let workdir = self.repo.workdir().context("No workdir")?;

        // Sử dụng git pull --rebase=false để merge (không rebase)
        let output = std::process::Command::new("git")
            .current_dir(workdir)
            .args(["pull", "--no-rebase", &auth_url, branch])
            .output()
            .context("Cannot execute git pull command")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Kiểm tra xem có pull được gì không
            if stdout.contains("Already up to date") || stdout.contains("Already up-to-date") {
                return Ok(false);
            }
            return Ok(true);
        }

        // Kiểm tra lỗi cụ thể
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Nếu remote không có branch này (repo mới, empty)
        if stderr.contains("couldn't find remote ref")
            || stderr.contains("fatal: Couldn't find remote ref")
        {
            return Ok(false);
        }

        // Nếu có unrelated histories (repo được tạo với README vs local init)
        if stderr.contains("refusing to merge unrelated histories") {
            bail!(
                "Cannot merge: repositories have unrelated histories.\n\
                 This usually happens when the remote repo was initialized with a README.\n\
                 Solution: Delete the remote repo and let EchoVault create a fresh one,\n\
                 or manually resolve with: git pull --allow-unrelated-histories"
            );
        }

        // Nếu có merge conflict
        if stderr.contains("CONFLICT") || stderr.contains("Automatic merge failed") {
            bail!(
                "Merge conflict detected. Please resolve manually:\n\
                 1. cd {}\n\
                 2. Resolve conflicts in affected files\n\
                 3. git add . && git commit\n\
                 4. Run ev sync again",
                workdir.display()
            );
        }

        bail!("Git pull failed: {}", stderr);
    }

    /// Kiểm tra xem remote branch có tồn tại không
    pub fn remote_branch_exists(&self, remote_name: &str, branch: &str) -> bool {
        let ref_name = format!("refs/remotes/{}/{}", remote_name, branch);
        self.repo.find_reference(&ref_name).is_ok()
    }
}

/// Merge local index.json với remote index.json
/// Strategy: union của 2 indexes, newer mtime wins khi có conflict
fn merge_index_files(workdir: &Path, remote_ref: &str) -> Result<()> {
    use std::collections::HashMap;
    use std::fs;

    let index_path = workdir.join("index.json");

    // Load local index
    let local_index: HashMap<String, (u64, u64)> = if index_path.exists() {
        let content = fs::read_to_string(&index_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Get remote index content via git show
    let output = std::process::Command::new("git")
        .current_dir(workdir)
        .args(["show", &format!("{}:index.json", remote_ref)])
        .output()
        .context("Cannot execute git show")?;

    let remote_index: HashMap<String, (u64, u64)> = if output.status.success() {
        let content = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        // Remote không có index.json (first push)
        HashMap::new()
    };

    // Merge: union với newer mtime wins
    let mut merged = local_index.clone();
    let remote_count = remote_index.len();
    for (id, (remote_mtime, remote_size)) in remote_index {
        merged
            .entry(id)
            .and_modify(|local| {
                // Nếu remote mới hơn, dùng remote
                if remote_mtime > local.0 {
                    *local = (remote_mtime, remote_size);
                }
            })
            .or_insert((remote_mtime, remote_size));
    }

    // Save merged index
    let merged_json = serde_json::to_string_pretty(&merged)?;
    fs::write(&index_path, merged_json)?;

    println!(
        "[git] Index merged: {} local + {} remote = {} total",
        local_index.len(),
        remote_count,
        merged.len()
    );

    Ok(())
}

/// Distributed lock để prevent race conditions khi sync từ nhiều machines
/// Lock file chứa machine_id và timestamp
#[allow(dead_code)]
mod sync_lock {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    const LOCK_FILE: &str = ".sync.lock";
    const LOCK_TIMEOUT_SECS: u64 = 60; // Lock hết hạn sau 60 giây

    /// Tạo unique machine ID
    fn get_machine_id() -> String {
        // Sử dụng hostname + pid để tạo unique ID
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let pid = std::process::id();
        format!("{}:{}", hostname, pid)
    }

    /// Tạo lock file trước khi sync
    pub fn acquire_lock(workdir: &Path) -> Result<(), String> {
        let lock_path = workdir.join(LOCK_FILE);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();

        // Check existing lock
        if lock_path.exists() {
            let content = fs::read_to_string(&lock_path).unwrap_or_default();
            let parts: Vec<&str> = content.split(':').collect();
            if parts.len() >= 2 {
                if let Ok(lock_time) = parts[0].parse::<u64>() {
                    // Lock còn hiệu lực?
                    if now - lock_time < LOCK_TIMEOUT_SECS {
                        let owner = parts[1..].join(":");
                        let my_id = get_machine_id();
                        if owner != my_id {
                            println!(
                                "[sync_lock] Lock held by {} ({}s ago)",
                                owner,
                                now - lock_time
                            );
                            return Err(format!("Sync in progress by another device: {}", owner));
                        }
                        // Lock của chính mình, refresh
                    }
                    // Lock hết hạn, có thể override
                }
            }
        }

        // Tạo lock mới
        let machine_id = get_machine_id();
        let lock_content = format!("{}:{}", now, machine_id);
        fs::write(&lock_path, &lock_content).map_err(|e| e.to_string())?;
        println!("[sync_lock] Lock acquired by {}", machine_id);
        Ok(())
    }

    /// Xóa lock file sau khi sync xong
    pub fn release_lock(workdir: &Path) {
        let lock_path = workdir.join(LOCK_FILE);
        if lock_path.exists() {
            let _ = fs::remove_file(&lock_path);
            println!("[sync_lock] Lock released");
        }
    }
}

#[allow(dead_code)]
impl GitSync {
    /// Push lên remote với access token, tự động xử lý conflicts
    /// Logic: push -> nếu rejected -> pull rebase -> nếu conflict -> abort + merge theirs
    /// Trả về Ok(true) nếu push thành công, Ok(false) nếu repo not found
    pub fn push_with_pull(
        &self,
        remote_name: &str,
        branch: &str,
        access_token: &str,
    ) -> Result<bool> {
        let remote = self.repo.find_remote(remote_name)?;
        let original_url = remote.url().context("Remote has no URL")?;
        let https_url = ssh_to_https_url(original_url);

        let auth_url = https_url.replace(
            "https://github.com/",
            &format!("https://x-access-token:{}@github.com/", access_token),
        );

        let workdir = self.repo.workdir().context("No workdir")?;

        // Helper để tạo git command với isolated environment
        let git_cmd = |args: &[&str]| {
            std::process::Command::new("git")
                .current_dir(workdir)
                // Isolated git config - không bị ảnh hưởng bởi user config
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .env("GIT_AUTHOR_NAME", "EchoVault")
                .env("GIT_AUTHOR_EMAIL", "echovault@local")
                .env("GIT_COMMITTER_NAME", "EchoVault")
                .env("GIT_COMMITTER_EMAIL", "echovault@local")
                .args(args)
                .output()
        };

        // 1. Thử push trước
        println!("[git] Attempting push...");
        let output = git_cmd(&["push", "-u", "--progress", &auth_url, branch])
            .context("Cannot execute git push command")?;

        if output.status.success() {
            println!("[git] Push successful!");
            return Ok(true);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Repo không tồn tại
        if stderr.contains("not found") || stderr.contains("Repository not found") {
            return Ok(false);
        }

        // Push bị rejected - remote có commits ahead
        if stderr.contains("rejected") || stderr.contains("Updates were rejected") {
            println!("[git] Push rejected, attempting pull --rebase first...");

            // 2. Thử pull --rebase
            let pull_output = git_cmd(&["pull", "--rebase", &auth_url, branch])
                .context("Cannot execute git pull --rebase command")?;

            if pull_output.status.success() {
                println!("[git] Pull --rebase success, retrying push...");
                let retry_output = git_cmd(&["push", "-u", "--progress", &auth_url, branch])
                    .context("Cannot execute git push command (retry)")?;

                if retry_output.status.success() {
                    return Ok(true);
                }
                let retry_stderr = String::from_utf8_lossy(&retry_output.stderr);
                bail!("Git push failed after rebase: {}", retry_stderr);
            }

            let pull_stderr = String::from_utf8_lossy(&pull_output.stderr);

            // 3. Rebase failed - có thể do conflict ở vault.json
            println!("[git] Pull --rebase failed: {}", pull_stderr);

            // Abort rebase nếu đang trong trạng thái rebase
            println!("[git] Aborting rebase...");
            let _ = git_cmd(&["rebase", "--abort"]);

            // 4. Strategy: Checkout setup files từ remote, MERGE index, giữ local sessions
            // Đây là cách đúng: lấy vault.json từ remote (salt gốc), merge index.json, giữ local data
            println!("[git] Fetching remote...");
            let _ = git_cmd(&["fetch", &auth_url, branch]);

            // Checkout chỉ vault.json từ remote (setup file với salt)
            println!("[git] Checking out vault.json from remote...");
            let _ = git_cmd(&["checkout", "FETCH_HEAD", "--", "vault.json"]);
            let _ = git_cmd(&["checkout", "FETCH_HEAD", "--", ".gitignore"]);

            // MERGE index.json thay vì overwrite
            println!("[git] Merging index.json...");
            if let Err(e) = merge_index_files(workdir, "FETCH_HEAD") {
                println!("[git] Warning: Failed to merge index.json: {}", e);
                // Continue anyway, không fatal
            }

            // Stage tất cả changes (bao gồm local sessions và merged files)
            println!("[git] Staging all changes...");
            let _ = git_cmd(&["add", "-A"]);

            // Commit merge
            println!("[git] Creating merge commit...");
            let commit_output = git_cmd(&[
                "commit",
                "-m",
                "Merge: use remote vault.json, keep local sessions",
                "--allow-empty",
            ]);

            if let Ok(output) = commit_output {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!("[git] Commit warning: {}", stderr);
                }
            }

            // Push lên remote với retry logic
            println!("[git] Pushing merged changes...");

            // Retry với exponential backoff thay vì force push ngay
            let max_retries = 3;
            for attempt in 1..=max_retries {
                let push_output = git_cmd(&["push", "-u", "--force-with-lease", &auth_url, branch])
                    .context("Cannot execute git push after merge")?;

                if push_output.status.success() {
                    println!("[git] Push successful after merge (attempt {})!", attempt);
                    return Ok(true);
                }

                let push_stderr = String::from_utf8_lossy(&push_output.stderr);
                println!(
                    "[git] Push attempt {} failed: {}",
                    attempt,
                    push_stderr.lines().next().unwrap_or("")
                );

                if attempt < max_retries {
                    // Exponential backoff: 1s, 2s, 4s
                    let delay = std::time::Duration::from_secs(1 << (attempt - 1));
                    println!("[git] Retrying in {:?}...", delay);
                    std::thread::sleep(delay);

                    // Pull lại trước khi retry
                    println!("[git] Pulling latest changes before retry...");
                    let _ = git_cmd(&["pull", "--rebase", &auth_url, branch]);

                    // Merge index lại
                    if let Err(e) = merge_index_files(workdir, "FETCH_HEAD") {
                        println!("[git] Warning: Failed to merge index on retry: {}", e);
                    }
                }
            }

            // Sau 3 lần retry thất bại, emit warning nhưng KHÔNG force push
            // Để tránh mất data của người khác
            println!(
                "[git] WARNING: All push attempts failed after {} retries",
                max_retries
            );
            println!("[git] Local changes are saved but not synced to remote.");
            println!("[git] Please try syncing again later or check for conflicts.");

            // Thay vì force push, trả về thành công một phần
            // Local data đã được lưu, chỉ là chưa push được
            bail!(
                "Sync partially completed. Local changes saved but could not push to remote.\n\
                 This usually happens when another device is syncing at the same time.\n\
                 Please try syncing again in a few seconds."
            );
        }

        // Lỗi khác
        bail!("Git push failed: {}", stderr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_repository() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let vault_dir = temp_dir.path().join("vault");

        let git = GitSync::init(&vault_dir)?;

        // Verify .git directory exists
        assert!(vault_dir.join(".git").exists());

        // Verify .gitignore was created
        assert!(vault_dir.join(".gitignore").exists());

        // Verify repo is valid (HEAD may not exist before first commit)
        assert!(!git.repo.is_bare());

        Ok(())
    }

    #[test]
    fn test_stage_and_commit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let vault_dir = temp_dir.path().join("vault");

        let git = GitSync::init(&vault_dir)?;

        // Create a test file
        std::fs::write(vault_dir.join("test.json"), "{}")?;

        // Stage and commit
        git.stage_all()?;
        let commit_id = git.commit("Initial commit")?;

        // Verify commit was created
        let commit = git.repo.find_commit(commit_id)?;
        assert_eq!(commit.message().unwrap(), "Initial commit");

        Ok(())
    }

    #[test]
    fn test_has_changes() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let vault_dir = temp_dir.path().join("vault");

        let git = GitSync::init(&vault_dir)?;

        // Initially no changes (except .gitignore)
        let has_changes = git.has_changes()?;
        assert!(has_changes); // .gitignore is uncommitted

        // Commit everything
        git.stage_all()?;
        git.commit("Initial commit")?;

        // Now no changes
        let has_changes = git.has_changes()?;
        assert!(!has_changes);

        // Create new file
        std::fs::write(vault_dir.join("new.json"), "{}")?;
        let has_changes = git.has_changes()?;
        assert!(has_changes);

        Ok(())
    }

    #[test]
    fn test_add_remote() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let vault_dir = temp_dir.path().join("vault");

        let git = GitSync::init(&vault_dir)?;
        let remote_url = "https://github.com/user/vault.git";
        git.add_remote("origin", remote_url)?;

        // Verify remote exists
        let remote = git.repo.find_remote("origin")?;
        let stored_url = remote.url().unwrap();

        // Remote URL có thể được chuyển đổi bởi git2/libgit2
        // Chấp nhận cả HTTPS và SSH format
        assert!(
            stored_url == remote_url
                || stored_url.contains("github.com")
                    && stored_url.contains("user")
                    && stored_url.contains("vault"),
            "Unexpected remote URL: {}",
            stored_url
        );

        Ok(())
    }

    #[test]
    fn test_ssh_to_https_url() {
        use super::ssh_to_https_url;

        // SSH format: git@github.com:user/repo.git
        assert_eq!(
            ssh_to_https_url("git@github.com:user/repo.git"),
            "https://github.com/user/repo.git"
        );

        // Already HTTPS
        assert_eq!(
            ssh_to_https_url("https://github.com/user/repo.git"),
            "https://github.com/user/repo.git"
        );
    }

    #[test]
    fn test_merge_index_files_no_remote() -> Result<()> {
        // Test khi không có remote index (first push scenario)
        let temp_dir = TempDir::new()?;

        // Tạo local index
        let local_index: std::collections::HashMap<String, (u64, u64)> = [
            ("session1".to_string(), (1000u64, 100u64)),
            ("session2".to_string(), (2000u64, 200u64)),
        ]
        .into();

        let index_path = temp_dir.path().join("index.json");
        let index_json = serde_json::to_string_pretty(&local_index)?;
        std::fs::write(&index_path, &index_json)?;

        // Init git repo
        GitSync::init(temp_dir.path())?;

        // Merge (remote không có index.json)
        let result = merge_index_files(temp_dir.path(), "FETCH_HEAD");
        // Có thể lỗi vì không có FETCH_HEAD, nhưng local index vẫn phải giữ nguyên
        if result.is_err() {
            // Verify local index không bị thay đổi
            let saved_index: std::collections::HashMap<String, (u64, u64)> =
                serde_json::from_str(&std::fs::read_to_string(&index_path)?)?;
            assert_eq!(saved_index.len(), 2);
        }

        Ok(())
    }

    #[test]
    fn test_merge_index_logic() {
        // Unit test cho merge logic (không cần git)
        use std::collections::HashMap;

        let local: HashMap<String, (u64, u64)> = [
            ("session1".to_string(), (1000u64, 100u64)),
            ("session2".to_string(), (2000u64, 200u64)),
        ]
        .into();

        let remote: HashMap<String, (u64, u64)> = [
            ("session2".to_string(), (3000u64, 250u64)), // newer
            ("session3".to_string(), (2500u64, 300u64)),
        ]
        .into();

        // Merge logic: giống như trong merge_index_files
        let mut merged = local.clone();
        for (id, (remote_mtime, remote_size)) in remote {
            merged
                .entry(id)
                .and_modify(|local| {
                    if remote_mtime > local.0 {
                        *local = (remote_mtime, remote_size);
                    }
                })
                .or_insert((remote_mtime, remote_size));
        }

        // Verify merged result
        assert_eq!(merged.len(), 3); // session1, session2, session3
        assert_eq!(merged.get("session1"), Some(&(1000, 100))); // local (chỉ có trong local)
        assert_eq!(merged.get("session2"), Some(&(3000, 250))); // remote (mới hơn)
        assert_eq!(merged.get("session3"), Some(&(2500, 300))); // remote (chỉ có trong remote)
    }
}
