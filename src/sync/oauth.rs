//! GitHub OAuth Device Flow cho CLI authentication.
//!
//! OAuth Device Flow cho phép CLI apps authenticate mà không cần:
//! - Copy/paste Personal Access Token
//! - Setup SSH key
//!
//! Flow:
//! 1. CLI request device code từ GitHub
//! 2. User mở browser và nhập code
//! 3. CLI poll để lấy access token
//! 4. Access token được lưu để sử dụng cho git operations

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// GitHub OAuth App Client ID cho EchoVault
/// Note: Đây là public client, client_secret không cần thiết cho Device Flow
const GITHUB_CLIENT_ID: &str = "Ov23li17qiwKE2rOq5fx";

/// GitHub OAuth endpoints
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

/// Response từ device code request
#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    /// Code để gửi cho GitHub khi poll
    pub device_code: String,
    /// Code để user nhập vào browser
    pub user_code: String,
    /// URL để user mở (github.com/login/device)
    pub verification_uri: String,
    /// Thời gian device_code còn hiệu lực (seconds)
    pub expires_in: u64,
    /// Khoảng thời gian tối thiểu giữa các lần poll (seconds)
    pub interval: u64,
}

/// Response từ access token request
#[derive(Debug, Deserialize)]
pub struct AccessTokenResponse {
    /// Access token (nếu thành công)
    pub access_token: Option<String>,
    /// Token type (bearer)
    pub token_type: Option<String>,
    /// Scopes đã granted
    pub scope: Option<String>,
    /// Error code (nếu chưa authorized)
    pub error: Option<String>,
    /// Error description
    pub error_description: Option<String>,
}

/// Lưu trữ OAuth credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredentials {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

/// OAuth Device Flow implementation
pub struct OAuthDeviceFlow {
    client: reqwest::blocking::Client,
    client_id: String,
}

impl Default for OAuthDeviceFlow {
    fn default() -> Self {
        Self::new()
    }
}

impl OAuthDeviceFlow {
    /// Tạo instance mới với default client_id
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            client_id: GITHUB_CLIENT_ID.to_string(),
        }
    }

    /// Tạo instance với custom client_id (cho testing)
    #[allow(dead_code)]
    pub fn with_client_id(client_id: &str) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            client_id: client_id.to_string(),
        }
    }

    /// Bước 1: Request device code
    /// User sẽ cần mở verification_uri và nhập user_code
    pub fn request_device_code(&self) -> Result<DeviceCodeResponse> {
        let response = self
            .client
            .post(DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", &self.client_id),
                ("scope", &"repo".to_string()),
            ])
            .send()
            .context("Cannot request device code from GitHub")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            bail!("GitHub returned error {}: {}", status, body);
        }

        let device_code: DeviceCodeResponse = response
            .json()
            .context("Cannot parse device code response")?;

        Ok(device_code)
    }

    /// Bước 2: Poll cho access token
    /// Gọi hàm này sau khi user đã authorize trong browser
    pub fn poll_for_token(&self, device_code: &DeviceCodeResponse) -> Result<OAuthCredentials> {
        let start = Instant::now();
        let timeout = Duration::from_secs(device_code.expires_in);
        // GitHub yêu cầu interval tối thiểu, thường là 5 giây
        let interval = Duration::from_secs(device_code.interval.max(5));

        // Tạo spinner để hiển thị progress
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        spinner.enable_steady_tick(Duration::from_millis(100));

        // Đợi 1 giây đầu để user có thời gian mở browser
        std::thread::sleep(Duration::from_secs(1));

        loop {
            // Check timeout
            let elapsed = start.elapsed();
            if elapsed > timeout {
                spinner.finish_and_clear();
                bail!("Device code expired. Please try again.");
            }

            let remaining = (timeout - elapsed).as_secs();
            spinner.set_message(format!(
                "Đang chờ xác thực từ browser... (còn {} giây)",
                remaining
            ));

            // Poll for token TRƯỚC, rồi mới sleep
            let response = self
                .client
                .post(ACCESS_TOKEN_URL)
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", &self.client_id),
                    ("device_code", &device_code.device_code),
                    (
                        "grant_type",
                        &"urn:ietf:params:oauth:grant-type:device_code".to_string(),
                    ),
                ])
                .send()
                .context("Cannot poll for access token")?;

            // Parse response
            let response_text = response.text().context("Cannot read response")?;
            
            // DEBUG: In ra response để xem GitHub trả về gì
            // spinner.suspend(|| {
            //     eprintln!("[DEBUG] GitHub response: {}", response_text);
            // });
            
            let token_response: AccessTokenResponse = serde_json::from_str(&response_text)
                .context(format!("Cannot parse access token response: {}", response_text))?;

            // Check for errors
            if let Some(error) = &token_response.error {
                match error.as_str() {
                    "authorization_pending" => {
                        // User chưa authorize, sleep rồi tiếp tục poll
                        std::thread::sleep(interval);
                        continue;
                    }
                    "slow_down" => {
                        // Cần poll chậm hơn
                        std::thread::sleep(interval + Duration::from_secs(5));
                        continue;
                    }
                    "expired_token" => {
                        spinner.finish_and_clear();
                        bail!("Device code expired. Please try again.");
                    }
                    "access_denied" => {
                        spinner.finish_and_clear();
                        bail!("User denied authorization.");
                    }
                    _ => {
                        spinner.finish_and_clear();
                        let desc = token_response
                            .error_description
                            .as_deref()
                            .unwrap_or("Unknown error");
                        bail!("OAuth error: {} - {}", error, desc);
                    }
                }
            }

            // Success!
            if let Some(access_token) = token_response.access_token {
                spinner.finish_and_clear();
                return Ok(OAuthCredentials {
                    access_token,
                    token_type: token_response
                        .token_type
                        .unwrap_or_else(|| "bearer".to_string()),
                    scope: token_response.scope.unwrap_or_default(),
                });
            }

            // Unexpected response - không có error nhưng cũng không có token
            // Tiếp tục poll thay vì bail
            std::thread::sleep(interval);
        }
    }

    /// Full flow: request device code + poll for token
    /// Trả về callback để hiển thị instructions cho user
    pub fn authenticate<F>(&self, display_instructions: F) -> Result<OAuthCredentials>
    where
        F: FnOnce(&DeviceCodeResponse),
    {
        // Request device code
        let device_code = self.request_device_code()?;

        // Display instructions
        display_instructions(&device_code);

        // Poll for token
        self.poll_for_token(&device_code)
    }
}

