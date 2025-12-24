//! Tauri commands - API giữa frontend và backend
//!
//! Các commands này được gọi từ frontend qua IPC.
//! Simplified version - only Rclone provider, no encryption.

use echovault_core::{
    AuthStatus, Config, RcloneProvider, SyncOptions, SyncProvider, VaultMetadata,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;
use tracing::{error, info, warn};

/// State chứa RcloneProvider
#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<Mutex<RcloneProvider>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            provider: Arc::new(Mutex::new(RcloneProvider::new())),
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
    pub path: String,
}

/// Kết quả scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
}

/// Trạng thái auth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatusResponse {
    pub status: String,
    pub message: Option<String>,
}

impl From<AuthStatus> for AuthStatusResponse {
    fn from(status: AuthStatus) -> Self {
        match status {
            AuthStatus::NotAuthenticated => Self {
                status: "not_authenticated".to_string(),
                message: None,
            },
            AuthStatus::Authenticated => Self {
                status: "authenticated".to_string(),
                message: Some("Connected to cloud storage".to_string()),
            },
            AuthStatus::Pending {
                user_code,
                verify_url,
            } => Self {
                status: "pending".to_string(),
                message: Some(format!("Enter code {} at {}", user_code, verify_url)),
            },
            AuthStatus::Error(message) => Self {
                status: "error".to_string(),
                message: Some(message),
            },
        }
    }
}

/// Config để frontend hiển thị
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub setup_complete: bool,
    pub vault_path: String,
    pub remote_name: Option<String>,
    pub folder_name: String,
}

/// Setup request từ frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupRequest {
    pub folder_name: String,
}

// ============ SETUP COMMANDS ============

/// Kiểm tra đã setup chưa
#[tauri::command]
pub async fn check_setup_complete() -> Result<bool, String> {
    let config = Config::load_default().map_err(|e| e.to_string())?;
    Ok(config.setup_complete)
}

/// Hoàn tất setup
#[tauri::command]
pub async fn complete_setup(
    request: SetupRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use echovault_core::config::default_config_path;

    info!(
        "[complete_setup] Starting setup with folder: {}",
        request.folder_name
    );

    let mut config = Config::load_default().map_err(|e| e.to_string())?;
    let vault_path = config.vault_path.clone();

    // Check if Rclone remote is configured
    let provider = state.provider.lock().map_err(|e| e.to_string())?;
    if !provider.check_remote_exists().unwrap_or(false) {
        return Err("Please connect to cloud storage first".to_string());
    }
    drop(provider);

    // Update config
    config.setup_complete = true;
    config.sync.remote_name = Some("echovault".to_string());
    config.sync.folder_name = request.folder_name;

    config
        .save(&default_config_path())
        .map_err(|e| e.to_string())?;

    // Create vault directory if not exists
    if !vault_path.exists() {
        std::fs::create_dir_all(&vault_path)
            .map_err(|e| format!("Failed to create vault directory: {}", e))?;
        info!("[complete_setup] Vault directory created: {:?}", vault_path);
    }

    // Create vault metadata
    if !VaultMetadata::exists(&vault_path) {
        let metadata = VaultMetadata::new();
        metadata
            .save(&vault_path)
            .map_err(|e| format!("Failed to save vault metadata: {}", e))?;
        info!("[complete_setup] vault.json created");
    }

    info!("[complete_setup] Setup complete!");
    Ok(())
}

// ============ CONFIG COMMANDS ============

/// Lấy config hiện tại
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    let config = Config::load_default().map_err(|e| e.to_string())?;
    Ok(AppConfig {
        setup_complete: config.setup_complete,
        vault_path: config.vault_path.to_string_lossy().to_string(),
        remote_name: config.sync.remote_name,
        folder_name: config.sync.folder_name,
    })
}

// ============ AUTH COMMANDS ============

/// Lấy trạng thái auth hiện tại
#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    let provider = state.provider.lock().map_err(|e| e.to_string())?;
    Ok(AuthStatusResponse::from(provider.auth_status()))
}

/// Bắt đầu auth flow - mở browser để user đăng nhập
#[tauri::command]
pub async fn start_auth(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    let mut provider = state.provider.lock().map_err(|e| e.to_string())?;
    let status = provider.start_auth().map_err(|e| e.to_string())?;
    Ok(AuthStatusResponse::from(status))
}

/// Hoàn tất auth - check xem user đã đăng nhập chưa
#[tauri::command]
pub async fn complete_auth(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    let mut provider = state.provider.lock().map_err(|e| e.to_string())?;
    let status = provider.complete_auth().map_err(|e| e.to_string())?;
    Ok(AuthStatusResponse::from(status))
}

