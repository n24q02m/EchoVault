//! Git operations cho EchoVault.
//!
//! Sử dụng libgit2 (qua git2 crate) để:
//! - Init repository
//! - Stage và commit changes
//! - Push lên remote (GitHub)
//! - Pull changes từ remote

use anyhow::{bail, Context, Result};
use git2::{
    Commit, Cred, FetchOptions, ObjectType, RemoteCallbacks, Repository,
    RepositoryInitOptions, Signature,
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

/// Git sync engine cho EchoVault vault
pub struct GitSync {
    repo: Repository,
}

#[allow(dead_code)]
impl GitSync {
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
    pub fn has_changes(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)?;
        Ok(!statuses.is_empty())
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
    pub fn clone(url: &str, path: &Path, access_token: &str) -> Result<Self> {
        let mut callbacks = RemoteCallbacks::new();
        let token = access_token.to_string();

        callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
            Cred::userpass_plaintext("x-access-token", &token)
        });

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let repo = git2::build::RepoBuilder::new()
            .fetch_options(fetch_opts)
            .clone(url, path)
            .with_context(|| format!("Cannot clone repository: {}", url))?;

        Ok(Self { repo })
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
}
