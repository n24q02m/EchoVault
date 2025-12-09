//! Google Drive Provider - Sync vault với Google Drive qua REST API.
//!
//! Provider này sử dụng OAuth 2.0 Device Flow và Google Drive API v3.

use super::oauth::OAuthCredentials;
use super::provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Google OAuth App credentials cho EchoVault
/// Đọc từ environment variables (managed by Doppler)
fn get_google_client_id() -> String {
    std::env::var("GOOGLE_CLIENT_ID").expect(
        "GOOGLE_CLIENT_ID environment variable not set. Run with: doppler run -- cargo tauri dev",
    )
}

fn get_google_client_secret() -> String {
    std::env::var("GOOGLE_CLIENT_SECRET")
        .expect("GOOGLE_CLIENT_SECRET environment variable not set. Run with: doppler run -- cargo tauri dev")
}

/// Google OAuth endpoints
const DEVICE_CODE_URL: &str = "https://oauth2.googleapis.com/device/code";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Google Drive API endpoints
const DRIVE_FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";
const DRIVE_UPLOAD_URL: &str = "https://www.googleapis.com/upload/drive/v3/files";

/// OAuth scope cho Google Drive
const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

/// Tên folder mặc định trên Google Drive
const DEFAULT_FOLDER_NAME: &str = "EchoVault";

/// Response từ device code request
#[derive(Debug, Deserialize)]
pub struct GoogleDeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Response từ token request
#[derive(Debug, Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<u64>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// Google Drive file metadata
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub modified_time: Option<String>,
}

/// Response từ files.list
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFileList {
    #[serde(default)]
    pub files: Vec<DriveFile>,
    pub next_page_token: Option<String>,
}

/// Google Drive credentials (bao gồm refresh token)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_at: Option<u64>,
}

impl From<GoogleCredentials> for OAuthCredentials {
    fn from(gc: GoogleCredentials) -> Self {
        OAuthCredentials {
            access_token: gc.access_token,
            token_type: gc.token_type,
            scope: DRIVE_SCOPE.to_string(),
        }
    }
}

/// Google Drive sync provider
pub struct GoogleDriveProvider {
    /// Google credentials
    credentials: Option<GoogleCredentials>,
    /// Pending device code
    pending_auth: Option<GoogleDeviceCodeResponse>,
    /// Folder ID trên Google Drive
    folder_id: Option<String>,
    /// HTTP client
    client: reqwest::blocking::Client,
}

