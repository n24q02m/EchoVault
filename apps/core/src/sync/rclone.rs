//! Rclone Provider - Sync vault via Rclone.
//!
//! This provider uses Rclone as backend for syncing with Google Drive.
//!
//! Advantages:
//! - No user setup of OAuth Client ID/Secret required
//! - Rclone comes with built-in OAuth credentials for Google Drive
//! - Bundled into app, no separate installation needed

use super::provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tracing::info;

/// Default remote name for Google Drive
const DEFAULT_REMOTE_NAME: &str = "echovault-gdrive";

/// Remote path on cloud storage
const DEFAULT_REMOTE_PATH: &str = "EchoVault";

/// Rclone sync provider
pub struct RcloneProvider {
    /// Path to rclone binary
    rclone_path: PathBuf,
    /// Configured remote name (e.g., "echovault-gdrive")
    remote_name: String,
    /// Path on remote (e.g., "EchoVault")
    remote_path: String,
    /// Whether remote is configured
    is_configured: bool,
}

impl RcloneProvider {
    /// Create new provider with bundled rclone binary.
    pub fn new() -> Self {
        let rclone_path = Self::find_rclone_binary();
        let mut provider = Self {
            rclone_path,
            remote_name: DEFAULT_REMOTE_NAME.to_string(),
            remote_path: DEFAULT_REMOTE_PATH.to_string(),
            is_configured: false,
        };

        // Check if remote is already configured
        provider.is_configured = provider.check_remote_exists().unwrap_or(false);
        provider
    }

    /// Create provider with custom remote name.
    pub fn with_remote(remote_name: &str, remote_path: &str) -> Self {
        let rclone_path = Self::find_rclone_binary();
        let mut provider = Self {
            rclone_path,
            remote_name: remote_name.to_string(),
            remote_path: remote_path.to_string(),
            is_configured: false,
        };

        provider.is_configured = provider.check_remote_exists().unwrap_or(false);
        provider
    }

    /// Find rclone binary - prefer bundled, fallback to system.
    fn find_rclone_binary() -> PathBuf {
        // Try to find bundled rclone first (Tauri sidecar)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let bundled = if cfg!(windows) {
                    exe_dir.join("rclone.exe")
                } else {
                    exe_dir.join("rclone")
                };
                if bundled.exists() {
                    return bundled;
                }