// ============ SESSION COMMANDS ============

/// Scan tất cả sessions có sẵn (local + synced từ vault)
#[tauri::command]
pub async fn scan_sessions() -> Result<ScanResult, String> {
    use echovault_core::extractors::{
        antigravity::AntigravityExtractor, cline::ClineExtractor, cursor::CursorExtractor,
        vscode_copilot::VSCodeCopilotExtractor, Extractor,
    };
    use std::collections::HashSet;

    let sessions = tokio::task::spawn_blocking(move || {
        let mut all_sessions = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();

        // 1. Scan local sessions từ VSCodeCopilotExtractor
        let extractor = VSCodeCopilotExtractor::new();
        if let Ok(locations) = extractor.find_storage_locations() {
            for location in locations {
                if let Ok(files) = extractor.list_session_files(&location) {
                    for file in files {
                        let id = file.metadata.id.clone();
                        if seen_ids.insert(id) {
                            all_sessions.push(SessionInfo {
                                id: file.metadata.id,
                                source: file.metadata.source,
                                title: file.metadata.title,
                                workspace_name: file.metadata.workspace_name,
                                created_at: file.metadata.created_at.map(|d| d.to_rfc3339()),
                                file_size: file.metadata.file_size,
                                path: file.source_path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }

        // 2. Scan Antigravity artifacts
        let antigravity_extractor = AntigravityExtractor::new();
        if let Ok(locations) = antigravity_extractor.find_storage_locations() {
            for location in locations {
                if let Ok(files) = antigravity_extractor.list_session_files(&location) {
                    for file in files {
                        let id = file.metadata.id.clone();
                        if seen_ids.insert(id) {
                            all_sessions.push(SessionInfo {
                                id: file.metadata.id,
                                source: file.metadata.source,
                                title: file.metadata.title,
                                workspace_name: file.metadata.workspace_name,
                                created_at: file.metadata.created_at.map(|d| d.to_rfc3339()),
                                file_size: file.metadata.file_size,
                                path: file.source_path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }

        // 3. Scan Cursor sessions
        let cursor_extractor = CursorExtractor::new();
        if let Ok(locations) = cursor_extractor.find_storage_locations() {
            for location in locations {
                if let Ok(files) = cursor_extractor.list_session_files(&location) {
                    for file in files {
                        let id = file.metadata.id.clone();
                        if seen_ids.insert(id) {
                            all_sessions.push(SessionInfo {
                                id: file.metadata.id,
                                source: file.metadata.source,
                                title: file.metadata.title,
                                workspace_name: file.metadata.workspace_name,
                                created_at: file.metadata.created_at.map(|d| d.to_rfc3339()),
                                file_size: file.metadata.file_size,
                                path: file.source_path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }

        // 4. Scan Cline sessions
        let cline_extractor = ClineExtractor::new();
        if let Ok(locations) = cline_extractor.find_storage_locations() {
            for location in locations {
                if let Ok(files) = cline_extractor.list_session_files(&location) {
                    for file in files {
                        let id = file.metadata.id.clone();
                        if seen_ids.insert(id) {
                            all_sessions.push(SessionInfo {
                                id: file.metadata.id,
                                source: file.metadata.source,
                                title: file.metadata.title,
                                workspace_name: file.metadata.workspace_name,
                                created_at: file.metadata.created_at.map(|d| d.to_rfc3339()),
                                file_size: file.metadata.file_size,
                                path: file.source_path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }

        // 5. Read sessions from vault.db (synced from other machines)
        if let Ok(config) = Config::load_default() {
            let vault_dir = config.vault_path;

            if let Ok(vault_db) = echovault_core::storage::VaultDb::open(&vault_dir) {
                if let Ok(db_sessions) = vault_db.get_all_sessions() {
                    info!(
                        "[scan_sessions] Found {} entries in vault.db",
                        db_sessions.len()
                    );

                    // For each session in vault.db that we don't already have locally,
                    // add it from vault (synced from other machine)
                    for db_session in db_sessions {
                        if seen_ids.insert(db_session.id.clone()) {
                            let session_info = find_vault_session_info(
                                &vault_dir,
                                &db_session.id,
                                db_session.file_size,
                                &db_session.source,
                            );
                            all_sessions.push(session_info);
                        }
                    }
                }
            }
        }

        // Sort by created_at descending (newest first)
        all_sessions.sort_by(|a, b| {
            let a_time = a.created_at.as_deref().unwrap_or("");
            let b_time = b.created_at.as_deref().unwrap_or("");
            b_time.cmp(a_time)
        });

        all_sessions
    })
    .await
    .map_err(|e| e.to_string())?;

    let total = sessions.len();
    Ok(ScanResult { sessions, total })
}

// ============ SYNC COMMANDS ============

/// Tìm thông tin session từ vault files (cho sessions đã sync từ máy khác)
fn find_vault_session_info(
    vault_dir: &std::path::Path,
    session_id: &str,
    file_size: u64,
    source: &str,
) -> SessionInfo {
    use std::fs;

    let sessions_dir = vault_dir.join("sessions");

    // Xử lý ID có chứa `/` (Antigravity artifact format: uuid/filename)
    let file_part = if session_id.contains('/') {
        let parts: Vec<&str> = session_id.splitn(2, '/').collect();
        parts.get(1).copied()
    } else {
        None
    };

    // Tìm file path dựa vào source đã biết từ index
    let source_dir = sessions_dir.join(source);
    let mut found_path = String::new();
    let mut display_title: Option<String> = None;

    if source_dir.exists() {
        // Tạo các patterns để tìm file
        let patterns = if let Some(file_name) = file_part {
            // Antigravity artifact: file name là phần sau `/`
            let clean_name = file_name.replace(".md", "");
            display_title = Some(clean_name.replace('_', " "));
            vec![format!("{}.md", clean_name), file_name.to_string()]
        } else {
            // Normal session
            let extension = if source == "antigravity" {
                "pb"
            } else {
                "json"
            };
            vec![
                format!("{}.{}", session_id, extension),
                session_id.to_string(),
            ]
        };

        for pattern in &patterns {
            let file_path = source_dir.join(pattern);
            if file_path.exists() {
                found_path = file_path.to_string_lossy().to_string();
                break;
            }
        }
    }

    // Nếu không tìm thấy path cụ thể, dùng đường dẫn sessions
    if found_path.is_empty() {
        found_path = sessions_dir.to_string_lossy().to_string();
    }

    // Thử đọc file để lấy thêm thông tin
    let title = if !found_path.is_empty() && std::path::Path::new(&found_path).exists() {
        if let Ok(content) = fs::read_to_string(&found_path) {
            // Parse JSON để lấy title/workspace
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                json.get("title")
                    .or_else(|| json.get("workspace_name"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                display_title
            }
        } else {
            display_title
        }
    } else {
        display_title
    };

    SessionInfo {
        id: session_id.to_string(),
        source: source.to_string(),
        title,
        workspace_name: None,
        created_at: None, // Không có timestamp chính xác từ index
        file_size,
        path: found_path,
    }
}

/// Ingest sessions từ local extractors vào vault
fn ingest_sessions(vault_dir: &std::path::Path) -> Result<bool, String> {
    use echovault_core::extractors::{
        antigravity::AntigravityExtractor, cline::ClineExtractor, cursor::CursorExtractor,
        vscode_copilot::VSCodeCopilotExtractor, Extractor,
    };
    use rayon::prelude::*;

    use parking_lot::Mutex;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Configure thread pool: use num_cpus - 2 (minimum 1)
    let num_threads = std::cmp::max(1, num_cpus::get().saturating_sub(2));
    info!(
        "[ingest_sessions] Using {} threads for parallel processing",
        num_threads
    );

    // Build custom thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .map_err(|e| format!("Failed to build thread pool: {}", e))?;

    info!("[ingest_sessions] Starting scan...");

    let mut sessions = Vec::new();

    // 1. Scan VS Code Copilot sessions
    let vscode_extractor = VSCodeCopilotExtractor::new();
    if let Ok(locations) = vscode_extractor.find_storage_locations() {
        info!(
            "[ingest_sessions] VS Code Copilot: {} locations",
            locations.len()
        );
        for location in &locations {
            if let Ok(files) = vscode_extractor.list_session_files(location) {
                info!(
                    "[ingest_sessions] Location {:?}: {} files",
                    location,
                    files.len()
                );
                sessions.extend(files);
            }
        }
    }

    // 2. Scan Antigravity artifacts
    let antigravity_extractor = AntigravityExtractor::new();
    if let Ok(locations) = antigravity_extractor.find_storage_locations() {
        info!(
            "[ingest_sessions] Antigravity: {} locations",
            locations.len()
        );
        for location in &locations {
            if let Ok(files) = antigravity_extractor.list_session_files(location) {
                info!(
                    "[ingest_sessions] Antigravity {:?}: {} files",
                    location.file_name().unwrap_or_default(),
                    files.len()
                );
                sessions.extend(files);
            }
        }
    }

    // 3. Scan Cursor sessions
    let cursor_extractor = CursorExtractor::new();
    if let Ok(locations) = cursor_extractor.find_storage_locations() {
        info!("[ingest_sessions] Cursor: {} locations", locations.len());
        for location in &locations {
            if let Ok(files) = cursor_extractor.list_session_files(location) {
                info!(
                    "[ingest_sessions] Cursor {:?}: {} files",
                    location.file_name().unwrap_or_default(),
                    files.len()
                );
                sessions.extend(files);
            }
        }
    }

    // 4. Scan Cline sessions
    let cline_extractor = ClineExtractor::new();
    if let Ok(locations) = cline_extractor.find_storage_locations() {
        info!("[ingest_sessions] Cline: {} locations", locations.len());
        for location in &locations {
            if let Ok(files) = cline_extractor.list_session_files(location) {
                info!(
                    "[ingest_sessions] Cline {:?}: {} files",
                    location.file_name().unwrap_or_default(),
                    files.len()
                );
                sessions.extend(files);
            }
        }
    }

    let total_sessions = sessions.len();
    info!(
        "[ingest_sessions] Total sessions to check: {}",
        total_sessions
    );

    // 2. Open VaultDb for deduplication
    let vault_db = echovault_core::storage::VaultDb::open(vault_dir)
        .map_err(|e| format!("Failed to open vault.db: {}", e))?;

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

            // Check against vault.db
            let should_process = match vault_db.get_session_mtime(&session.metadata.id) {
                Ok(Some(cached_mtime)) => mtime > cached_mtime,
                Ok(None) => true,
                Err(_) => true, // Process if we can't check
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
    info!(
        "[ingest_sessions] To process: {}, Already up-to-date: {}",
        to_process, skipped
    );

    if to_process == 0 {
        info!("[ingest_sessions] Nothing to process, complete");
        return Ok(false);
    }

    // 4. Process sessions in parallel
    let processed = AtomicUsize::new(0);
    let errors = Mutex::new(Vec::<String>::new());
    let new_entries = Mutex::new(Vec::<echovault_core::storage::SessionEntry>::new());

    pool.install(|| {
        sessions_to_process
            .par_iter()
            .for_each(|(session, mtime, file_size)| {
                let source_path = &session.metadata.original_path;
                let dest_dir = sessions_dir.join(&session.metadata.source);

                // Create dest dir
                if let Err(e) = fs::create_dir_all(&dest_dir) {
                    errors
                        .lock()
                        .push(format!("Failed to create dir {:?}: {}", dest_dir, e));
                    return;
                }

                let current = processed.fetch_add(1, Ordering::Relaxed) + 1;
                if current.is_multiple_of(10) || current == to_process {
                    info!(
                        "[ingest_sessions] Progress: {}/{} ({}%)",
                        current,
                        to_process,
                        current * 100 / to_process
                    );
                }

                // Prepare destination path
                let extension = source_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("json");

                // Smart filename handling
                let dest_path = if session.metadata.id.ends_with(extension) {
                    dest_dir.join(&session.metadata.id)
                } else {
                    dest_dir.join(format!("{}.{}", session.metadata.id, extension))
                };

                // Ensure parent directory exists (for IDs with slashes)
                if let Some(parent) = dest_path.parent() {
                    if !parent.exists() {
                        let _ = fs::create_dir_all(parent);
                    }
                }

                if let Err(e) = fs::copy(source_path, &dest_path) {
                    errors
                        .lock()
                        .push(format!("Failed to copy {}: {}", session.metadata.id, e));
                    return;
                }

                // Record for vault.db update
                new_entries
                    .lock()
                    .push(echovault_core::storage::SessionEntry {
                        id: session.metadata.id.clone(),
                        source: session.metadata.source.clone(),
                        mtime: *mtime,
                        file_size: *file_size,
                        title: session.metadata.title.clone(),
                        workspace_name: session.metadata.workspace_name.clone(),
                        created_at: session.metadata.created_at.map(|d| d.to_rfc3339()),
                        vault_path: dest_path.to_string_lossy().to_string(),
                        original_path: source_path.to_string_lossy().to_string(),
                    });
            });
    });

    // Check for errors
    let errs = errors.into_inner();
    if !errs.is_empty() {
        info!("[ingest_sessions] {} errors occurred:", errs.len());
        for e in &errs {
            warn!("{}", e);
        }
        // Continue anyway, just log errors
    }

    // 5. Update vault.db
    let entries = new_entries.into_inner();
    if !entries.is_empty() {
        for entry in &entries {
            if let Err(e) = vault_db.upsert_session(entry) {
                warn!("[ingest_sessions] Failed to upsert {}: {}", entry.id, e);
            }
        }
        if let Err(e) = vault_db.log_sync("ingest", Some(&format!("{} sessions", entries.len()))) {
            warn!("[ingest_sessions] Failed to log sync: {}", e);
        }
        info!(
            "[ingest_sessions] vault.db updated with {} entries",
            entries.len()
        );
    }

    info!(
        "[ingest_sessions] Complete: {} processed, {} skipped",
        processed.load(Ordering::Relaxed),
        skipped
    );
    Ok(true)
}

/// Sync vault với cloud (Pull -> Ingest -> Push)
#[tauri::command]
pub async fn sync_vault(state: State<'_, AppState>) -> Result<String, String> {
    use std::sync::atomic::{AtomicBool, Ordering};

    // Local sync lock để prevent concurrent sync từ cùng instance
    static SYNC_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

    // Try to acquire lock
    if SYNC_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        info!("[sync_vault] Another sync is already in progress, skipping...");
        return Ok("Sync already in progress".to_string());
    }

    // Ensure lock is released when function returns
    struct SyncLockGuard;
    impl Drop for SyncLockGuard {
        fn drop(&mut self) {
            SYNC_IN_PROGRESS.store(false, Ordering::SeqCst);
            info!("[sync_vault] Sync lock released");
        }
    }
    let _lock_guard = SyncLockGuard;

    info!("[sync_vault] Starting (lock acquired)...");

    // Check auth status
    {
        let provider = state.provider.lock().map_err(|e| {
            error!("[sync_vault] Failed to lock provider: {}", e);
            e.to_string()
        })?;
        info!(
            "[sync_vault] is_authenticated: {}",
            provider.is_authenticated()
        );
        if !provider.is_authenticated() {
            info!("[sync_vault] Not authenticated, returning error");
            return Err("Not authenticated".to_string());
        }
    }

    info!("[sync_vault] Auth check passed");

    let config = Config::load_default().map_err(|e| {
        error!("[sync_vault] Failed to load config: {}", e);
        e.to_string()
    })?;
    let vault_dir = config.vault_path.clone();
    info!("[sync_vault] vault_dir: {:?}", vault_dir);

    // 1. Pull from Remote (get changes from other machines first)
    info!("[sync_vault] Pulling from remote...");
    let vault_dir_for_pull = vault_dir.clone();
    let provider_for_pull = state.provider.clone();
    let options_for_pull = SyncOptions::default();

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
            info!(
                "[sync_vault] Pull complete: has_changes={}",
                result.has_changes
            );
        }
        Err(e) => {
            // Pull failure is not fatal - might be first sync or network issue
            warn!("[sync_vault] Pull failed (continuing anyway): {}", e);
        }
    }

    // 2. Ingest Sessions (local -> vault)
    info!("[sync_vault] Ingesting sessions...");
    let vault_dir_for_ingest = vault_dir.clone();
    let ingest_result = tokio::task::spawn_blocking(move || ingest_sessions(&vault_dir_for_ingest))
        .await
        .map_err(|e| e.to_string())??;
    info!("[sync_vault] Ingest complete: changes={}", ingest_result);

    // 3. Push to Remote
    info!("[sync_vault] Pushing to remote...");
    let options = SyncOptions::default();
    let vault_dir_clone = vault_dir.clone();
    let provider_clone = state.provider.clone();

    let result = tokio::task::spawn_blocking(move || {
        let provider = provider_clone.lock().map_err(|e| e.to_string())?;
        info!("[sync_vault] Calling provider.push...");
        provider.push(&vault_dir_clone, &options).map_err(|e| {
            error!("[sync_vault] Push failed: {}", e);
            e.to_string()
        })
    })
    .await
    .map_err(|e| {
        error!("[sync_vault] spawn_blocking failed: {}", e);
        e.to_string()
    })??;

    info!(
        "[sync_vault] Push complete: files_pushed={}",
        result.files_pushed
    );
    Ok(format!("Synced {} files", result.files_pushed))
}

// ============ UTILITY COMMANDS ============

/// Mở URL trong browser
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Đọc nội dung file để hiển thị trong text editor
#[tauri::command]
pub async fn read_file_content(path: String) -> Result<String, String> {
    use std::fs;

    let path = std::path::Path::new(&path);

    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    // Giới hạn 50MB
    const MAX_SIZE: u64 = 50 * 1024 * 1024;
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;

    if metadata.len() > MAX_SIZE {
        return Err(format!(
            "File too large: {} bytes (max: {} bytes)",
            metadata.len(),
            MAX_SIZE
        ));
    }

    fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))
}