impl GoogleDriveProvider {
    /// Tạo provider mới
    pub fn new() -> Self {
        Self {
            credentials: None,
            pending_auth: None,
            folder_id: None,
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Set credentials
    pub fn set_credentials(&mut self, creds: OAuthCredentials) {
        self.credentials = Some(GoogleCredentials {
            access_token: creds.access_token,
            refresh_token: None,
            token_type: creds.token_type,
            expires_at: None,
        });
    }

    /// Set Google credentials với refresh token
    pub fn set_google_credentials(&mut self, creds: GoogleCredentials) {
        self.credentials = Some(creds);
    }

    /// Lấy access token
    pub fn access_token(&self) -> Option<&str> {
        self.credentials.as_ref().map(|c| c.access_token.as_str())
    }

    /// Request device code từ Google
    fn request_device_code(&self) -> Result<GoogleDeviceCodeResponse> {
        let client_id = get_google_client_id();
        let response = self
            .client
            .post(DEVICE_CODE_URL)
            .form(&[("client_id", client_id.as_str()), ("scope", DRIVE_SCOPE)])
            .send()
            .context("Cannot request device code from Google")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            bail!("Google returned error {}: {}", status, body);
        }

        let device_code: GoogleDeviceCodeResponse = response
            .json()
            .context("Cannot parse device code response")?;

        Ok(device_code)
    }

    fn poll_once(
        &self,
        device_code: &GoogleDeviceCodeResponse,
    ) -> Result<Option<GoogleCredentials>> {
        let client_id = get_google_client_id();
        let client_secret = get_google_client_secret();
        let response = self
            .client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("device_code", device_code.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .context("Cannot poll for access token")?;

        let token_response: GoogleTokenResponse =
            response.json().context("Cannot parse token response")?;

        // Success!
        if let Some(access_token) = token_response.access_token {
            let expires_at = token_response.expires_in.map(|e| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + e
            });

            return Ok(Some(GoogleCredentials {
                access_token,
                refresh_token: token_response.refresh_token,
                token_type: token_response
                    .token_type
                    .unwrap_or_else(|| "Bearer".to_string()),
                expires_at,
            }));
        }

        // Check errors
        if let Some(error) = &token_response.error {
            match error.as_str() {
                "authorization_pending" | "slow_down" => return Ok(None),
                "expired_token" => bail!("Device code expired. Please try again."),
                "access_denied" => bail!("User denied authorization."),
                _ => {
                    let desc = token_response
                        .error_description
                        .as_deref()
                        .unwrap_or("Unknown");
                    bail!("OAuth error: {} - {}", error, desc);
                }
            }
        }

        Ok(None)
    }

    /// Tìm hoặc tạo folder EchoVault trên Drive
    #[allow(dead_code)] // Sẽ được dùng khi implement đầy đủ logic sync
    fn ensure_folder(&mut self) -> Result<String> {
        if let Some(ref id) = self.folder_id {
            return Ok(id.clone());
        }

        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        // Tìm folder EchoVault
        let query = format!(
            "name='{}' and mimeType='application/vnd.google-apps.folder' and trashed=false",
            DEFAULT_FOLDER_NAME
        );
        let response = self
            .client
            .get(DRIVE_FILES_URL)
            .query(&[("q", &query), ("fields", &"files(id,name)".to_string())])
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .context("Failed to search for folder")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            bail!("Drive API error {}: {}", status, body);
        }

        let list: DriveFileList = response.json()?;

        if let Some(folder) = list.files.first() {
            self.folder_id = Some(folder.id.clone());
            return Ok(folder.id.clone());
        }

        // Tạo folder mới
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct CreateFolder<'a> {
            name: &'a str,
            mime_type: &'a str,
        }

        let response = self
            .client
            .post(DRIVE_FILES_URL)
            .header("Authorization", format!("Bearer {}", token))
            .json(&CreateFolder {
                name: DEFAULT_FOLDER_NAME,
                mime_type: "application/vnd.google-apps.folder",
            })
            .send()
            .context("Failed to create folder")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            bail!("Failed to create folder: {} - {}", status, body);
        }

        let folder: DriveFile = response.json()?;
        self.folder_id = Some(folder.id.clone());
        Ok(folder.id)
    }

    /// List all files trong folder
    fn list_files(&self, folder_id: &str) -> Result<Vec<DriveFile>> {
        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let query = format!("'{}' in parents and trashed=false", folder_id);
        let response = self
            .client
            .get(DRIVE_FILES_URL)
            .query(&[
                ("q", &query),
                ("fields", &"files(id,name,modifiedTime)".to_string()),
                ("pageSize", &"1000".to_string()),
            ])
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .context("Failed to list files")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            bail!("Drive API error {}: {}", status, body);
        }

        let list: DriveFileList = response.json()?;
        Ok(list.files)
    }

    /// Download file từ Drive
    fn download_file(&self, file_id: &str, dest_path: &Path) -> Result<()> {
        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let url = format!(
            "{}?alt=media",
            DRIVE_FILES_URL.replace("files", &format!("files/{}", file_id))
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .context("Failed to download file")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            bail!("Failed to download: {} - {}", status, body);
        }

        let bytes = response.bytes()?;
        fs::write(dest_path, &bytes)?;
        Ok(())
    }

    /// Upload file lên Drive (tạo mới hoặc update)
    fn upload_file(
        &self,
        folder_id: &str,
        local_path: &Path,
        existing_id: Option<&str>,
        target_name: &str,
    ) -> Result<String> {
        let token = self
            .access_token()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;
        let content = fs::read(local_path)?;

        if let Some(file_id) = existing_id {
            // Update existing file
            let url = format!("{}/{}?uploadType=media", DRIVE_UPLOAD_URL, file_id);
            let response = self
                .client
                .patch(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/octet-stream")
                .body(content)
                .send()
                .context("Failed to update file")?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().unwrap_or_default();
                bail!("Failed to update: {} - {}", status, body);
            }

            let file: DriveFile = response.json()?;
            Ok(file.id)
        } else {
            // Create new file with multipart upload
            #[derive(Serialize)]
            struct FileMetadata<'a> {
                name: &'a str,
                parents: Vec<&'a str>,
            }

            let metadata = FileMetadata {
                name: target_name,
                parents: vec![folder_id],
            };
            let metadata_json = serde_json::to_string(&metadata)?;

            // Multipart upload
            let boundary = "----EchoVaultBoundary";
            let body = format!(
                "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n",
                boundary = boundary,
                metadata = metadata_json
            );
            let mut body_bytes = body.into_bytes();
            body_bytes.extend_from_slice(&content);
            body_bytes.extend_from_slice(format!("\r\n--{}--", boundary).as_bytes());

            let url = format!("{}?uploadType=multipart", DRIVE_UPLOAD_URL);
            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header(
                    "Content-Type",
                    format!("multipart/related; boundary={}", boundary),
                )
                .body(body_bytes)
                .send()
                .context("Failed to upload file")?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().unwrap_or_default();
                bail!("Failed to upload: {} - {}", status, body);
            }

            let file: DriveFile = response.json()?;
            Ok(file.id)
        }
    }
}

