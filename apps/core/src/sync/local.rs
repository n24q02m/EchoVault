use crate::sync::provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Local file system provider.
/// Syncs vault to another local directory (e.g., USB drive, backup folder).
pub struct LocalProvider {
    target_path: PathBuf,
}

impl LocalProvider {
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Recursive copy helper
    fn copy_dir_all(src: &Path, dst: &Path) -> Result<usize> {
        if !dst.exists() {
            fs::create_dir_all(dst)?;
        }

        let mut count = 0;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let dst_path = dst.join(entry.file_name());

            if ty.is_dir() {
                count += Self::copy_dir_all(&entry.path(), &dst_path)?;
            } else {
                // Only copy if modified time differs or size differs
                let should_copy = if dst_path.exists() {
                    let src_meta = entry.metadata()?;
                    let dst_meta = fs::metadata(&dst_path)?;
                    src_meta.len() != dst_meta.len() || src_meta.modified()? > dst_meta.modified()?
                } else {
                    true
                };

                if should_copy {
                    fs::copy(entry.path(), &dst_path)?;
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Check for differences between directories
    fn check_diff(src: &Path, dst: &Path) -> Result<bool> {
        if !dst.exists() {
            return Ok(true);
        }

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let dst_path = dst.join(entry.file_name());

            if !dst_path.exists() {
                return Ok(true);
            }

            if ty.is_dir() {
                if Self::check_diff(&entry.path(), &dst_path)? {
                    return Ok(true);
                }
            } else {
                let src_meta = entry.metadata()?;
                let dst_meta = fs::metadata(&dst_path)?;
                if src_meta.len() != dst_meta.len() || src_meta.modified()? > dst_meta.modified()? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

impl SyncProvider for LocalProvider {
    fn name(&self) -> &'static str {
        "local"
    }

    fn is_authenticated(&self) -> bool {
        // For local provider, "authenticated" means the target path exists and is writable
        self.target_path.exists()
    }

    fn auth_status(&self) -> AuthStatus {
        if self.is_authenticated() {
            AuthStatus::Authenticated
        } else {
            AuthStatus::NotAuthenticated
        }
    }

    fn start_auth(&mut self) -> Result<AuthStatus> {
        // Create directory if not exists
        if !self.target_path.exists() {
            fs::create_dir_all(&self.target_path)?;
        }
        Ok(AuthStatus::Authenticated)
    }

    fn complete_auth(&mut self) -> Result<AuthStatus> {
        self.start_auth()
    }

    fn pull(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PullResult> {
        if !self.target_path.exists() {
            bail!("Target path does not exist");
        }

        info!("[Local] Pulling from {:?} to {:?}", self.target_path, vault_dir);
        let count = Self::copy_dir_all(&self.target_path, vault_dir)?;

        Ok(PullResult {
            has_changes: count > 0,
            new_files: count,
            updated_files: 0,
        })
    }

    fn push(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PushResult> {
        if !self.target_path.exists() {
            fs::create_dir_all(&self.target_path)?;
        }

        info!("[Local] Pushing from {:?} to {:?}", vault_dir, self.target_path);
        let count = Self::copy_dir_all(vault_dir, &self.target_path)?;

        Ok(PushResult {
            success: true,
            files_pushed: count,
            message: Some(format!("Synced to {:?}", self.target_path)),
        })
    }

    fn has_local_changes(&self, vault_dir: &Path) -> Result<bool> {
        // Check if vault has changes not in target
        Self::check_diff(vault_dir, &self.target_path)
    }

    fn has_remote_changes(&self, vault_dir: &Path) -> Result<bool> {
        // Check if target has changes not in vault
        Self::check_diff(&self.target_path, vault_dir)
    }
}