                // Try in binaries directory (development mode)
                let dev_bundled = if cfg!(windows) {
                    exe_dir.join("binaries").join("rclone.exe")
                } else {
                    exe_dir.join("binaries").join("rclone")
                };
                if dev_bundled.exists() {
                    return dev_bundled;
                }
            }
        }

        // Fallback: use system rclone
        if cfg!(windows) {
            PathBuf::from("rclone.exe")
        } else {
            PathBuf::from("rclone")
        }
    }

    /// Run rclone command and return output.
    fn run_rclone(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.rclone_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Cannot execute rclone")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Rclone failed: {}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run rclone command with direct output (for interactive commands).
    fn run_rclone_interactive(&self, args: &[&str]) -> Result<()> {
        let status = Command::new(&self.rclone_path)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Cannot execute rclone")?;

        if !status.success() {
            bail!("Rclone command failed with exit code: {:?}", status.code());
        }

        Ok(())
    }

    /// List configured remotes.
    pub fn list_remotes(&self) -> Result<Vec<String>> {
        let output = self.run_rclone(&["listremotes"])?;
        let remotes: Vec<String> = output
            .lines()
            .map(|line| line.trim_end_matches(':').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(remotes)
    }

    /// Check if remote exists.
    pub fn check_remote_exists(&self) -> Result<bool> {
        let remotes = self.list_remotes()?;
        Ok(remotes.contains(&self.remote_name))
    }

    /// Configure new remote (interactive).
    pub fn configure_remote(&self, remote_type: &str) -> Result<()> {
        info!("[Rclone] Configuring remote '{}'...", self.remote_name);
        info!("[Rclone] Browser will open for you to login.");

        // rclone config create <name> <type> --config
        // For Google Drive: rclone config create echovault-gdrive drive
        self.run_rclone_interactive(&["config", "create", &self.remote_name, remote_type])?;

        Ok(())
    }

    /// Get full remote URL (remote:path).
    fn get_remote_url(&self) -> String {
        format!("{}:{}", self.remote_name, self.remote_path)
    }

    /// Check if rclone is available.
    pub fn check_rclone_available(&self) -> Result<bool> {
        match self.run_rclone(&["version"]) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get rclone version.
    pub fn get_version(&self) -> Result<String> {
        let output = self.run_rclone(&["version"])?;
        // Get first line containing version
        let version = output.lines().next().unwrap_or("Unknown").to_string();
        Ok(version)
    }

    /// Get current remote name.
    pub fn remote_name(&self) -> &str {
        &self.remote_name
    }

    /// Get current remote path.
    pub fn remote_path(&self) -> &str {
        &self.remote_path
    }
}

impl Default for RcloneProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncProvider for RcloneProvider {
    fn name(&self) -> &'static str {
        "rclone"
    }

    fn is_authenticated(&self) -> bool {
        self.is_configured
    }

    fn auth_status(&self) -> AuthStatus {
        if self.is_configured {
            AuthStatus::Authenticated
        } else {
            AuthStatus::NotAuthenticated
        }
    }

    fn start_auth(&mut self) -> Result<AuthStatus> {
        // Check if rclone is available
        if !self.check_rclone_available()? {
            bail!("Rclone not found. Please ensure rclone is installed or bundled.");
        }

        // Run rclone config create with Google Drive
        // rclone will automatically open browser for OAuth
        info!("[Rclone] Starting Google Drive configuration...");
        info!("[Rclone] Browser will automatically open for Google login.");

        // Create remote with Google Drive
        let result = self.configure_remote("drive");

        if result.is_ok() {
            self.is_configured = self.check_remote_exists().unwrap_or(false);
            if self.is_configured {
                return Ok(AuthStatus::Authenticated);
            }
        }

        // Return pending so frontend can retry checking
        Ok(AuthStatus::Pending {
            user_code: "Configuring...".to_string(),
            verify_url: "Please complete in browser".to_string(),
        })
    }

    fn complete_auth(&mut self) -> Result<AuthStatus> {
        // Recheck if remote has been configured
        self.is_configured = self.check_remote_exists()?;

        if self.is_configured {
            Ok(AuthStatus::Authenticated)
        } else {
            Ok(AuthStatus::NotAuthenticated)
        }
    }

    fn pull(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PullResult> {
        if !self.is_configured {
            bail!("Remote not configured. Please run start_auth first.");
        }

        let remote_url = self.get_remote_url();
        let local_path = vault_dir.to_string_lossy();

        info!("[Rclone] Pulling from {} to {}...", remote_url, local_path);

        // rclone sync remote:path local_path
        // Use --verbose to be able to parse output later
        let output = self.run_rclone(&[
            "sync",
            &remote_url,
            &local_path,
            "--verbose",
            "--stats-one-line",
        ])?;

        // Parse output to count files (simplified)
        let new_files = output.matches("Transferred:").count();

        Ok(PullResult {
            has_changes: new_files > 0,
            new_files,
            updated_files: 0,
        })
    }

    fn push(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PushResult> {
        if !self.is_configured {
            bail!("Remote not configured. Please run start_auth first.");
        }

        let remote_url = self.get_remote_url();
        let local_path = vault_dir.to_string_lossy();

        info!("[Rclone] Pushing from {} to {}...", local_path, remote_url);

        // rclone sync local_path remote:path
        let output = self.run_rclone(&[
            "sync",
            &local_path,
            &remote_url,
            "--verbose",
            "--stats-one-line",
        ])?;

        // Parse output to count files (simplified)
        let files_pushed = output.matches("Transferred:").count();

        Ok(PushResult {
            success: true,
            files_pushed,
            message: Some(format!("Synced to {}", remote_url)),
        })
    }

    fn has_local_changes(&self, vault_dir: &Path) -> Result<bool> {
        if !self.is_configured {
            return Ok(false);
        }

        let remote_url = self.get_remote_url();
        let local_path = vault_dir.to_string_lossy();

        // rclone check local remote --one-way --differ
        let output = self.run_rclone(&[
            "check",
            &local_path,
            &remote_url,
            "--one-way",
            "--differ",
            "-q",
        ]);

        // If there's output = there are differences
        match output {
            Ok(out) => Ok(!out.trim().is_empty()),
            Err(_) => Ok(true), // Assume there are changes if check fails
        }
    }

    fn has_remote_changes(&self, vault_dir: &Path) -> Result<bool> {
        if !self.is_configured {
            return Ok(false);
        }

        let remote_url = self.get_remote_url();
        let local_path = vault_dir.to_string_lossy();

        // rclone check remote local --one-way --differ
        let output = self.run_rclone(&[
            "check",
            &remote_url,
            &local_path,
            "--one-way",
            "--differ",
            "-q",
        ]);

        match output {
            Ok(out) => Ok(!out.trim().is_empty()),
            Err(_) => Ok(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rclone_provider_new() {
        let provider = RcloneProvider::new();
        assert_eq!(provider.name(), "rclone");
        assert_eq!(provider.remote_name(), DEFAULT_REMOTE_NAME);
        assert_eq!(provider.remote_path(), DEFAULT_REMOTE_PATH);
    }

    #[test]
    fn test_rclone_provider_with_remote() {
        let provider = RcloneProvider::with_remote("my-drive", "MyBackup");
        assert_eq!(provider.remote_name(), "my-drive");
        assert_eq!(provider.remote_path(), "MyBackup");
    }

    #[test]
    fn test_get_remote_url() {
        let provider = RcloneProvider::new();
        assert_eq!(provider.get_remote_url(), "echovault-gdrive:EchoVault");
    }
}
