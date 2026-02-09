use serde::{Deserialize, Serialize};

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

/// Tìm thông tin session từ vault files (cho sessions đã sync từ máy khác)
pub fn find_vault_session_info(
    vault_dir: &std::path::Path,
    session_id: &str,
    file_size: u64,
    source: &str,
    db_created_at: Option<&str>,
    db_title: Option<&str>,
    db_workspace_name: Option<&str>,
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
            // Normal session - try both .json and .jsonl extensions
            let extension = if source == "antigravity" {
                "pb"
            } else {
                "json" // Will try jsonl as fallback below
            };
            vec![
                format!("{}.{}", session_id, extension),
                format!("{}.jsonl", session_id), // JSONL fallback for vscode-copilot/cursor
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

    // Ưu tiên title từ db, nếu không có thì thử đọc từ file
    let title = db_title.map(|s| s.to_string()).or_else(|| {
        if !found_path.is_empty() && std::path::Path::new(&found_path).exists() {
            if let Ok(content) = fs::read_to_string(&found_path) {
                // Parse JSON để lấy title/workspace
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    json.get("title")
                        .or_else(|| json.get("workspace_name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    display_title.clone()
                }
            } else {
                display_title.clone()
            }
        } else {
            display_title.clone()
        }
    });

    SessionInfo {
        id: session_id.to_string(),
        source: source.to_string(),
        title,
        workspace_name: db_workspace_name.map(|s| s.to_string()),
        created_at: db_created_at.map(|s| s.to_string()),
        file_size,
        path: found_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_find_existing_session_file() {
        let dir = tempdir().unwrap();
        let vault_path = dir.path();
        let sessions_dir = vault_path.join("sessions");
        let source_dir = sessions_dir.join("test_source");
        fs::create_dir_all(&source_dir).unwrap();

        let session_id = "test_session";
        let file_path = source_dir.join(format!("{}.json", session_id));
        fs::write(
            &file_path,
            r#"{"title": "Test Title", "workspace_name": "Test Workspace"}"#,
        )
        .unwrap();

        let info = find_vault_session_info(
            vault_path,
            session_id,
            100,
            "test_source",
            Some("2023-01-01T00:00:00Z"),
            Some("DB Title"),
            Some("DB Workspace"),
        );

        assert_eq!(info.id, session_id);
        assert_eq!(info.source, "test_source");
        assert_eq!(info.path, file_path.to_string_lossy().to_string());
        // DB title should take precedence
        assert_eq!(info.title, Some("DB Title".to_string()));
    }

    #[test]
    fn test_find_session_with_slash_id() {
        let dir = tempdir().unwrap();
        let vault_path = dir.path();
        let sessions_dir = vault_path.join("sessions");
        let source_dir = sessions_dir.join("antigravity");
        fs::create_dir_all(&source_dir).unwrap();

        let session_id = "uuid/filename.md";
        let file_path = source_dir.join("filename.md");
        fs::write(&file_path, "content").unwrap();

        let info =
            find_vault_session_info(vault_path, session_id, 200, "antigravity", None, None, None);

        assert_eq!(info.path, file_path.to_string_lossy().to_string());
        assert_eq!(info.title, Some("filename".to_string()));
    }

    #[test]
    fn test_fallback_to_jsonl() {
        let dir = tempdir().unwrap();
        let vault_path = dir.path();
        let sessions_dir = vault_path.join("sessions");
        let source_dir = sessions_dir.join("vscode");
        fs::create_dir_all(&source_dir).unwrap();

        let session_id = "session_123";
        let file_path = source_dir.join(format!("{}.jsonl", session_id));
        fs::write(&file_path, r#"{"v": {"customTitle": "JSONL Title"}}"#).unwrap();

        let info = find_vault_session_info(vault_path, session_id, 300, "vscode", None, None, None);

        assert_eq!(info.path, file_path.to_string_lossy().to_string());
    }

    #[test]
    fn test_extract_title_from_file() {
        let dir = tempdir().unwrap();
        let vault_path = dir.path();
        let sessions_dir = vault_path.join("sessions");
        let source_dir = sessions_dir.join("cursor");
        fs::create_dir_all(&source_dir).unwrap();

        let session_id = "session_456";
        let file_path = source_dir.join(format!("{}.json", session_id));
        fs::write(&file_path, r#"{"title": "File Title"}"#).unwrap();

        let info = find_vault_session_info(vault_path, session_id, 400, "cursor", None, None, None);

        assert_eq!(info.title, Some("File Title".to_string()));
    }

    #[test]
    fn test_missing_file() {
        let dir = tempdir().unwrap();
        let vault_path = dir.path();
        let sessions_dir = vault_path.join("sessions");
        let source_dir = sessions_dir.join("unknown");
        fs::create_dir_all(&source_dir).unwrap();

        let session_id = "missing_session";

        let info = find_vault_session_info(
            vault_path,
            session_id,
            500,
            "unknown",
            None,
            Some("Preserved Title"),
            None,
        );

        assert_eq!(info.path, sessions_dir.to_string_lossy().to_string());
        assert_eq!(info.title, Some("Preserved Title".to_string()));
    }
}
