//! Tauri commands - API giữa frontend và backend
//!
//! Các commands này được gọi từ frontend qua IPC.

use echovault_core::{AuthStatus, GitHubProvider, SyncOptions, SyncProvider};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;

/// State chứa provider hiện tại
#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<Mutex<GitHubProvider>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            provider: Arc::new(Mutex::new(GitHubProvider::new())),
        }
    }
}

/// Thông tin một session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub source: String,
    pub title: Option<String>,
    pub workspace_name: Option<String>,
    pub created_at: Option<String>,
    pub file_size: u64,
    pub path: String, // Absolute path to the original file
}

/// Kết quả scan - grouped theo source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
}

/// Trạng thái auth để frontend hiển thị
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatusResponse {
    pub status: String, // "not_authenticated", "pending", "authenticated", "error"
    pub user_code: Option<String>,
    pub verify_url: Option<String>,
    pub error: Option<String>,
}

impl From<AuthStatus> for AuthStatusResponse {
    fn from(status: AuthStatus) -> Self {
        match status {
            AuthStatus::NotAuthenticated => Self {
                status: "not_authenticated".to_string(),
                user_code: None,
                verify_url: None,
                error: None,
            },
            AuthStatus::Pending {
                user_code,
                verify_url,
            } => Self {
                status: "pending".to_string(),
                user_code: Some(user_code),
                verify_url: Some(verify_url),
                error: None,
            },
            AuthStatus::Authenticated => Self {
                status: "authenticated".to_string(),
                user_code: None,
                verify_url: None,
                error: None,
            },
            AuthStatus::Error(e) => Self {
                status: "error".to_string(),
                user_code: None,
                verify_url: None,
                error: Some(e),
            },
        }
    }
}

/// Config để frontend hiển thị
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub setup_complete: bool,
    pub provider: String,
    pub repo_name: Option<String>,
    pub encrypt: bool,
    pub compress: bool,
}

/// Setup request từ frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupRequest {
    pub provider: String,
    pub repo_name: String,
    pub encrypt: bool,
    pub compress: bool,
    pub passphrase: Option<String>,
}

// ============ SETUP COMMANDS ============

/// Kiểm tra đã setup chưa
#[tauri::command]
pub async fn check_setup_complete() -> Result<bool, String> {
    use echovault_core::Config;
    let config = Config::load_default().map_err(|e| e.to_string())?;
    Ok(config.setup_complete)
}

