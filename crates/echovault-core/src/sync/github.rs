//! GitHub Provider - Sync vault với GitHub qua Git.
//!
//! Provider này sử dụng Git (qua command) và OAuth Device Flow.

use super::git::GitSync;
use super::oauth::{DeviceCodeResponse, OAuthCredentials, OAuthDeviceFlow};
use super::provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
use anyhow::{Context, Result};
use std::path::Path;

/// GitHub sync provider
pub struct GitHubProvider {
    /// Remote URL (e.g., https://github.com/user/repo.git)
    remote_url: Option<String>,
    /// GitHub credentials
    credentials: Option<OAuthCredentials>,
    /// GitHub OAuth handler
    auth: OAuthDeviceFlow,
    /// Pending device code (nếu đang chờ user xác thực)
    pending_auth: Option<DeviceCodeResponse>,
}

impl GitHubProvider {
    /// Tạo provider mới
    pub fn new() -> Self {
        Self {
            remote_url: None,
            credentials: None,
            auth: OAuthDeviceFlow::new(),
            pending_auth: None,
        }
    }

    /// Tạo provider với remote URL đã có
    pub fn with_remote(remote_url: String) -> Self {
        Self {
            remote_url: Some(remote_url),
            credentials: None,
            auth: OAuthDeviceFlow::new(),
            pending_auth: None,
        }
    }

    /// Set remote URL
    pub fn set_remote(&mut self, url: String) {
        self.remote_url = Some(url);
    }

    /// Set credentials
    pub fn set_credentials(&mut self, creds: OAuthCredentials) {
        self.credentials = Some(creds);
    }

    /// Lấy access token (nếu có)
    pub fn access_token(&self) -> Option<&str> {
        self.credentials.as_ref().map(|c| c.access_token.as_str())
    }

    /// Lấy remote URL
    pub fn remote_url(&self) -> Option<&str> {
        self.remote_url.as_deref()
    }

    /// Kiểm tra repo có tồn tại trên GitHub không
    pub fn repo_exists(&self, repo_name: &str) -> Result<bool> {
        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        // Gọi GitHub API để check repo
        let client = reqwest::blocking::Client::new();
        let response = client
            .get("https://api.github.com/user/repos?per_page=100")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "EchoVault")
            .header("Accept", "application/vnd.github+json")
            .send()
            .context("Failed to fetch repos from GitHub")?;

        if !response.status().is_success() {
            anyhow::bail!("GitHub API error: {}", response.status());
        }

        let repos: Vec<serde_json::Value> =
            response.json().context("Failed to parse GitHub response")?;

        // Tìm repo với tên phù hợp
        let exists = repos
            .iter()
            .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(repo_name));

        Ok(exists)
    }

    /// Clone repo về local
    pub fn clone_repo(&self, repo_name: &str) -> Result<()> {
        use crate::config::default_vault_path;

        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        // Lấy username từ GitHub API
        let username = self.get_username()?;
        let remote_url = format!("https://github.com/{}/{}.git", username, repo_name);
        let vault_path = default_vault_path();

        // Clone với token
        let git = super::git::GitSync::clone(&remote_url, &vault_path, token)?;

        // Set remote URL
        let _ = git; // Git clone đã xong

        Ok(())
    }

    /// Lấy username từ GitHub API
    pub fn get_username(&self) -> Result<String> {
        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let client = reqwest::blocking::Client::new();
        let response = client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "EchoVault")
            .header("Accept", "application/vnd.github+json")
            .send()
            .context("Failed to get user info from GitHub")?;

        if !response.status().is_success() {
            anyhow::bail!("GitHub API error: {}", response.status());
        }

        let user: serde_json::Value = response.json().context("Failed to parse user response")?;

        user.get("login")
            .and_then(|l| l.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Could not get username from GitHub"))
    }

    /// Tạo private repo mới trên GitHub
    pub fn create_repo(&self, repo_name: &str) -> Result<String> {
        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let client = reqwest::blocking::Client::new();

        // Tạo repo mới với GitHub API
        let body = serde_json::json!({
            "name": repo_name,
            "description": "EchoVault - Encrypted AI chat session backup",
            "private": true,
            "auto_init": false  // Không tạo initial commit để tránh conflict
        });

        let response = client
            .post("https://api.github.com/user/repos")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "EchoVault")
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .context("Failed to create repo on GitHub")?;

        if response.status() == 201 {
            // Repo created successfully
            let repo: serde_json::Value = response
                .json()
                .context("Failed to parse create repo response")?;
            let clone_url = repo
                .get("clone_url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            Ok(clone_url)
        } else if response.status() == 422 {
            // Repo already exists (validation failed)
            let username = self.get_username()?;
            Ok(format!("https://github.com/{}/{}.git", username, repo_name))
        } else {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            anyhow::bail!("Failed to create repo: {} - {}", status, error_text)
        }
    }
}

