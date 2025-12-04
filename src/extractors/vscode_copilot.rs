//! VS Code Copilot Extractor
//!
//! Trích xuất chat history từ GitHub Copilot trong VS Code.
//! Hỗ trợ nhiều phiên bản format JSON:
//! - Version 1/2: response.result.value là string hoặc response là array
//! - Version 3: response.result.value.node là tree structure (VS Code 1.96+)

use super::{ChatMessage, ChatSession, Extractor};
use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::path::PathBuf;

/// VS Code Copilot Extractor
pub struct VSCodeCopilotExtractor {
    /// Các đường dẫn có thể chứa workspaceStorage
    storage_paths: Vec<PathBuf>,
}

impl VSCodeCopilotExtractor {
    /// Tạo extractor mới với các đường dẫn mặc định theo platform
    pub fn new() -> Self {
        let mut storage_paths = Vec::new();

        // Lấy đường dẫn theo platform
        if let Some(config_dir) = dirs::config_dir() {
            // Linux: ~/.config/Code/User/workspaceStorage
            // macOS: ~/Library/Application Support/Code/User/workspaceStorage
            storage_paths.push(config_dir.join("Code/User/workspaceStorage"));
            storage_paths.push(config_dir.join("Code - Insiders/User/workspaceStorage"));
        }

        #[cfg(target_os = "windows")]
        if let Some(appdata) = dirs::data_dir() {
            // Windows: %APPDATA%\Code\User\workspaceStorage
            storage_paths.push(appdata.join("Code/User/workspaceStorage"));
            storage_paths.push(appdata.join("Code - Insiders/User/workspaceStorage"));
        }

        // WSL: ~/.vscode-server/data/User/workspaceStorage
        if let Some(home) = dirs::home_dir() {
            storage_paths.push(home.join(".vscode-server/data/User/workspaceStorage"));
        }

        // WSL: Truy cập Windows filesystem qua /mnt/c
        // Đọc từ Windows AppData khi chạy trong WSL
        #[cfg(target_os = "linux")]
        {
            // Kiểm tra nếu đang chạy trong WSL
            if std::path::Path::new("/mnt/c/Windows").exists() {
                // Thử tìm Windows username
                if let Ok(entries) = std::fs::read_dir("/mnt/c/Users") {
                    for entry in entries.flatten() {
                        let username = entry.file_name();
                        let username_str = username.to_string_lossy();
                        // Bỏ qua các thư mục hệ thống
                        if username_str == "Public"
                            || username_str == "Default"
                            || username_str == "Default User"
                            || username_str == "All Users"
                        {
                            continue;
                        }
                        let appdata_path = entry.path().join("AppData/Roaming");
                        if appdata_path.exists() {
                            storage_paths
                                .push(appdata_path.join("Code/User/workspaceStorage"));
                            storage_paths
                                .push(appdata_path.join("Code - Insiders/User/workspaceStorage"));
                        }
                    }
                }
            }
        }

        Self { storage_paths }
    }

    /// Tìm tất cả workspace directories có chứa chatSessions
    fn find_workspaces_with_sessions(&self) -> Result<Vec<PathBuf>> {
        let mut workspaces = Vec::new();

        for storage_path in &self.storage_paths {
            if !storage_path.exists() {
                continue;
            }

            // Duyệt qua tất cả workspace hash directories
            if let Ok(entries) = std::fs::read_dir(storage_path) {
                for entry in entries.flatten() {
                    let chat_sessions_dir = entry.path().join("chatSessions");
                    if chat_sessions_dir.exists() && chat_sessions_dir.is_dir() {
                        // Kiểm tra có file JSON nào không
                        if let Ok(sessions) = std::fs::read_dir(&chat_sessions_dir) {
                            let has_json = sessions
                                .flatten()
                                .any(|e| e.path().extension().is_some_and(|ext| ext == "json"));
                            if has_json {
                                workspaces.push(entry.path());
                            }
                        }
                    }
                }
            }
        }

        Ok(workspaces)
    }