/// Hoàn tất setup
#[tauri::command]
pub async fn complete_setup(
    request: SetupRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use echovault_core::config::{
        default_config_path, CompressionConfig, EncryptionConfig, SyncConfig,
    };
    use echovault_core::sync::GitSync;
    use echovault_core::Config;
    use echovault_core::VaultMetadata;

    // Clone state cho spawn_blocking
    let provider_clone = state.provider.clone();

    // Lấy username từ GitHub API trong blocking context
    let username = tokio::task::spawn_blocking(move || {
        let provider = provider_clone.lock().map_err(|e| e.to_string())?;

        if provider.is_authenticated() {
            match provider.get_username() {
                Ok(name) => {
                    println!("[complete_setup] Got username from GitHub: {}", name);
                    Ok(name)
                }
                Err(e) => {
                    println!(
                        "[complete_setup] Failed to get username: {}, using 'user'",
                        e
                    );
                    Ok("user".to_string())
                }
            }
        } else {
            println!("[complete_setup] Not authenticated, using 'user'");
            Ok("user".to_string())
        }
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {}", e))?
    .map_err(|e: String| e)?;

    // Tạo remote URL
    let remote_url = format!("https://github.com/{}/{}.git", username, request.repo_name);
    println!("[complete_setup] Remote URL: {}", remote_url);

    let mut config = Config::load_default().map_err(|e| e.to_string())?;
    let vault_path = config.vault_path.clone();

    // Force encrypt và compress cho GitHub provider
    // (Encrypt luôn bật vì security, compress bắt buộc vì GitHub file size limits)
    let force_encrypt = true;
    let force_compress = request.provider == "github";

    config.setup_complete = true;
    config.sync = SyncConfig {
        remote: Some(remote_url.clone()),
        repo_name: Some(request.repo_name),
        provider: request.provider,
    };
    config.encryption = EncryptionConfig {
        enabled: force_encrypt,
    };
    config.compression = CompressionConfig {
        enabled: force_compress || request.compress,
    };

    config
        .save(&default_config_path())
        .map_err(|e| e.to_string())?;

    // Tạo vault directory và metadata nếu chưa tồn tại (new vault)
    if !VaultMetadata::exists(&vault_path) {
        println!("[complete_setup] Creating new vault at {:?}", vault_path);

        // Tạo vault directory
        std::fs::create_dir_all(&vault_path)
            .map_err(|e| format!("Failed to create vault directory: {}", e))?;

        // Tạo vault.json
        // Tạo vault.json với forced encrypt/compress
        let metadata = VaultMetadata::new(force_encrypt, force_compress || request.compress);
        metadata
            .save(&vault_path)
            .map_err(|e| format!("Failed to save vault metadata: {}", e))?;
        println!("[complete_setup] vault.json created");

        // Khởi tạo git repository
        let git = GitSync::init(&vault_path)
            .map_err(|e| format!("Failed to init git repository: {}", e))?;
        println!("[complete_setup] Git repository initialized");

        // Thêm remote 'origin'
        git.add_remote("origin", &remote_url)
            .map_err(|e| format!("Failed to add git remote: {}", e))?;
        println!("[complete_setup] Git remote 'origin' added: {}", remote_url);

        // Tạo initial commit
        git.stage_all()
            .map_err(|e| format!("Failed to stage files: {}", e))?;
        git.commit("Initial vault setup")
            .map_err(|e| format!("Failed to create initial commit: {}", e))?;
        println!("[complete_setup] Initial commit created");

        // Set remote cho provider
        let mut provider = state.provider.lock().map_err(|e| e.to_string())?;
        provider.set_remote(remote_url);
    }

    // Lưu passphrase nếu encryption enabled
    if request.encrypt {
        if let Some(ref passphrase) = request.passphrase {
            // Try keyring first
            let keyring_result = keyring::Entry::new("echovault", "passphrase")
                .and_then(|entry| entry.set_password(passphrase));

            if let Err(e) = keyring_result {
                println!(
                    "[complete_setup] Keyring failed: {}, using file fallback",
                    e
                );
            }

            // Always save to file as fallback (for WSL)
            save_passphrase_to_file(passphrase)
                .map_err(|e| format!("Failed to save passphrase: {}", e))?;
            println!("[complete_setup] Passphrase saved to file fallback");
        }
    }

    Ok(())
}

// ============ VAULT COMMANDS ============

/// Response cho vault metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMetadataResponse {
    pub exists: bool,
    pub encrypted: bool,
    pub compressed: bool,
}

/// Kiểm tra repo có tồn tại trên GitHub không
#[tauri::command]
pub async fn check_repo_exists(
    repo_name: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let provider = state.provider.clone();

    let exists = tokio::task::spawn_blocking(move || {
        let provider = provider.lock().map_err(|e| e.to_string())?;
        // Sử dụng GitHub API để check
        provider.repo_exists(&repo_name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: String| e)?;

    Ok(exists)
}

/// Clone vault từ remote về local
#[tauri::command]
pub async fn clone_vault(repo_name: String, state: State<'_, AppState>) -> Result<(), String> {
    let provider = state.provider.clone();

    tokio::task::spawn_blocking(move || {
        let provider = provider.lock().map_err(|e| e.to_string())?;
        provider.clone_repo(&repo_name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: String| e)?;

    Ok(())
}

/// Đọc vault metadata sau khi clone
#[tauri::command]
pub async fn get_vault_metadata() -> Result<VaultMetadataResponse, String> {
    use echovault_core::config::default_vault_path;
    use echovault_core::VaultMetadata;

    let vault_path = default_vault_path();

    if !VaultMetadata::exists(&vault_path) {
        return Ok(VaultMetadataResponse {
            exists: false,
            encrypted: false,
            compressed: false,
        });
    }

    let metadata = VaultMetadata::load(&vault_path).map_err(|e| e.to_string())?;

    Ok(VaultMetadataResponse {
        exists: true,
        encrypted: metadata.encrypted,
        compressed: metadata.compressed,
    })
}

/// Verify passphrase cho encrypted vault
#[tauri::command]
pub async fn verify_passphrase_cmd(passphrase: String) -> Result<bool, String> {
    use echovault_core::config::default_vault_path;

    let vault_path = default_vault_path();

    tokio::task::spawn_blocking(move || {
        echovault_core::verify_passphrase(&vault_path, &passphrase).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ============ AUTH COMMANDS ============

/// Initialize provider với saved credentials (gọi khi app start)
#[tauri::command]
pub async fn init_provider(state: State<'_, AppState>) -> Result<bool, String> {
    use echovault_core::config::default_credentials_path;
    use echovault_core::sync::load_credentials_from_file;
    use echovault_core::Config;

    println!("[init_provider] Starting...");

    // Load saved config
    let config = Config::load_default().map_err(|e| {
        println!("[init_provider] Failed to load config: {}", e);
        e.to_string()
    })?;

    println!("[init_provider] setup_complete: {}", config.setup_complete);
    println!("[init_provider] sync.remote: {:?}", config.sync.remote);

    if !config.setup_complete {
        println!("[init_provider] Setup not complete, skipping");
        return Ok(false);
    }

    // Try to load OAuth credentials từ saved file
    let creds_path = default_credentials_path();
    println!("[init_provider] Credentials path: {:?}", creds_path);
    println!(
        "[init_provider] Credentials file exists: {}",
        creds_path.exists()
    );

    if creds_path.exists() {
        match load_credentials_from_file(&creds_path) {
            Ok(creds) => {
                println!("[init_provider] Credentials loaded successfully");
                let mut provider = state.provider.lock().map_err(|e| e.to_string())?;
                provider.set_credentials(creds);

                if let Some(remote) = &config.sync.remote {
                    println!("[init_provider] Setting remote: {}", remote);
                    provider.set_remote(remote.clone());
                }

                println!("[init_provider] Provider initialized successfully");
                return Ok(true);
            }
            Err(e) => {
                println!("[init_provider] Failed to load credentials: {}", e);
                eprintln!("Failed to load credentials: {}", e);
            }
        }
    } else {
        println!("[init_provider] Credentials file does not exist");
    }

    Ok(false)
}

/// Lấy trạng thái auth hiện tại
#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    let provider = state.provider.lock().map_err(|e| e.to_string())?;
    Ok(provider.auth_status().into())
}

/// Bắt đầu auth flow - trả về user_code và verify_url
#[tauri::command]
pub async fn start_auth(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    // Clone provider to use in spawn_blocking
    let provider = state.provider.clone();

    let status = tokio::task::spawn_blocking(move || {
        let mut provider = provider.lock().map_err(|e| e.to_string())?;
        provider.start_auth().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: String| e)?;

    Ok(status.into())
}

/// Hoàn tất auth - poll cho token
#[tauri::command]
pub async fn complete_auth(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    use echovault_core::config::default_credentials_path;
    use echovault_core::sync::{save_credentials_to_file, OAuthCredentials};

    // Clone provider to use in spawn_blocking
    let provider = state.provider.clone();

    let status = tokio::task::spawn_blocking(move || {
        let mut provider = provider.lock().map_err(|e| e.to_string())?;
        let status = provider.complete_auth().map_err(|e| e.to_string())?;

        // Save credentials nếu auth thành công
        if let echovault_core::AuthStatus::Authenticated = &status {
            if let Some(token) = provider.access_token() {
                let creds = OAuthCredentials {
                    access_token: token.to_string(),
                    token_type: "bearer".to_string(),
                    scope: "repo".to_string(),
                };
                let creds_path = default_credentials_path();
                // Đảm bảo thư mục tồn tại
                if let Some(parent) = creds_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = save_credentials_to_file(&creds, &creds_path) {
                    eprintln!("Failed to save credentials: {}", e);
                }
            }
        }

        Ok(status)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: String| e)?;

    Ok(status.into())
}

// ============ SESSION COMMANDS ============

/// Helper: Tìm thông tin session từ vault directory
/// Đọc từ manifest files hoặc tạo default info nếu không tìm thấy
fn find_vault_session_info(
    vault_dir: &std::path::Path,
    session_id: &str,
    file_size: u64,
) -> SessionInfo {
    use std::fs;

    let sessions_dir = vault_dir.join("sessions");

    // Thử tìm trong các source directories
    let sources = ["vscode-copilot", "antigravity", "antigravity-artifact"];
    let mut found_source = "vault".to_string();
    let mut found_path = String::new();

    for source in &sources {
        let source_dir = sessions_dir.join(source);
        if source_dir.exists() {
            // Tìm manifest file hoặc .enc file
            let manifest_pattern = format!("{}.json.gz.enc.manifest", session_id);
            let enc_pattern = format!("{}.json.gz.enc", session_id);

            let manifest_path = source_dir.join(&manifest_pattern);
            let enc_path = source_dir.join(&enc_pattern);

            if manifest_path.exists() {
                found_source = source.to_string();
                found_path = manifest_path.to_string_lossy().to_string();
                break;
            } else if enc_path.exists() {
                found_source = source.to_string();
                found_path = enc_path.to_string_lossy().to_string();
                break;
            }
        }
    }

    // Nếu không tìm thấy path cụ thể, đặt path tới vault dir
    if found_path.is_empty() {
        found_path = sessions_dir.to_string_lossy().to_string();
    }

    // Thử đọc manifest để lấy thêm thông tin
    let title = if found_path.ends_with(".manifest") {
        if let Ok(content) = fs::read_to_string(&found_path) {
            // Parse manifest để lấy original_file name
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                json.get("original_file")
                    .and_then(|v| v.as_str())
                    .map(|s| s.replace(".json", ""))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    SessionInfo {
        id: session_id.to_string(),
        source: format!("{} (synced)", found_source),
        title,
        workspace_name: None,
        created_at: None, // Không có timestamp chính xác từ index
        file_size,
        path: found_path,
    }
}

/// Scan tất cả sessions có sẵn (async, offloaded to blocking thread)
#[tauri::command]
pub async fn scan_sessions() -> Result<ScanResult, String> {
    // Move heavy I/O work to spawn_blocking để không block Tauri runtime
    tokio::task::spawn_blocking(|| {
        use echovault_core::extractors::{
            antigravity::AntigravityExtractor, vscode_copilot::VSCodeCopilotExtractor, Extractor,
        };
        use echovault_core::Config;
        use std::collections::{HashMap, HashSet};

        let mut sessions = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();

        // 1. Scan VS Code Copilot sessions (local)
        let vscode_extractor = VSCodeCopilotExtractor::new();
        if let Ok(locations) = vscode_extractor.find_storage_locations() {
            for location in locations {
                if let Ok(files) = vscode_extractor.list_session_files(&location) {
                    for file in files {
                        let id = file.metadata.id.clone();
                        if seen_ids.insert(id) {
                            sessions.push(SessionInfo {
                                id: file.metadata.id,
                                source: file.metadata.source,
                                title: file.metadata.title,
                                workspace_name: file.metadata.workspace_name,
                                created_at: file.metadata.created_at.map(|d| d.to_rfc3339()),
                                file_size: file.metadata.file_size,
                                path: file.metadata.original_path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }

        // 2. Scan Antigravity sessions (local)
        let antigravity_extractor = AntigravityExtractor::new();
        if let Ok(locations) = antigravity_extractor.find_storage_locations() {
            for location in locations {
                if let Ok(files) = antigravity_extractor.list_session_files(&location) {
                    for file in files {
                        let id = file.metadata.id.clone();
                        if seen_ids.insert(id) {
                            sessions.push(SessionInfo {
                                id: file.metadata.id,
                                source: file.metadata.source,
                                title: file.metadata.title,
                                workspace_name: file.metadata.workspace_name,
                                created_at: file.metadata.created_at.map(|d| d.to_rfc3339()),
                                file_size: file.metadata.file_size,
                                path: file.metadata.original_path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }

        // 3. Read sessions from vault (synced from other machines)
        // The vault contains an index.json with metadata of all ingested sessions
        if let Ok(config) = Config::load_default() {
            let vault_dir = config.vault_path;
            let index_path = vault_dir.join("index.json");

            if index_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&index_path) {
                    // index.json format: { "session_id": (mtime, file_size), ... }
                    if let Ok(index) = serde_json::from_str::<HashMap<String, (u64, u64)>>(&content)
                    {
                        println!(
                            "[scan_sessions] Found {} entries in vault index",
                            index.len()
                        );

                        // For each session in index that we don't already have locally,
                        // add it as a "vault" session (synced from other machine)
                        for (session_id, (_mtime, file_size)) in index {
                            if seen_ids.insert(session_id.clone()) {
                                // This session is from another machine (not in local extractors)
                                // Try to find more info from the vault files
                                let session_info =
                                    find_vault_session_info(&vault_dir, &session_id, file_size);
                                sessions.push(session_info);
                            }
                        }
                    }
                }
            }
        }

        // Sort by created_at descending (newest first)
        sessions.sort_by(|a, b| {
            let a_time = a.created_at.as_deref().unwrap_or("");
            let b_time = b.created_at.as_deref().unwrap_or("");
            b_time.cmp(a_time)
        });

        let total = sessions.len();
        Ok(ScanResult { sessions, total })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Mở URL trong browser (hỗ trợ WSL)
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    use std::process::Command;

    // Kiểm tra nếu đang chạy trong WSL
    let is_wsl = std::fs::read_to_string("/proc/version")
        .map(|v| v.contains("microsoft") || v.contains("WSL"))
        .unwrap_or(false);

    if is_wsl {
        // Trong WSL, dùng cmd.exe để mở browser trong Windows
        Command::new("cmd.exe")
            .args(["/c", "start", "", &url])
            .spawn()
            .map_err(|e| format!("Failed to open URL in Windows: {}", e))?;
    } else {
        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open")
                .arg(&url)
                .spawn()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(["/c", "start", "", &url])
                .spawn()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg(&url)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// Mở file trong text editor
#[tauri::command]
pub async fn open_file(file_path: String) -> Result<(), String> {
    use std::process::Command;

    #[cfg(target_os = "linux")]
    {
        // WSL detection: check if running in WSL
        let is_wsl = std::fs::read_to_string("/proc/version")
            .map(|v| v.to_lowercase().contains("microsoft"))
            .unwrap_or(false);

        if is_wsl {
            // Convert Linux path to Windows path and open with notepad
            let win_path = Command::new("wslpath")
                .arg("-w")
                .arg(&file_path)
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| file_path.clone());

            Command::new("cmd.exe")
                .args(["/C", "notepad.exe", &win_path])
                .spawn()
                .map_err(|e| format!("Failed to open file: {}", e))?;
        } else {
            // Native Linux - use xdg-open
            Command::new("xdg-open")
                .arg(&file_path)
                .spawn()
                .map_err(|e| format!("Failed to open file: {}", e))?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("notepad")
            .arg(&file_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-t")
            .arg(&file_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ============ CONFIG COMMANDS ============

/// Lấy config hiện tại
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    use echovault_core::Config;

    let config = Config::load_default().map_err(|e| e.to_string())?;

    Ok(AppConfig {
        setup_complete: config.setup_complete,
        provider: config.sync.provider,
        repo_name: config.sync.repo_name,
        encrypt: config.encryption.enabled,
        compress: config.compression.enabled,
    })
}

// ============ SYNC COMMANDS ============

/// Path for fallback passphrase file (when keyring doesn't work in WSL)
fn passphrase_file_path() -> std::path::PathBuf {
    echovault_core::config::default_config_dir().join(".passphrase")
}

/// Helper: Save passphrase to file (fallback for WSL)
fn save_passphrase_to_file(passphrase: &str) -> Result<(), std::io::Error> {
    use std::fs;
    let path = passphrase_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Simple obfuscation (not secure, but better than plaintext)
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, passphrase);
    fs::write(&path, encoded)
}

/// Helper: Load passphrase from file (fallback for WSL)
fn load_passphrase_from_file() -> Option<String> {
    use std::fs;
    let path = passphrase_file_path();
    if path.exists() {
        if let Ok(encoded) = fs::read_to_string(&path) {
            if let Ok(decoded) =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded.trim())
            {
                return String::from_utf8(decoded).ok();
            }
        }
    }
    None
}

/// Helper: Load passphrase from keyring, fallback to file
fn get_passphrase() -> Result<Option<String>, keyring::Error> {
    // Try keyring first
    let entry = keyring::Entry::new("echovault", "passphrase")?;
    match entry.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => {
            // Fallback to file (for WSL)
            println!("[get_passphrase] Keyring empty, trying file fallback...");
            Ok(load_passphrase_from_file())
        }
        Err(e) => {
            // Try file fallback on any error
            println!(
                "[get_passphrase] Keyring error: {}, trying file fallback...",
                e
            );
            Ok(load_passphrase_from_file())
        }
    }
}

/// Helper: Ingest sessions (Scan -> Encrypt -> Save) with parallel processing
fn ingest_sessions(
    vault_dir: &std::path::Path,
    encryptor: Option<&echovault_core::crypto::Encryptor>,
    _compress: bool,
) -> Result<bool, String> {
    use echovault_core::extractors::{
        antigravity::AntigravityExtractor, vscode_copilot::VSCodeCopilotExtractor, Extractor,
    };
    use echovault_core::storage::chunked::compress_encrypt_chunk;
    use rayon::prelude::*;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    // Configure thread pool: use num_cpus - 2: (minimum 1)
    let num_threads = std::cmp::max(1, num_cpus::get().saturating_sub(2));
    println!(
        "[ingest_sessions] Using {} threads for parallel processing",
        num_threads
    );

    // Build custom thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .map_err(|e| format!("Failed to build thread pool: {}", e))?;

    println!("[ingest_sessions] Starting scan...");

    let mut sessions = Vec::new();

    // 1. Scan VS Code Copilot sessions
    let vscode_extractor = VSCodeCopilotExtractor::new();
    if let Ok(locations) = vscode_extractor.find_storage_locations() {
        println!(
            "[ingest_sessions] VS Code Copilot: {} locations",
            locations.len()
        );
        for location in &locations {
            if let Ok(files) = vscode_extractor.list_session_files(location) {
                println!(
                    "[ingest_sessions] Location {:?}: {} files",
                    location,
                    files.len()
                );
                sessions.extend(files);
            }
        }
    }

    // 2. Scan Antigravity sessions
    let antigravity_extractor = AntigravityExtractor::new();
    if let Ok(locations) = antigravity_extractor.find_storage_locations() {
        println!(
            "[ingest_sessions] Antigravity: {} locations",
            locations.len()
        );
        for location in &locations {
            if let Ok(files) = antigravity_extractor.list_session_files(location) {
                println!(
                    "[ingest_sessions] Location {:?}: {} files",
                    location,
                    files.len()
                );
                sessions.extend(files);
            }
        }
    }

    let total_sessions = sessions.len();
    println!(
        "[ingest_sessions] Total sessions to check: {}",
        total_sessions
    );

    // 2. Load deduplication index (simple JSON)
    let index_path = vault_dir.join("index.json");
    let index: HashMap<String, (u64, u64)> = if index_path.exists() {
        fs::read_to_string(&index_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };

    let sessions_dir = vault_dir.join("sessions");
    fs::create_dir_all(&sessions_dir).map_err(|e| e.to_string())?;

    // 3. Filter sessions that need processing
    let sessions_to_process: Vec<_> = sessions
        .into_iter()
        .filter_map(|session| {
            let source_path = &session.metadata.original_path;
            let file_size = session.metadata.file_size;

            // Skip if source file doesn't exist
            let metadata = match fs::metadata(source_path) {
                Ok(m) => m,
                Err(_) => return None,
            };

            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Check against index
            let should_process = if let Some(&(cached_mtime, cached_size)) =
                index.get(session.metadata.id.as_str())
            {
                cached_mtime != mtime || cached_size != file_size
            } else {
                true
            };

            if should_process {
                Some((session, mtime, file_size))
            } else {
                None
            }
        })
        .collect();

    let to_process = sessions_to_process.len();
    let skipped = total_sessions - to_process;
    println!(
        "[ingest_sessions] To process: {}, Already up-to-date: {}",
        to_process, skipped
    );

    if to_process == 0 {
        println!("[ingest_sessions] Nothing to process, complete");
        return Ok(false);
    }

    // 4. Process sessions in parallel
    let processed = AtomicUsize::new(0);
    let errors = Mutex::new(Vec::<String>::new());
    let new_index_entries = Mutex::new(Vec::<(String, (u64, u64))>::new());

    pool.install(|| {
        sessions_to_process
            .par_iter()
            .for_each(|(session, mtime, file_size)| {
                let source_path = &session.metadata.original_path;
                let dest_dir = sessions_dir.join(&session.metadata.source);

                // Create dest dir (may race but that's fine)
                if let Err(e) = fs::create_dir_all(&dest_dir) {
                    errors
                        .lock()
                        .unwrap()
                        .push(format!("Failed to create dir {:?}: {}", dest_dir, e));
                    return;
                }

                let current = processed.fetch_add(1, Ordering::Relaxed) + 1;
                if current.is_multiple_of(10) || current == to_process {
                    println!(
                        "[ingest_sessions] Progress: {}/{} ({}%)",
                        current,
                        to_process,
                        current * 100 / to_process
                    );
                }

                if let Some(enc) = encryptor {
                    // Encrypt + Compress + Chunk
                    if let Err(e) = compress_encrypt_chunk(source_path, &dest_dir, enc) {
                        errors
                            .lock()
                            .unwrap()
                            .push(format!("Failed to process {}: {}", session.metadata.id, e));
                        return;
                    }
                } else {
                    // Copy raw
                    let dest_path = dest_dir.join(format!("{}.json", session.metadata.id));
                    if let Err(e) = fs::copy(source_path, &dest_path) {
                        errors
                            .lock()
                            .unwrap()
                            .push(format!("Failed to copy {}: {}", session.metadata.id, e));
                        return;
                    }
                }

                // Record for index update
                new_index_entries
                    .lock()
                    .unwrap()
                    .push((session.metadata.id.clone(), (*mtime, *file_size)));
            });
    });

    // Check for errors
    let errs = errors.into_inner().unwrap();
    if !errs.is_empty() {
        println!("[ingest_sessions] {} errors occurred:", errs.len());
        for e in &errs {
            println!("  - {}", e);
        }
        // Continue anyway, just log errors
    }

    // 5. Update and save index
    let new_entries = new_index_entries.into_inner().unwrap();
    if !new_entries.is_empty() {
        let mut index = index; // Take ownership
        for (id, entry) in new_entries {
            index.insert(id, entry);
        }
        let index_json = serde_json::to_string_pretty(&index).map_err(|e| e.to_string())?;
        fs::write(&index_path, index_json).map_err(|e| e.to_string())?;
        println!("[ingest_sessions] Index saved with {} entries", index.len());
    }

    println!(
        "[ingest_sessions] Complete: {} processed, {} skipped",
        processed.load(Ordering::Relaxed),
        skipped
    );
    Ok(true)
}

/// Sync vault lên remote (Auto: Ingest -> Push)
#[tauri::command]
pub async fn sync_vault(state: State<'_, AppState>) -> Result<bool, String> {
    use echovault_core::crypto::{key_derivation::derive_key, Encryptor};
    use echovault_core::Config;
    use echovault_core::VaultMetadata;

    println!("[sync_vault] Starting...");

    // Check auth status early and drop lock immediately
    {
        let provider = state.provider.lock().map_err(|e| {
            println!("[sync_vault] Failed to lock provider: {}", e);
            e.to_string()
        })?;
        println!(
            "[sync_vault] is_authenticated: {}",
            provider.is_authenticated()
        );
        if !provider.is_authenticated() {
            println!("[sync_vault] Not authenticated, returning error");
            return Err("Not authenticated".to_string());
        }
    }

    println!("[sync_vault] Auth check passed");

    let config = Config::load_default().map_err(|e| {
        println!("[sync_vault] Failed to load config: {}", e);
        e.to_string()
    })?;
    let vault_dir = config.vault_path.clone();
    println!("[sync_vault] vault_dir: {:?}", vault_dir);

    // 1. Prepare Encryptor
    println!(
        "[sync_vault] Preparing encryptor, encryption.enabled: {}",
        config.encryption.enabled
    );
    let encryptor = if config.encryption.enabled {
        let passphrase = get_passphrase()
            .map_err(|e| {
                println!("[sync_vault] Keyring error: {}", e);
                format!("Keyring error: {}", e)
            })?
            .ok_or_else(|| {
                println!("[sync_vault] Passphrase not found in keyring");
                "Passphrase not found in keyring".to_string()
            })?;

        println!("[sync_vault] Passphrase loaded, loading vault metadata...");

        // Load salt from vault.json
        let metadata = VaultMetadata::load(&vault_dir).map_err(|e| {
            println!("[sync_vault] Failed to load vault metadata: {}", e);
            e.to_string()
        })?;
        let salt_bytes = metadata
            .salt_bytes()
            .map_err(|e| {
                println!("[sync_vault] Failed to get salt bytes: {}", e);
                e.to_string()
            })?
            .ok_or_else(|| {
                println!("[sync_vault] Missing salt in encrypted vault");
                "Missing salt in encrypted vault".to_string()
            })?;

        // Derive key
        let mut salt_arr = [0u8; 16]; // SALT_LEN
        if salt_bytes.len() != 16 {
            println!("[sync_vault] Invalid salt length: {}", salt_bytes.len());
            return Err("Invalid salt length".to_string());
        }
        salt_arr.copy_from_slice(&salt_bytes);

        let key = derive_key(&passphrase, &salt_arr).map_err(|e| {
            println!("[sync_vault] Failed to derive key: {}", e);
            e.to_string()
        })?;
        println!("[sync_vault] Key derived successfully");
        Some(Encryptor::new(&key))
    } else {
        None
    };

    // 2. Pull from Remote (get changes from other machines first)
    println!("[sync_vault] Pulling from remote...");
    let vault_dir_for_pull = vault_dir.clone();
    let provider_for_pull = state.provider.clone();
    let options_for_pull = SyncOptions {
        encrypt: config.encryption.enabled,
        compress: config.compression.enabled,
    };

    let pull_result = tokio::task::spawn_blocking(move || {
        let provider = provider_for_pull.lock().map_err(|e| e.to_string())?;
        provider
            .pull(&vault_dir_for_pull, &options_for_pull)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;

    match pull_result {
        Ok(result) => {
            println!(
                "[sync_vault] Pull complete: has_changes={}",
                result.has_changes
            );
        }
        Err(e) => {
            // Pull failure is not fatal - might be first sync or network issue
            println!("[sync_vault] Pull failed (continuing anyway): {}", e);
        }
    }

    // 3. Ingest Sessions (local -> vault)
    println!("[sync_vault] Ingesting sessions...");
    let _ingest_changes =
        ingest_sessions(&vault_dir, encryptor.as_ref(), config.compression.enabled)?;
    println!("[sync_vault] Ingest complete");

    // 4. Push to Remote
    println!("[sync_vault] Pushing to remote...");
    let options = SyncOptions {
        encrypt: config.encryption.enabled,
        compress: config.compression.enabled,
    };

    // Use spawn_blocking for long running git operations
    let vault_dir_clone = vault_dir.clone(); // Clone for closure
    let provider_clone = state.provider.clone(); // Use cloned provider for thread safety

    let result = tokio::task::spawn_blocking(move || {
        let provider = provider_clone.lock().map_err(|e| e.to_string())?;
        println!("[sync_vault] Calling provider.push...");
        provider.push(&vault_dir_clone, &options).map_err(|e| {
            println!("[sync_vault] Push failed: {}", e);
            e.to_string()
        })
    })
    .await
    .map_err(|e| {
        println!("[sync_vault] spawn_blocking failed: {}", e);
        e.to_string()
    })??;

    println!("[sync_vault] Push result: success={}", result.success);
    Ok(result.success)
}
