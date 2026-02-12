//! VS Code Copilot Extractor
//!
//! Extracts chat history from GitHub Copilot in VS Code.
//! ONLY COPY raw JSON files, DO NOT parse/transform content.

use super::{Extractor, ExtractorKind, SessionFile, SessionMetadata};
use crate::utils::wsl;
use anyhow::Result;
use chrono::{TimeZone, Utc};
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::Value;
use std::io::BufRead;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct QuickMetadata {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "creationDate")]
    creation_date: Option<i64>,
    #[serde(rename = "customTitle")]
    custom_title: Option<String>,
    #[serde(default)]
    requests: Option<FirstRequestOnly>,
}

struct FirstRequestOnly {
    text: Option<String>,
}

impl<'de> Deserialize<'de> for FirstRequestOnly {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FirstRequestVisitor;

        impl<'de> serde::de::Visitor<'de> for FirstRequestVisitor {
            type Value = FirstRequestOnly;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a list of requests")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                #[derive(Deserialize)]
                struct RequestStub {
                    message: Option<MessageStub>,
                }
                #[derive(Deserialize)]
                struct MessageStub {
                    text: Option<String>,
                }

                // Read the first element
                let first: Option<RequestStub> = seq.next_element()?;

                let text = first.and_then(|r| r.message).and_then(|m| m.text);

                // Drain the rest of the sequence without allocating
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}

                Ok(FirstRequestOnly { text })
            }
        }

        deserializer.deserialize_seq(FirstRequestVisitor)
    }
}

/// VS Code Copilot Extractor
pub struct VSCodeCopilotExtractor {
    /// Paths that may contain workspaceStorage
    storage_paths: Vec<PathBuf>,
}

/// VS Code workspace storage relative paths (from home dir).
const VSCODE_WORKSPACE_SUBPATHS: &[&str] = &[
    ".config/Code/User/workspaceStorage",
    ".config/Code - Insiders/User/workspaceStorage",
];

impl VSCodeCopilotExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        let mut storage_paths = Vec::new();

        // Prefer reading from HOME env variable (for testing with HOME override)
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            for subpath in VSCODE_WORKSPACE_SUBPATHS {
                storage_paths.push(home_path.join(subpath));
            }
        }

        // Fallback: Get path per platform via dirs crate
        if let Some(config_dir) = dirs::config_dir() {
            // Linux: ~/.config/Code/User/workspaceStorage
            // macOS: ~/Library/Application Support/Code/User/workspaceStorage
            let code_path = config_dir.join("Code/User/workspaceStorage");
            if !storage_paths.contains(&code_path) {
                storage_paths.push(code_path);
            }
            let insiders_path = config_dir.join("Code - Insiders/User/workspaceStorage");
            if !storage_paths.contains(&insiders_path) {
                storage_paths.push(insiders_path);
            }
        }

        // NOTE: On Windows, dirs::config_dir() already returns %APPDATA% (Roaming)
        // which is the correct location for VS Code storage.
        // dirs::data_dir() returns %LOCALAPPDATA% (Local) which is NOT where VS Code stores data.

        // Windows: Scan WSL distributions for VS Code storage (Remote-WSL scenario).
        // When VS Code connects to WSL, CLI tools like Claude Code, Gemini CLI
        // store data inside WSL filesystem, but VS Code workspace storage stays on Windows.
        // However, users may have native VS Code inside WSL too.
        for subpath in VSCODE_WORKSPACE_SUBPATHS {
            for wsl_path in wsl::find_wsl_paths(subpath) {
                if !storage_paths.contains(&wsl_path) {
                    storage_paths.push(wsl_path);
                }
            }
        }

        Self { storage_paths }
    }

    /// Quick metadata extraction from JSON/JSONL file (only read required fields).
    fn extract_quick_metadata(
        &self,
        path: &PathBuf,
        workspace_name: &str,
    ) -> Option<SessionMetadata> {
        let is_jsonl = path.extension().is_some_and(|ext| ext == "jsonl");

        let (session_id, creation_date, custom_title, first_message_text) = if is_jsonl {
            // JSONL format: first line is kind=0 (session header), data in "v" field
            let file = std::fs::File::open(path).ok()?;
            let reader = std::io::BufReader::with_capacity(128 * 1024, file);
            let first_line = reader.lines().next()?.ok()?;
            let wrapper: Value = serde_json::from_str(&first_line).ok()?;
            // Extract the "v" object which contains session metadata
            let v = wrapper.get("v")?;
            (
                v.get("sessionId")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                v.get("creationDate").and_then(|d| d.as_i64()),
                v.get("customTitle")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string()),
                None, // JSONL doesn't store first message in header usually, handled later
            )
        } else {
            // Legacy JSON format: use streaming parser to avoid loading full file
            let file = std::fs::File::open(path).ok()?;
            let reader = std::io::BufReader::with_capacity(128 * 1024, file);
            let metadata: QuickMetadata = serde_json::from_reader(reader).ok()?;
            (
                metadata.session_id,
                metadata.creation_date,
                metadata.custom_title,
                metadata.requests.and_then(|r| r.text),
            )
        };

        // Get session ID from filename or JSON
        let session_id = session_id.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or_default()
        });

        // Get title if available
        let title = custom_title
            .or_else(|| {
                // Fallback: get text from first request (legacy JSON)
                first_message_text.as_ref().map(|s| {
                    let truncated: String = s.chars().take(60).collect();
                    if s.chars().count() > 60 {
                        format!("{}...", truncated)
                    } else {
                        truncated
                    }
                })
            })
            .or_else(|| {
                // Fallback for JSONL: read subsequent lines to find first user message
                if !is_jsonl {
                    return None;
                }
                let file = std::fs::File::open(path).ok()?;
                let reader = std::io::BufReader::with_capacity(128 * 1024, file);
                // Skip first line (header), look for kind=1 with string value (user message)
                for line in reader.lines().skip(1).take(20).flatten() {
                    if let Ok(obj) = serde_json::from_str::<Value>(&line) {
                        if obj.get("kind").and_then(|k| k.as_i64()) == Some(1) {
                            if let Some(text) = obj.get("v").and_then(|v| v.as_str()) {
                                if !text.is_empty() && text.len() > 5 {
                                    let truncated: String = text.chars().take(60).collect();
                                    return if text.chars().count() > 60 {
                                        Some(format!("{}...", truncated))
                                    } else {
                                        Some(truncated)
                                    };
                                }
                            }
                        }
                    }
                }
                None
            });

        // Get timestamp
        let created_at = creation_date.and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        // Get file size
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        Some(SessionMetadata {
            id: session_id,
            source: "vscode-copilot".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(), // Will be set after copy
            original_path: path.clone(),
            file_size,
            workspace_name: Some(workspace_name.to_string()),
            ide_origin: None,
        })
    }
}

