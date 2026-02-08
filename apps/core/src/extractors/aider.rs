//! Aider AI Coding Assistant Extractor
//!
//! Extracts conversation history from Aider.
//! Aider stores history as project-local files:
//! - .aider.chat.history.md (chat history in Markdown)
//! - .aider.llm.history (raw LLM conversation log)
//! - .aider.input.history (input history)
//!
//! Paths can be configured via env vars:
//! - AIDER_CHAT_HISTORY_FILE
//! - AIDER_LLM_HISTORY_FILE
//! - AIDER_INPUT_HISTORY_FILE

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

/// Aider Extractor
pub struct AiderExtractor {
    /// Directories to scan for Aider files.
    /// We scan known project directories and any paths from env vars.
    scan_dirs: Vec<PathBuf>,
}

/// Known Aider file names
const AIDER_CHAT_HISTORY: &str = ".aider.chat.history.md";
const AIDER_LLM_HISTORY: &str = ".aider.llm.history";

impl AiderExtractor {
    /// Create new extractor.
    /// Scans the current directory and any env var overrides.
    pub fn new() -> Self {
        let mut scan_dirs = Vec::new();

        // Check env var for custom path
        if let Ok(path) = std::env::var("AIDER_CHAT_HISTORY_FILE") {
            if let Some(parent) = PathBuf::from(path).parent() {
                scan_dirs.push(parent.to_path_buf());
            }
        }

        // Check common project directories
        if let Some(home) = dirs::home_dir() {
            // Scan home directory itself
            scan_dirs.push(home.clone());

            // Common project directories
            let project_dirs = ["projects", "repos", "dev", "code", "workspace", "src"];
            for dir in &project_dirs {
                let path = home.join(dir);
                if path.exists() && path.is_dir() {
                    scan_dirs.push(path);
                }
            }
        }

        // Current working directory
        if let Ok(cwd) = std::env::current_dir() {
            if !scan_dirs.contains(&cwd) {
                scan_dirs.push(cwd);
            }
        }

        Self { scan_dirs }
    }

    /// Recursively find directories containing .aider.chat.history.md (max depth 2)
    fn find_aider_dirs(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();

        for scan_dir in &self.scan_dirs {
            // Check the scan dir itself
            if scan_dir.join(AIDER_CHAT_HISTORY).exists() {
                found.push(scan_dir.clone());
            }

            // Check subdirectories (depth 1)
            if let Ok(entries) = std::fs::read_dir(scan_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if path.join(AIDER_CHAT_HISTORY).exists() {
                            found.push(path.clone());
                        }

                        // Depth 2
                        if let Ok(sub_entries) = std::fs::read_dir(&path) {
                            for sub_entry in sub_entries.flatten() {
                                let sub_path = sub_entry.path();
                                if sub_path.is_dir() && sub_path.join(AIDER_CHAT_HISTORY).exists() {
                                    found.push(sub_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        found.sort();
        found.dedup();
        found
    }

    /// Extract metadata from an Aider history file.
    fn extract_metadata(&self, dir: &Path) -> Option<SessionMetadata> {
        let chat_history = dir.join(AIDER_CHAT_HISTORY);
        if !chat_history.exists() {
            return None;
        }

        let file_size = std::fs::metadata(&chat_history)
            .map(|m| m.len())
            .unwrap_or(0);

        // Skip empty files
        if file_size < 10 {
            return None;
        }

        // Use directory name as project/workspace
        let project_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        // Session ID from dir path (hash it for uniqueness)
        let session_id = format!(
            "aider-{}",
            dir.to_string_lossy()
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect::<String>()
        );

        // Get title from first meaningful line of chat history
        let title = std::fs::read_to_string(&chat_history)
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|line| {
                        let trimmed = line.trim();
                        !trimmed.is_empty()
                            && !trimmed.starts_with('#')
                            && !trimmed.starts_with("---")
                            && trimmed.len() > 5
                    })
                    .map(|line| {
                        let truncated: String = line.chars().take(60).collect();
                        if line.chars().count() > 60 {
                            format!("{}...", truncated)
                        } else {
                            truncated
                        }
                    })
            })
            .or_else(|| Some(format!("Aider - {}", project_name)));

        // Timestamp from file modified time
        let created_at = std::fs::metadata(&chat_history)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0));

        // Total size includes both chat and llm history
        let total_size = file_size
            + std::fs::metadata(dir.join(AIDER_LLM_HISTORY))
                .map(|m| m.len())
                .unwrap_or(0);

        Some(SessionMetadata {
            id: session_id,
            source: "aider".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: chat_history,
            file_size: total_size,
            workspace_name: Some(project_name),
            ide_origin: None,
        })
    }
}

impl Default for AiderExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for AiderExtractor {
    fn source_name(&self) -> &'static str {
        "aider"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        Ok(self.find_aider_dirs())
    }

    fn get_workspace_name(&self, location: &Path) -> String {
        location
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Aider".to_string())
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        if let Some(metadata) = self.extract_metadata(location) {
            Ok(vec![SessionFile {
                source_path: location.join(AIDER_CHAT_HISTORY),
                metadata,
            }])
        } else {
            Ok(Vec::new())
        }
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        if location.join(AIDER_CHAT_HISTORY).exists() {
            Ok(1)
        } else {
            Ok(0)
        }
    }

    /// Custom copy: copy both chat history and LLM history files.
    fn copy_to_vault(&self, session: &SessionFile, vault_dir: &Path) -> Result<Option<PathBuf>> {
        let source_dir = vault_dir.join(self.source_name());
        std::fs::create_dir_all(&source_dir)?;

        // Use a sanitized directory name for the project
        let project_name = session
            .metadata
            .workspace_name
            .as_deref()
            .unwrap_or("unknown");
        let dest_subdir = source_dir.join(project_name);
        std::fs::create_dir_all(&dest_subdir)?;

        let src_dir = session
            .source_path
            .parent()
            .unwrap_or(session.source_path.as_path());

        let mut copied = false;

        // Copy chat history
        let chat_src = src_dir.join(AIDER_CHAT_HISTORY);
        let chat_dest = dest_subdir.join(AIDER_CHAT_HISTORY);
        if chat_src.exists() {
            let should_copy = if chat_dest.exists() {
                let src_meta = chat_src.metadata()?;
                let dest_meta = chat_dest.metadata()?;
                src_meta.modified()? > dest_meta.modified()? || src_meta.len() != dest_meta.len()
            } else {
                true
            };
            if should_copy {
                std::fs::copy(&chat_src, &chat_dest)?;
                copied = true;
            }
        }

        // Copy LLM history
        let llm_src = src_dir.join(AIDER_LLM_HISTORY);
        let llm_dest = dest_subdir.join(AIDER_LLM_HISTORY);
        if llm_src.exists() {
            let should_copy = if llm_dest.exists() {
                let src_meta = llm_src.metadata()?;
                let dest_meta = llm_dest.metadata()?;
                src_meta.modified()? > dest_meta.modified()? || src_meta.len() != dest_meta.len()
            } else {
                true
            };
            if should_copy {
                std::fs::copy(&llm_src, &llm_dest)?;
                copied = true;
            }
        }

        if copied {
            Ok(Some(chat_dest))
        } else {
            Ok(None)
        }
    }
}