impl Default for GitHubProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncProvider for GitHubProvider {
    fn name(&self) -> &'static str {
        "github"
    }

    fn is_authenticated(&self) -> bool {
        self.credentials.is_some()
    }

    fn auth_status(&self) -> AuthStatus {
        if self.credentials.is_some() {
            AuthStatus::Authenticated
        } else if let Some(ref pending) = self.pending_auth {
            AuthStatus::Pending {
                user_code: pending.user_code.clone(),
                verify_url: pending.verification_uri.clone(),
            }
        } else {
            AuthStatus::NotAuthenticated
        }
    }

    fn start_auth(&mut self) -> Result<AuthStatus> {
        // Bắt đầu OAuth Device Flow
        let device_code = self
            .auth
            .request_device_code()
            .context("Failed to start device flow")?;

        let status = AuthStatus::Pending {
            user_code: device_code.user_code.clone(),
            verify_url: device_code.verification_uri.clone(),
        };

        self.pending_auth = Some(device_code);
        Ok(status)
    }

    fn complete_auth(&mut self) -> Result<AuthStatus> {
        let pending = self
            .pending_auth
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No pending auth"))?;

        // Poll một lần, không loop
        match self.auth.poll_once(pending) {
            Ok(Some(creds)) => {
                self.pending_auth = None;
                self.credentials = Some(creds);
                Ok(AuthStatus::Authenticated)
            }
            Ok(None) => {
                // Chưa authorize, trả về status pending để frontend biết
                Ok(AuthStatus::Pending {
                    user_code: pending.user_code.clone(),
                    verify_url: pending.verification_uri.clone(),
                })
            }
            Err(e) => Err(e),
        }
    }

    fn pull(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PullResult> {
        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let git = GitSync::open(vault_dir).context("Failed to open repository")?;

        let has_changes = git.pull("origin", "main", token)?;

        Ok(PullResult {
            has_changes,
            new_files: 0, // TODO: Track actual counts
            updated_files: 0,
        })
    }

    fn push(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PushResult> {
        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let git = GitSync::open(vault_dir).context("Failed to open repository")?;

        // Stage và commit nếu có changes
        if git.has_changes()? {
            git.stage_all()?;
            git.commit("Auto-sync from EchoVault")?;
        }

        // Lần push đầu tiên
        let success = git.push("origin", "main", token)?;

        if success {
            return Ok(PushResult {
                success: true,
                files_pushed: 0,
                message: None,
            });
        }

        // Push failed (likely repo doesn't exist), try to create it
        println!("[GitHubProvider::push] Push failed, attempting to create repo...");

        // Lấy repo name từ remote URL
        let remote_url = git.get_remote_url("origin")?;
        let repo_name = remote_url
            .split('/')
            .last()
            .unwrap_or("vault")
            .trim_end_matches(".git");

        // Tạo repo trên GitHub
        match self.create_repo(repo_name) {
            Ok(url) => {
                println!("[GitHubProvider::push] Created repo: {}", url);
            }
            Err(e) => {
                println!(
                    "[GitHubProvider::push] create_repo error (may already exist): {}",
                    e
                );
                // Continue anyway, repo might already exist
            }
        }

        // Retry push
        let success_retry = git.push("origin", "main", token)?;

        Ok(PushResult {
            success: success_retry,
            files_pushed: 0,
            message: if success_retry {
                None
            } else {
                Some("Push failed after creating repo".to_string())
            },
        })
    }

    fn has_local_changes(&self, vault_dir: &Path) -> Result<bool> {
        let git = GitSync::open(vault_dir)?;
        git.has_changes()
    }

    fn has_remote_changes(&self, vault_dir: &Path) -> Result<bool> {
        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let git = GitSync::open(vault_dir)?;
        git.fetch("origin", token)?;

        let (ahead, behind) = git.get_ahead_behind("origin", "main")?;
        Ok(behind > 0 || ahead > 0)
    }
}