/// Lưu credentials vào keyring (system keychain)
#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub fn save_credentials(credentials: &OAuthCredentials) -> Result<()> {
    // Trên Linux, sử dụng secret-service hoặc file
    let keyring = keyring::Entry::new("echovault", "github_token")?;
    keyring.set_password(&credentials.access_token)?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn save_credentials(credentials: &OAuthCredentials) -> Result<()> {
    let keyring = keyring::Entry::new("echovault", "github_token")?;
    keyring.set_password(&credentials.access_token)?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn save_credentials(credentials: &OAuthCredentials) -> Result<()> {
    let keyring = keyring::Entry::new("echovault", "github_token")?;
    keyring.set_password(&credentials.access_token)?;
    Ok(())
}

/// Fallback: Lưu vào file (không sử dụng keyring)
pub fn save_credentials_to_file(
    credentials: &OAuthCredentials,
    path: &std::path::Path,
) -> Result<()> {
    let json = serde_json::to_string_pretty(credentials)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load credentials từ file
pub fn load_credentials_from_file(path: &std::path::Path) -> Result<OAuthCredentials> {
    let json = std::fs::read_to_string(path)?;
    let credentials: OAuthCredentials = serde_json::from_str(&json)?;
    Ok(credentials)
}

/// Kiểm tra GitHub repository có tồn tại không
/// Trả về Ok(true) nếu repo tồn tại, Ok(false) nếu không tồn tại
pub fn check_repo_exists(repo_url: &str, access_token: &str) -> Result<bool> {
    // Parse repo URL để lấy owner/repo
    // https://github.com/owner/repo.git -> owner/repo
    let repo_path = repo_url
        .trim_end_matches(".git")
        .trim_start_matches("https://github.com/")
        .trim_start_matches("git@github.com:");

    let api_url = format!("https://api.github.com/repos/{}", repo_path);

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&api_url)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "EchoVault")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .context("Cannot connect to GitHub API")?;

    match response.status().as_u16() {
        200 => Ok(true),  // Repo exists
        404 => Ok(false), // Repo not found
        status => {
            let body = response.text().unwrap_or_default();
            bail!("GitHub API error {}: {}", status, body);
        }
    }
}

/// Tạo GitHub repository mới qua API
/// Trả về URL của repository được tạo
pub fn create_github_repo(repo_name: &str, access_token: &str, private: bool) -> Result<String> {
    let client = reqwest::blocking::Client::new();

    #[derive(Serialize)]
    struct CreateRepoRequest<'a> {
        name: &'a str,
        description: &'a str,
        private: bool,
        auto_init: bool,
    }

    let request = CreateRepoRequest {
        name: repo_name,
        description: "EchoVault - AI Chat History Backup",
        private,
        auto_init: false, // Không tạo README để tránh conflict
    };

    let response = client
        .post("https://api.github.com/user/repos")
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "EchoVault")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&request)
        .send()
        .context("Cannot connect to GitHub API")?;

    if response.status().is_success() {
        #[derive(Deserialize)]
        struct CreateRepoResponse {
            clone_url: String,
        }

        let repo: CreateRepoResponse = response.json()?;
        Ok(repo.clone_url)
    } else {
        let status = response.status();
        let body = response.text().unwrap_or_default();

        if status.as_u16() == 422 && body.contains("name already exists") {
            bail!(
                "Repository '{}' already exists on your GitHub account",
                repo_name
            );
        }

        bail!("GitHub API error {}: {}", status, body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_load_credentials_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let creds_path = temp_dir.path().join("credentials.json");

        let credentials = OAuthCredentials {
            access_token: "test_token_123".to_string(),
            token_type: "bearer".to_string(),
            scope: "repo".to_string(),
        };

        save_credentials_to_file(&credentials, &creds_path)?;
        let loaded = load_credentials_from_file(&creds_path)?;

        assert_eq!(loaded.access_token, credentials.access_token);
        assert_eq!(loaded.token_type, credentials.token_type);

        Ok(())
    }

    // Note: Không test actual OAuth flow vì cần network access
    // và user interaction. Sử dụng integration tests cho điều này.
}
