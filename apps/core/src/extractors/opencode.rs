//! OpenCode Terminal AI Extractor
//!
//! Extracts chat session history from OpenCode (github.com/opencode-ai/opencode).
//! OpenCode stores sessions in a SQLite database per project:
//! - {project}/.opencode/opencode.db
//!
//! Database schema (key tables):
//! - sessions: id, title, model, created_at, updated_at
//! - messages: id, session_id, role, content, created_at, tool_call_id, parts (JSON)
//!
//! The extractor copies the entire opencode.db file to the vault.
//! Scanning is done in known project directories.

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

/// OpenCode Extractor
pub struct OpenCodeExtractor {
    /// Directories to scan for .opencode/ subdirectories.
    scan_dirs: Vec<PathBuf>,
}

/// OpenCode database filename
const OPENCODE_DB: &str = "opencode.db";
/// OpenCode directory name
const OPENCODE_DIR: &str = ".opencode";

impl OpenCodeExtractor {
    /// Create new extractor.
    pub fn new() -> Self {
        let mut scan_dirs = Vec::new();

        // Check OPENCODE_HOME env var
        if let Ok(home) = std::env::var("OPENCODE_HOME") {
            scan_dirs.push(PathBuf::from(home));
        }

        // Common project directories
        if let Some(home) = dirs::home_dir() {
            let project_dirs = ["projects", "repos", "dev", "code", "workspace", "src"];
            for dir in &project_dirs {
                let path = home.join(dir);
                if path.exists() && path.is_dir() {
                    scan_dirs.push(path);
                }
            }
            // Home directory itself
            scan_dirs.push(home);
        }

        // Current working directory
        if let Ok(cwd) = std::env::current_dir() {
            if !scan_dirs.contains(&cwd) {
                scan_dirs.push(cwd);
            }
        }

        Self { scan_dirs }
    }

    /// Recursively find directories containing .opencode/opencode.db (max depth 2)
    fn find_opencode_dirs(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();

        for scan_dir in &self.scan_dirs {
            Self::check_dir(scan_dir, 0, 2, &mut found);
        }

        found.sort();
        found.dedup();
        found
    }

    fn check_dir(dir: &Path, depth: usize, max_depth: usize, found: &mut Vec<PathBuf>) {
        let db_path = dir.join(OPENCODE_DIR).join(OPENCODE_DB);
        if db_path.exists() {
            found.push(dir.to_path_buf());
        }

        if depth >= max_depth {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Skip hidden directories (except .opencode itself)
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') && name != OPENCODE_DIR {
                            continue;
                        }
                        // Skip common non-project directories
                        if matches!(
                            name,
                            "node_modules" | "target" | ".git" | "vendor" | "__pycache__"
                        ) {
                            continue;
                        }
                    }
                    Self::check_dir(&path, depth + 1, max_depth, found);
                }
            }
        }
    }

    /// Extract metadata from an OpenCode database.
    fn extract_metadata(&self, project_dir: &Path) -> Vec<SessionMetadata> {
        let db_path = project_dir.join(OPENCODE_DIR).join(OPENCODE_DB);
        if !db_path.exists() {
            return Vec::new();
        }

        let file_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
        if file_size < 100 {
            return Vec::new();
        }

        let project_name = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        // Create a single session metadata for the entire database
        // (the parser will split into individual sessions)
        let session_id = format!(
            "opencode-{}",
            project_dir
                .to_string_lossy()
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect::<String>()
        );

        // Get modified time as creation time
        let created_at = std::fs::metadata(&db_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0));

        vec![SessionMetadata {
            id: session_id,
            source: "opencode".to_string(),
            title: Some(format!("OpenCode - {}", project_name)),
            created_at,
            vault_path: PathBuf::new(),
            original_path: db_path,
            file_size,
            workspace_name: Some(project_name),
            ide_origin: None,
        }]
    }
}

impl Default for OpenCodeExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for OpenCodeExtractor {
    fn source_name(&self) -> &'static str {
        "opencode"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        Ok(self.find_opencode_dirs())
    }

    fn get_workspace_name(&self, location: &Path) -> String {
        location
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "OpenCode".to_string())
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let metadata_list = self.extract_metadata(location);
        let sessions: Vec<SessionFile> = metadata_list
            .into_iter()
            .map(|metadata| SessionFile {
                source_path: metadata.original_path.clone(),
                metadata,
            })
            .collect();
        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        let db_path = location.join(OPENCODE_DIR).join(OPENCODE_DB);
        if db_path.exists() {
            Ok(1)
        } else {
            Ok(0)
        }
    }

    /// Custom copy: copy the opencode.db to vault under a project-specific name.
    fn copy_to_vault(&self, session: &SessionFile, vault_dir: &Path) -> Result<Option<PathBuf>> {
        let source_dir = vault_dir.join(self.source_name());
        std::fs::create_dir_all(&source_dir)?;

        // Use project name as filename to avoid collisions
        let project_name = session
            .metadata
            .workspace_name
            .as_deref()
            .unwrap_or("unknown");
        let dest_path = source_dir.join(format!("{}.db", project_name));

        let should_copy = if dest_path.exists() {
            let src_meta = session.source_path.metadata()?;
            let dest_meta = dest_path.metadata()?;
            src_meta.modified()? > dest_meta.modified()? || src_meta.len() != dest_meta.len()
        } else {
            true
        };

        if should_copy {
            std::fs::copy(&session.source_path, &dest_path)?;
            Ok(Some(dest_path))
        } else {
            Ok(None)
        }
    }
}