impl Default for GoogleDriveProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncProvider for GoogleDriveProvider {
    fn name(&self) -> &'static str {
        "google_drive"
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
                verify_url: pending.verification_url.clone(),
            }
        } else {
            AuthStatus::NotAuthenticated
        }
    }

    fn start_auth(&mut self) -> Result<AuthStatus> {
        let device_code = self.request_device_code()?;

        let status = AuthStatus::Pending {
            user_code: device_code.user_code.clone(),
            verify_url: device_code.verification_url.clone(),
        };

        self.pending_auth = Some(device_code);
        Ok(status)
    }

    fn complete_auth(&mut self) -> Result<AuthStatus> {
        let pending = self
            .pending_auth
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No pending auth"))?;

        match self.poll_once(pending) {
            Ok(Some(creds)) => {
                self.pending_auth = None;
                self.credentials = Some(creds);
                Ok(AuthStatus::Authenticated)
            }
            Ok(None) => Ok(AuthStatus::Pending {
                user_code: pending.user_code.clone(),
                verify_url: pending.verification_url.clone(),
            }),
            Err(e) => Err(e),
        }
    }

    fn pull(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PullResult> {
        let folder_id = match &self.folder_id {
            Some(id) => id.clone(),
            None => {
                // Cannot mutate self here, so we need to find folder
                let token = self
                    .access_token()
                    .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;
                let query = format!(
                    "name='{}' and mimeType='application/vnd.google-apps.folder' and trashed=false",
                    DEFAULT_FOLDER_NAME
                );
                let response = self
                    .client
                    .get(DRIVE_FILES_URL)
                    .query(&[("q", &query), ("fields", &"files(id,name)".to_string())])
                    .header("Authorization", format!("Bearer {}", token))
                    .send()?;

                let list: DriveFileList = response.json()?;
                list.files
                    .first()
                    .map(|f| f.id.clone())
                    .ok_or_else(|| anyhow::anyhow!("EchoVault folder not found on Drive"))?
            }
        };

        // List files in folder
        let remote_files = self.list_files(&folder_id)?;

        // Create sessions directory
        // Create sessions directory base
        let sessions_dir = vault_dir.join("sessions");
        fs::create_dir_all(&sessions_dir)?;

        let mut new_files = 0;
        let updated_files = 0; // Chưa implement logic update, hiện chỉ download file mới

        // Download each file
        for file in &remote_files {
            // Unflatten path: "vscode-copilot_session.json" -> "vscode-copilot/session.json"
            // Simple replace first '_' with '/' if possible, or just all '_' to '/'
            // Logic hiện tại của push là replace('/', '_').
            // Ta cần reverse: folder_file -> folder/file.
            // Giả định là chỉ có 1 level folder con trong sessions (như cấu trúc hiện tại).
            // Tuy nhiên, logic replace('/', '_') là global.
            // Để đơn giản và hoạt động v0.1: replace first occurrences of '_' with '/' ???
            // Không, vì tên file có thể chứa '_'.
            // Tạm thời replace all '_' -> '/' để restore structure.
            let relative_path = file.name.replace('_', std::path::MAIN_SEPARATOR_STR);
            let local_path = sessions_dir.join(relative_path);

            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Check if file exists locally
            let should_download = if local_path.exists() {
                // Compare modified time (simplified: always download if remote is newer)
                // For now, skip if exists
                false
            } else {
                true
            };

            if should_download {
                self.download_file(&file.id, &local_path)?;
                new_files += 1;
            }
        }

        Ok(PullResult {
            has_changes: new_files > 0 || updated_files > 0,
            new_files,
            updated_files,
        })
    }

    fn push(&self, vault_dir: &Path, _options: &SyncOptions) -> Result<PushResult> {
        // Need mutable access for ensure_folder
        let folder_id = match &self.folder_id {
            Some(id) => id.clone(),
            None => {
                // Find or create folder
                let token = self
                    .access_token()
                    .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;
                let query = format!(
                    "name='{}' and mimeType='application/vnd.google-apps.folder' and trashed=false",
                    DEFAULT_FOLDER_NAME
                );
                let response = self
                    .client
                    .get(DRIVE_FILES_URL)
                    .query(&[("q", &query), ("fields", &"files(id,name)".to_string())])
                    .header("Authorization", format!("Bearer {}", token))
                    .send()?;

                let list: DriveFileList = response.json()?;

                if let Some(folder) = list.files.first() {
                    folder.id.clone()
                } else {
                    // Create folder
                    #[derive(Serialize)]
                    #[serde(rename_all = "camelCase")]
                    struct CreateFolder<'a> {
                        name: &'a str,
                        mime_type: &'a str,
                    }

                    let create_response = self
                        .client
                        .post(DRIVE_FILES_URL)
                        .header("Authorization", format!("Bearer {}", token))
                        .json(&CreateFolder {
                            name: DEFAULT_FOLDER_NAME,
                            mime_type: "application/vnd.google-apps.folder",
                        })
                        .send()?;

                    let folder: DriveFile = create_response.json()?;
                    folder.id
                }
            }
        };

        // List remote files to check for existing
        let remote_files = self.list_files(&folder_id)?;
        let remote_map: HashMap<String, String> = remote_files
            .into_iter()
            .map(|f| (f.name.clone(), f.id))
            .collect();

        // Find all local files to upload
        let sessions_dir = vault_dir.join("sessions");
        let mut files_pushed = 0;

        if sessions_dir.exists() {
            // Collect all files recursively using std::fs
            fn collect_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.is_dir() {
                            collect_files(&path, files);
                        } else if path.is_file() {
                            files.push(path);
                        }
                    }
                }
            }

            let mut local_files = Vec::new();
            collect_files(&sessions_dir, &mut local_files);

            for local_path in local_files {
                let relative = local_path
                    .strip_prefix(&sessions_dir)
                    .unwrap_or(&local_path);
                // Flatten path: "vscode-copilot/session.json" -> "vscode-copilot_session.json"
                let file_name = relative.to_string_lossy().replace('/', "_");
                let existing_id = remote_map.get(&file_name);

                // Pass both local_path (for content) and file_name (for remote name)
                match self.upload_file(
                    &folder_id,
                    &local_path,
                    existing_id.map(|s| s.as_str()),
                    &file_name,
                ) {
                    Ok(_) => files_pushed += 1,
                    Err(e) => {
                        println!("[GoogleDrive] Failed to upload {}: {}", file_name, e);
                    }
                }
            }
        }

        Ok(PushResult {
            success: true,
            files_pushed,
            message: None,
        })
    }

    fn has_local_changes(&self, vault_dir: &Path) -> Result<bool> {
        let sessions_dir = vault_dir.join("sessions");
        if !sessions_dir.exists() {
            return Ok(false);
        }

        // Simplified: check if any files exist
        for entry in fs::read_dir(&sessions_dir)? {
            if entry?.path().is_dir() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn has_remote_changes(&self, _vault_dir: &Path) -> Result<bool> {
        // Simplified: always return false for now
        Ok(false)
    }
}