    /// Đọc workspace name từ workspace.json
    pub fn get_workspace_name(&self, workspace_dir: &PathBuf) -> String {
        let workspace_json = workspace_dir.join("workspace.json");
        if workspace_json.exists() {
            if let Ok(content) = std::fs::read_to_string(&workspace_json) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    return json
                        .get("folder")
                        .and_then(|v| v.as_str())
                        .map(|s| {
                            // Lấy tên folder cuối cùng từ URI
                            s.rsplit('/').next().unwrap_or(s).to_string()
                        })
                        .unwrap_or_else(|| "Unknown".to_string());
                }
            }
        }
        "Unknown".to_string()
    }

    /// Extract text từ node tree (version 3 format)
    /// Node tree có cấu trúc: { type, children: [{ type, text, children, lineBreakBefore }] }
    fn extract_text_from_node(node: &Value) -> String {
        let mut text_parts = Vec::new();

        // Kiểm tra lineBreakBefore
        let needs_newline = node
            .get("lineBreakBefore")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if needs_newline && !text_parts.is_empty() {
            text_parts.push("\n".to_string());
        }

        // Nếu node có "text" field, lấy nó
        if let Some(text) = node.get("text").and_then(|v| v.as_str()) {
            text_parts.push(text.to_string());
        }

        // Đệ quy vào children
        if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
            for child in children {
                // Kiểm tra lineBreakBefore cho child
                let child_needs_newline = child
                    .get("lineBreakBefore")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if child_needs_newline && !text_parts.is_empty() {
                    // Chỉ thêm newline nếu chưa có ở cuối
                    if !text_parts.last().is_some_and(|s| s.ends_with('\n')) {
                        text_parts.push("\n".to_string());
                    }
                }

                let child_text = Self::extract_text_from_node(child);
                if !child_text.is_empty() {
                    text_parts.push(child_text);
                }
            }
        }

        text_parts.join("")
    }

    /// Parse một session JSON file thành ChatSession
    fn parse_session_file(&self, path: &PathBuf) -> Result<ChatSession> {
        let content = std::fs::read_to_string(path).context("Failed to read session file")?;

        // Parse thành generic Value trước để detect version
        let raw_value: Value = serde_json::from_str(&content)
            .with_context(|| format!("Invalid JSON in {}", path.display()))?;

        // Lấy version (mặc định là 1 nếu không có)
        let version = raw_value
            .get("version")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        // Lấy root-level metadata (version 3)
        let session_id = raw_value
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Fallback: lấy từ filename
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
            });

        let custom_title = raw_value
            .get("customTitle")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let creation_date = raw_value.get("creationDate").and_then(|v| v.as_i64());

        let last_message_date = raw_value.get("lastMessageDate").and_then(|v| v.as_i64());

        // Parse requests
        let requests = raw_value
            .get("requests")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut messages = Vec::new();
        let mut first_timestamp: Option<i64> = creation_date;
        let mut last_timestamp: Option<i64> = last_message_date;

        for request in requests {
            // Trích xuất user message
            if let Some(message) = request.get("message") {
                let user_text = self.extract_user_message(message);
                if !user_text.trim().is_empty() {
                    messages.push(ChatMessage {
                        role: "user".to_string(),
                        content: user_text.trim().to_string(),
                        model: None,
                    });
                }
            }

            // Trích xuất assistant response dựa trên version
            if let Some(response) = request.get("response") {
                let (assistant_text, model_name) = if version >= 3 {
                    self.extract_response_v3(response)
                } else {
                    self.extract_response_v1_v2(response)
                };

                // Cập nhật timestamp từ response (v1/v2)
                if let Some(result) = response.get("result") {
                    if let Some(ts) = result.get("createdAt").and_then(|v| v.as_i64()) {
                        if first_timestamp.is_none() {
                            first_timestamp = Some(ts);
                        }
                        last_timestamp = Some(ts);
                    }
                }

                if !assistant_text.trim().is_empty() {
                    // Lấy model từ request nếu không có trong response
                    let final_model = model_name.or_else(|| {
                        request
                            .get("modelId")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    });

                    messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: assistant_text.trim().to_string(),
                        model: final_model,
                    });
                }
            }
        }

        // Tạo title: ưu tiên customTitle, fallback về message đầu tiên
        let title = custom_title.or_else(|| {
            messages.first().map(|m| {
                let content = &m.content;
                // Truncate at char boundary, not byte boundary
                let truncated: String = content.chars().take(60).collect();
                if content.chars().count() > 60 {
                    format!("{}...", truncated)
                } else {
                    truncated
                }
            })
        });

        // Parse timestamps
        let created_at = first_timestamp.and_then(|ts| Utc.timestamp_millis_opt(ts).single());
        let updated_at = last_timestamp.and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        Ok(ChatSession {
            id: session_id,
            title,
            source: "vscode-copilot".to_string(),
            created_at,
            updated_at,
            messages,
        })
    }

    /// Extract user message từ message object
    fn extract_user_message(&self, message: &Value) -> String {
        // Thử lấy text trực tiếp (version 3)
        if let Some(text) = message.get("text").and_then(|v| v.as_str()) {
            return text.to_string();
        }

        // Fallback: lấy từ parts
        if let Some(parts) = message.get("parts").and_then(|v| v.as_array()) {
            let text_parts: Vec<String> = parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect();
            return text_parts.join("");
        }

        String::new()
    }

    /// Extract response từ version 1/2 format
    /// Cấu trúc: response.result.value (string) hoặc response (array)
    fn extract_response_v1_v2(&self, response: &Value) -> (String, Option<String>) {
        // Thử format cũ: response.result.value là string
        if let Some(result) = response.get("result") {
            if let Some(value) = result.get("value").and_then(|v| v.as_str()) {
                let model = result
                    .get("metadata")
                    .and_then(|m| m.get("modelId"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                return (value.to_string(), model);
            }
        }

        // Thử format v2: response là array
        if let Some(arr) = response.as_array() {
            let text_parts: Vec<String> = arr
                .iter()
                .filter_map(|item| {
                    // Chỉ lấy items không có "kind" field (text thuần)
                    // hoặc items có "kind" là null
                    let kind = item.get("kind");
                    if kind.is_none() || kind.is_some_and(|k| k.is_null()) {
                        item.get("value")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            return (text_parts.join(""), None);
        }

        (String::new(), None)
    }

    /// Extract response từ version 3 format
    /// Cấu trúc: response là array với các items có/không có "kind"
    /// - Items không có "kind": text thuần (nội dung response thật)
    /// - Items có "kind": metadata (thinking, toolInvocation, etc.) - bỏ qua
    fn extract_response_v3(&self, response: &Value) -> (String, Option<String>) {
        // Version 3 response là array of parts
        if let Some(arr) = response.as_array() {
            let text_parts: Vec<String> = arr
                .iter()
                .filter_map(|item| {
                    // Chỉ lấy items không có "kind" field (text thuần)
                    let kind = item.get("kind");
                    if kind.is_none() || kind.is_some_and(|k| k.is_null()) {
                        item.get("value")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();

            if !text_parts.is_empty() {
                return (text_parts.join(""), None);
            }
        }

        // Fallback: thử format với result.value
        if let Some(result) = response.get("result") {
            // Lấy model info
            let model = result
                .get("metadata")
                .and_then(|m| m.get("modelId"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Kiểm tra value
            if let Some(value) = result.get("value") {
                // Nếu value là string (tương thích ngược)
                if let Some(text) = value.as_str() {
                    return (text.to_string(), model);
                }

                // Nếu value là object với node tree
                if let Some(node) = value.get("node") {
                    let text = Self::extract_text_from_node(node);
                    return (text, model);
                }
            }
        }

        // Final fallback: thử v1/v2 format
        self.extract_response_v1_v2(response)
    }
}

impl Default for VSCodeCopilotExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for VSCodeCopilotExtractor {
    fn find_databases(&self) -> Result<Vec<PathBuf>> {
        // Trả về danh sách các workspace directories có chatSessions
        self.find_workspaces_with_sessions()
    }

    fn count_sessions(&self, workspace_path: &PathBuf) -> Result<usize> {
        let chat_sessions_dir = workspace_path.join("chatSessions");
        if !chat_sessions_dir.exists() {
            return Ok(0);
        }

        let count = std::fs::read_dir(&chat_sessions_dir)?
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .count();

        Ok(count)
    }

    fn extract_sessions(&self, workspace_path: &PathBuf) -> Result<Vec<ChatSession>> {
        let chat_sessions_dir = workspace_path.join("chatSessions");
        if !chat_sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();

        for entry in std::fs::read_dir(&chat_sessions_dir)?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                match self.parse_session_file(&path) {
                    Ok(session) => {
                        // Chỉ thêm sessions có messages
                        if !session.messages.is_empty() {
                            sessions.push(session);
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
                    }
                }
            }
        }

        // Sắp xếp theo thời gian tạo (mới nhất trước)
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(sessions)
    }
}