impl Default for VSCodeCopilotExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for VSCodeCopilotExtractor {
    fn source_name(&self) -> &'static str {
        "vscode-copilot"
    }

    fn extractor_kind(&self) -> ExtractorKind {
        ExtractorKind::Extension
    }

    fn supported_ides(&self) -> &'static [&'static str] {
        &["VS Code", "VS Code Insiders"]
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let mut workspaces = Vec::new();

        for storage_path in &self.storage_paths {
            if !storage_path.exists() {
                continue;
            }

            // Iterate through all workspace hash directories
            if let Ok(entries) = std::fs::read_dir(storage_path) {
                for entry in entries.flatten() {
                    let chat_sessions_dir = entry.path().join("chatSessions");
                    if chat_sessions_dir.exists() && chat_sessions_dir.is_dir() {
                        // Check if there are any JSON or JSONL files
                        if let Ok(sessions) = std::fs::read_dir(&chat_sessions_dir) {
                            let has_sessions = sessions.flatten().any(|e| {
                                e.path()
                                    .extension()
                                    .is_some_and(|ext| ext == "json" || ext == "jsonl")
                            });
                            if has_sessions {
                                workspaces.push(entry.path());
                            }
                        }
                    }
                }
            }
        }

        Ok(workspaces)
    }

    fn get_workspace_name(&self, location: &Path) -> String {
        let workspace_json = location.join("workspace.json");
        if workspace_json.exists() {
            if let Ok(content) = std::fs::read_to_string(&workspace_json) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    return json
                        .get("folder")
                        .and_then(|v| v.as_str())
                        .map(|s| {
                            // Get last folder name from URI
                            s.rsplit('/').next().unwrap_or(s).to_string()
                        })
                        .unwrap_or_else(|| "Unknown".to_string());
                }
            }
        }
        "Unknown".to_string()
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let chat_sessions_dir = location.join("chatSessions");
        if !chat_sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let workspace_name = self.get_workspace_name(location);

        // Collect all JSON and JSONL paths
        let json_paths: Vec<PathBuf> = std::fs::read_dir(&chat_sessions_dir)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|ext| ext == "json" || ext == "jsonl")
            })
            .collect();

        // Extract metadata in parallel with rayon
        let mut sessions: Vec<SessionFile> = json_paths
            .par_iter()
            .filter_map(|path| {
                self.extract_quick_metadata(path, &workspace_name)
                    .map(|metadata| SessionFile {
                        source_path: path.clone(),
                        metadata,
                    })
            })
            .collect();

        // Sort by creation time (newest first)
        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));

        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        let chat_sessions_dir = location.join("chatSessions");
        if !chat_sessions_dir.exists() {
            return Ok(0);
        }

        // Count JSON and JSONL files, don't parse metadata
        let count = std::fs::read_dir(&chat_sessions_dir)?
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "json" || ext == "jsonl")
            })
            .count();

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    #[ignore]
    fn test_legacy_json_extraction_perf() {
        let dir = tempdir().unwrap();
        let chat_sessions_dir = dir.path().join("chatSessions");
        std::fs::create_dir(&chat_sessions_dir).unwrap();

        // Create a large JSON file (~5MB)
        let file_path = chat_sessions_dir.join("large_session.json");
        let mut file = File::create(&file_path).unwrap();

        write!(file, r#"{{"sessionId":"123","creationDate":1700000000000,"customTitle":"Test Session","requests":["#).unwrap();

        // Write 10000 dummy requests
        for i in 0..10000 {
            if i > 0 {
                write!(file, ",").unwrap();
            }
            // Each message is roughly 500 bytes
            write!(file, r#"{{"message":{{"text":"This is a dummy message number {} repeated many times to increase file size. Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident sunt in culpa qui officia deserunt mollit anim id est laborum."}}}}"#, i).unwrap();
        }

        write!(file, r#"]}}"#).unwrap();

        let extractor = VSCodeCopilotExtractor::new();

        let start = std::time::Instant::now();
        // Since extract_quick_metadata is private, we can't call it directly in integration tests,
        // but here we are in the same module so we can.
        let metadata = extractor.extract_quick_metadata(&file_path, "test_workspace");
        let duration = start.elapsed();

        println!("Extraction time: {:?}", duration);

        assert!(metadata.is_some());
        let meta = metadata.unwrap();
        assert_eq!(meta.id, "123");
        // Title should be extracted correctly
        assert_eq!(meta.title.as_deref(), Some("Test Session"));
    }
}
