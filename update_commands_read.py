import sys

def replace_file_content(path, search_str, replace_str):
    try:
        with open(path, "r") as f:
            content = f.read()

        if search_str not in content:
            print(f"Error: Search string not found in {path}")
            sys.exit(1)

        new_content = content.replace(search_str, replace_str)

        with open(path, "w") as f:
            f.write(new_content)

        print(f"Successfully modified {path}")

    except Exception as e:
        print(f"Error modifying file: {e}")
        sys.exit(1)

search = """/// Đọc nội dung file để hiển thị trong text editor
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
}"""

replace = """/// Đọc nội dung file để hiển thị trong text editor
#[tauri::command]
pub async fn read_file_content(path: String, state: State<'_, AppState>) -> Result<String, String> {
    use std::fs;

    let path_buf = std::path::Path::new(&path);

    if !path_buf.exists() {
        return Err(format!("File not found: {}", path_buf.display()));
    }

    // Security check: validate path
    let canonical_path = fs::canonicalize(path_buf).map_err(|e| format!("Invalid path: {}", e))?;
    let mut allowed = false;

    // 1. Check if in vault directory
    if let Ok(config) = Config::load_default() {
        if let Ok(vault_canon) = fs::canonicalize(&config.vault_path) {
            if canonical_path.starts_with(&vault_canon) {
                allowed = true;
            }
        }

        // 2. Check if in export directory
        if !allowed {
            if let Some(export_path) = config.export_path {
                if let Ok(export_canon) = fs::canonicalize(&export_path) {
                    if canonical_path.starts_with(&export_canon) {
                        allowed = true;
                    }
                }
            }
        }
    }

    // 3. Check against known paths from scan_sessions
    if !allowed {
        if let Ok(known) = state.known_paths.lock() {
            if known.contains(&canonical_path.to_string_lossy().to_string()) {
                allowed = true;
            }
        }
    }

    if !allowed {
        warn!("[read_file_content] Access denied: {:?}", canonical_path);
        return Err(format!("Access denied: {}", path));
    }

    // Giới hạn 50MB
    const MAX_SIZE: u64 = 50 * 1024 * 1024;
    let metadata = fs::metadata(&canonical_path).map_err(|e| e.to_string())?;

    if metadata.len() > MAX_SIZE {
        return Err(format!(
            "File too large: {} bytes (max: {} bytes)",
            metadata.len(),
            MAX_SIZE
        ));
    }

    fs::read_to_string(&canonical_path).map_err(|e| format!("Failed to read file: {}", e))
}"""

replace_file_content("apps/tauri/src/commands.rs", search, replace)
