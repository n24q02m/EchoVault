//! Rclone Provider - Sync vault thông qua Rclone.
//!
//! Provider này sử dụng Rclone làm backend để sync với nhiều cloud storage:
//! - Google Drive
//! - Dropbox
//! - OneDrive
//! - S3 compatible storage
//! - Và nhiều hơn nữa...
//!
//! Ưu điểm:
//! - Không cần user setup OAuth Client ID/Secret
//! - Rclone đã có sẵn OAuth credentials cho các providers phổ biến
//! - Hỗ trợ 40+ cloud providers
//! - Được bundle vào app, không cần user cài đặt riêng

use super::provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Remote name mặc định cho Google Drive
const DEFAULT_REMOTE_NAME: &str = "echovault-gdrive";

/// Remote path trên cloud storage
const DEFAULT_REMOTE_PATH: &str = "EchoVault";

/// Rclone sync provider
pub struct RcloneProvider {
    /// Đường dẫn đến rclone binary
    rclone_path: PathBuf,
    /// Tên remote đã cấu hình (e.g., "echovault-gdrive")
    remote_name: String,
    /// Đường dẫn trên remote (e.g., "EchoVault")
    remote_path: String,
    /// Remote đã được cấu hình chưa
    is_configured: bool,
}

impl RcloneProvider {
    /// Tạo provider mới với bundled rclone binary
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

    /// Tạo provider với custom remote name
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

    /// Tìm rclone binary - ưu tiên bundled, fallback system
    fn find_rclone_binary() -> PathBuf {
        // Thử tìm bundled rclone trước (Tauri sidecar)
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

                // Thử trong thư mục binaries (development mode)
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

        // Fallback: sử dụng system rclone
        if cfg!(windows) {
            PathBuf::from("rclone.exe")
        } else {
            PathBuf::from("rclone")
        }
    }

    /// Chạy rclone command và trả về output
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

    /// Chạy rclone command với output trực tiếp (cho interactive commands)
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

    /// Liệt kê các remotes đã cấu hình
    pub fn list_remotes(&self) -> Result<Vec<String>> {
        let output = self.run_rclone(&["listremotes"])?;
        let remotes: Vec<String> = output
            .lines()
            .map(|line| line.trim_end_matches(':').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(remotes)
    }

    /// Kiểm tra remote đã tồn tại chưa
    pub fn check_remote_exists(&self) -> Result<bool> {
        let remotes = self.list_remotes()?;
        Ok(remotes.contains(&self.remote_name))
    }

    /// Cấu hình remote mới (interactive)
    pub fn configure_remote(&self, remote_type: &str) -> Result<()> {
        println!("[Rclone] Đang cấu hình remote '{}'...", self.remote_name);
        println!("[Rclone] Browser sẽ mở để bạn đăng nhập.");

        // rclone config create <name> <type> --config
        // Với Google Drive: rclone config create echovault-gdrive drive
        self.run_rclone_interactive(&["config", "create", &self.remote_name, remote_type])?;

        Ok(())
    }

    /// Lấy remote URL đầy đủ (remote:path)
    fn get_remote_url(&self) -> String {
        format!("{}:{}", self.remote_name, self.remote_path)
    }

    /// Kiểm tra rclone có sẵn không
    pub fn check_rclone_available(&self) -> Result<bool> {
        match self.run_rclone(&["version"]) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Lấy phiên bản rclone
    pub fn get_version(&self) -> Result<String> {
        let output = self.run_rclone(&["version"])?;
        // Lấy dòng đầu tiên chứa version
        let version = output.lines().next().unwrap_or("Unknown").to_string();
        Ok(version)
    }

    /// Lấy remote name hiện tại
    pub fn remote_name(&self) -> &str {
        &self.remote_name
    }

    /// Lấy remote path hiện tại
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
        // Kiểm tra rclone có sẵn không
        if !self.check_rclone_available()? {
            bail!(
                "Rclone không tìm thấy. Vui lòng đảm bảo rclone đã được cài đặt hoặc bundle."
            );
        }

        // Chạy rclone config create với Google Drive
        // rclone sẽ tự mở browser để OAuth
        println!("[Rclone] Bắt đầu cấu hình Google Drive...");
        println!("[Rclone] Browser sẽ tự động mở để đăng nhập Google.");

        // Tạo remote với Google Drive
        let result = self.configure_remote("drive");

        if result.is_ok() {
            self.is_configured = self.check_remote_exists().unwrap_or(false);
            if self.is_configured {
                return Ok(AuthStatus::Authenticated);
            }
        }

        // Trả về pending để frontend có thể retry checking
        Ok(AuthStatus::Pending {
            user_code: "Đang cấu hình...".to_string(),
            verify_url: "Vui lòng hoàn tất trong browser".to_string(),
        })
    }

    fn complete_auth(&mut self) -> Result<AuthStatus> {
        // Kiểm tra lại xem remote đã được cấu hình chưa
        self.is_configured = self.check_remote_exists()?;

        if self.is_configured {
            Ok(AuthStatus::Authenticated)
        } else {
            Ok(AuthStatus::NotAuthenticated)
        }
    }

    fn pull(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PullResult> {
        if !self.is_configured {
            bail!("Remote chưa được cấu hình. Vui lòng chạy start_auth trước.");
        }

        let remote_url = self.get_remote_url();
        let local_path = vault_dir.to_string_lossy();

        println!(
            "[Rclone] Đang pull từ {} về {}...",
            remote_url, local_path
        );

        // rclone sync remote:path local_path
        // Sử dụng --verbose để có thể parse output sau
        let output = self.run_rclone(&[
            "sync",
            &remote_url,
            &local_path,
            "--verbose",
            "--stats-one-line",
        ])?;

        // Parse output để đếm files (simplified)
        let new_files = output.matches("Transferred:").count();

        Ok(PullResult {
            has_changes: new_files > 0,
            new_files,
            updated_files: 0,
        })
    }

    fn push(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PushResult> {
        if !self.is_configured {
            bail!("Remote chưa được cấu hình. Vui lòng chạy start_auth trước.");
        }

        let remote_url = self.get_remote_url();
        let local_path = vault_dir.to_string_lossy();

        println!(
            "[Rclone] Đang push từ {} lên {}...",
            local_path, remote_url
        );

        // rclone sync local_path remote:path
        let output = self.run_rclone(&[
            "sync",
            &local_path,
            &remote_url,
            "--verbose",
            "--stats-one-line",
        ])?;

        // Parse output để đếm files (simplified)
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

        // Nếu có output = có khác biệt
        match output {
            Ok(out) => Ok(!out.trim().is_empty()),
            Err(_) => Ok(true), // Assume có changes nếu check fail
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
