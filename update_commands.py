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

search1 = """/// Scan tất cả sessions có sẵn (local + synced từ vault)
#[tauri::command]
pub async fn scan_sessions() -> Result<ScanResult, String> {"""

replace1 = """/// Scan tất cả sessions có sẵn (local + synced từ vault)
#[tauri::command]
pub async fn scan_sessions(state: State<'_, AppState>) -> Result<ScanResult, String> {"""

search2 = """.await
    .map_err(|e| e.to_string())?;

    let total = sessions.len();
    Ok(ScanResult { sessions, total })
}"""

replace2 = """.await
    .map_err(|e| e.to_string())?;

    // Update known paths for security
    if let Ok(mut known) = state.known_paths.lock() {
        known.clear();
        for session in &sessions {
            if let Ok(canon) = std::fs::canonicalize(&session.path) {
                known.insert(canon.to_string_lossy().to_string());
            }
        }
        info!("[scan_sessions] Updated known_paths with {} entries", known.len());
    }

    let total = sessions.len();
    Ok(ScanResult { sessions, total })
}"""

replace_file_content("apps/tauri/src/commands.rs", search1, replace1)
replace_file_content("apps/tauri/src/commands.rs", search2, replace2)
