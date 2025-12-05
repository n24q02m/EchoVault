//! Git operations cho EchoVault.
//!
//! Sử dụng libgit2 (qua git2 crate) để:
//! - Init repository
//! - Stage và commit changes
//! - Push lên remote (GitHub)
//! - Pull changes từ remote

use anyhow::{bail, Context, Result};
use git2::{
    Commit, Cred, FetchOptions, IndexAddOption, ObjectType, PushOptions, RemoteCallbacks,
    Repository, RepositoryInitOptions, Signature,
};
use std::path::Path;

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

    /// Stage tất cả changes
    pub fn stage_all(&self) -> Result<()> {
        let mut index = self.repo.index()?;
        index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)?;
        index.write()?;
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
    pub fn push(&self, remote_name: &str, branch: &str, access_token: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)?;

        // Setup callbacks cho authentication
        let mut callbacks = RemoteCallbacks::new();
        let token = access_token.to_string();

        callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
            // Sử dụng token như password với username "x-access-token"
            Cred::userpass_plaintext("x-access-token", &token)
        });

        let mut push_opts = PushOptions::new();
        push_opts.remote_callbacks(callbacks);

        // Push
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
        remote
            .push(&[&refspec], Some(&mut push_opts))
            .with_context(|| format!("Cannot push to remote '{}'", remote_name))?;

        Ok(())
    }

    /// Fetch từ remote với access token
    pub fn fetch(&self, remote_name: &str, access_token: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)?;

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
}
