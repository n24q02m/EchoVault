//! JetBrains AI Assistant Extractor
//!
//! Extracts AI chat history from JetBrains IDEs (IntelliJ, PyCharm, WebStorm, etc.).
//! Chat history is stored in workspace XML files in two possible locations:
//!
//! ## 1. IDE Config Directory
//! - Windows: `%APPDATA%\JetBrains\<Product><Version>\workspace\*.xml`
//! - Linux: `~/.config/JetBrains/<Product><Version>/workspace/*.xml`
//! - macOS: `~/Library/Application Support/JetBrains/<Product><Version>/workspace/*.xml`
//!
//! ## 2. Per-project `.idea/workspace.xml`
//! - Also stores chat history per-project
//!
//! The XML format varies between IDE versions. Known component names:
//! - `AiAssistantConversation` (newer JetBrains AI)
//! - `ChatSessionStateTemp` (older AI Assistant plugin)
//!
//! Parsing is defensive to handle both patterns.

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

/// JetBrains AI Assistant Extractor
pub struct JetBrainsExtractor {
    /// Top-level JetBrains config directories
    jetbrains_dirs: Vec<PathBuf>,
}

/// Known JetBrains IDE product prefixes
const JETBRAINS_PRODUCTS: &[&str] = &[
    "IntelliJIdea",
    "PhpStorm",
    "WebStorm",
    "PyCharm",
    "PyCharmCE",
    "RubyMine",
    "GoLand",
    "CLion",
    "Rider",
    "DataGrip",
    "RustRover",
    "Fleet",
    "AndroidStudio",
    "DataSpell",
    "Aqua",
];

/// XML component markers that indicate AI chat sessions
const CHAT_COMPONENTS: &[&str] = &[
    "AiAssistantConversation",
    "ChatSessionStateTemp",
    // JetBrains AI Assistant plugin variants
    "AiAssistantHistory",
];

impl JetBrainsExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        let mut jetbrains_dirs = Vec::new();

        #[cfg(target_os = "windows")]
        {
            if let Some(appdata) = dirs::config_dir() {
                let jb = appdata.join("JetBrains");
                if jb.exists() {
                    jetbrains_dirs.push(jb);
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(config) = dirs::config_dir() {
                let jb = config.join("JetBrains");
                if jb.exists() {
                    jetbrains_dirs.push(jb);
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(data) = dirs::data_dir() {
                let jb = data.join("JetBrains");
                if jb.exists() {
                    jetbrains_dirs.push(jb);
                }
            }
        }

        if let Ok(dir) = std::env::var("JETBRAINS_CONFIG") {
            let path = PathBuf::from(dir);
            if !jetbrains_dirs.contains(&path) {
                jetbrains_dirs.push(path);
            }
        }

        Self { jetbrains_dirs }
    }

    /// Find all workspace directories that may contain AI chat XML files.
    fn find_workspace_dirs(&self) -> Vec<PathBuf> {
        let mut workspace_dirs = Vec::new();

        for jb_dir in &self.jetbrains_dirs {
            if let Ok(entries) = std::fs::read_dir(jb_dir) {
                for entry in entries.flatten() {
                    let dir_name = entry.file_name().to_string_lossy().to_string();

                    let is_product = JETBRAINS_PRODUCTS
                        .iter()
                        .any(|product| dir_name.starts_with(product));

                    if is_product && entry.path().is_dir() {
                        let workspace = entry.path().join("workspace");
                        if workspace.exists() && workspace.is_dir() {
                            workspace_dirs.push(workspace);
                        }
                    }
                }
            }
        }

        workspace_dirs
    }

    /// Check if an XML file contains AI chat sessions.
    fn has_chat_sessions(path: &Path) -> bool {
        if let Ok(content) = std::fs::read_to_string(path) {
            CHAT_COMPONENTS
                .iter()
                .any(|component| content.contains(component))
        } else {
            false
        }
    }

    /// Extract metadata from an XML file containing AI chat sessions.
    fn extract_metadata(&self, path: &Path, workspace_dir: &Path) -> Option<SessionMetadata> {
        if !Self::has_chat_sessions(path) {
            return None;
        }

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if file_size < 100 {
            return None;
        }

        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())?;

        // Product name from path: .../JetBrains/IntelliJIdea2024.3/workspace/abc.xml
        let product = workspace_dir
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "JetBrains".to_string());

        // Try to extract first conversation title from content
        let title = std::fs::read_to_string(path)
            .ok()
            .and_then(|content| Self::extract_first_title(&content))
            .or_else(|| Some(format!("AI Chat - {}", product)));

        let created_at = std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0));

        Some(SessionMetadata {
            id: session_id,
            source: "jetbrains".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: path.to_path_buf(),
            file_size,
            workspace_name: Some(product),
            ide_origin: None,
        })
    }

    /// Extract first title from XML content (defensive across formats).
    fn extract_first_title(content: &str) -> Option<String> {
        // Pattern 1: <conversation ... title="..." ...>
        // Pattern 2: <option name="title" value="..." />
        // Pattern 3: title="..." anywhere in a chat/session element

        // Try attribute-style title
        for pattern in &["title=\"", "name=\"title\" value=\""] {
            if let Some(pos) = content.find(pattern) {
                let rest = &content[pos + pattern.len()..];
                if let Some(end) = rest.find('"') {
                    let value = &rest[..end];
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }

        None
    }
}

impl Default for JetBrainsExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for JetBrainsExtractor {
    fn source_name(&self) -> &'static str {
        "jetbrains"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        Ok(self.find_workspace_dirs())
    }

    fn get_workspace_name(&self, location: &Path) -> String {
        location
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "JetBrains".to_string())
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let mut sessions = Vec::new();

        if let Ok(entries) = std::fs::read_dir(location) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "xml") {
                    if let Some(metadata) = self.extract_metadata(&path, location) {
                        sessions.push(SessionFile {
                            source_path: path,
                            metadata,
                        });
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));
        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        let count = std::fs::read_dir(location)?
            .flatten()
            .filter(|e| {
                let path = e.path();
                path.is_file()
                    && path.extension().is_some_and(|ext| ext == "xml")
                    && Self::has_chat_sessions(&path)
            })
            .count();
        Ok(count)
    }
}
