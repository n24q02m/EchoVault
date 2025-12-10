//! Antigravity Extractor
//!
//! Trích xuất chat history và artifacts từ Google Antigravity.
//! CHỈ COPY raw files, KHÔNG parse/transform nội dung.
//!
//! Storage locations:
//! - Chat history: ~/.gemini/antigravity/conversations/{uuid}.pb
//! - Artifacts: ~/.gemini/antigravity/brain/{uuid}/*.md

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Antigravity Extractor
pub struct AntigravityExtractor {
    /// Đường dẫn có thể chứa Antigravity data
    storage_paths: Vec<PathBuf>,
}

/// Metadata JSON của artifact
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactMetadata {
    artifact_type: Option<String>,
    summary: Option<String>,
    updated_at: Option<String>,
}

impl AntigravityExtractor {
    /// Tạo extractor mới với các đường dẫn mặc định theo platform
    pub fn new() -> Self {
        let mut storage_paths = Vec::new();

        // Ưu tiên đọc từ HOME env variable (để hỗ trợ testing với HOME override)
        if let Ok(home) = std::env::var("HOME") {
            let home_path = Path::new(&home);
            storage_paths.push(home_path.join(".gemini/antigravity"));
        }

        // Fallback: ~/.gemini/antigravity/ qua dirs crate
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".gemini/antigravity");
            if !storage_paths.contains(&path) {
                storage_paths.push(path);
            }
        }

        // Windows: %USERPROFILE%\.gemini\antigravity\
        #[cfg(target_os = "windows")]
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".gemini").join("antigravity");
            if !storage_paths.contains(&path) {
                storage_paths.push(path);
            }
        }

        // Windows: Thêm hỗ trợ đọc từ WSL (\\wsl$\<distro>\home\<user>\.gemini\antigravity\)
        #[cfg(target_os = "windows")]
        {
            // Lấy username từ Windows để tạo đường dẫn WSL
            // Trong WSL, username thường giống Windows hoặc cần scan home directories
            if let Ok(wsl_path) = std::fs::read_dir(r"\\wsl$") {
                for entry in wsl_path.flatten() {
                    let distro_path = entry.path();
                    if distro_path.is_dir() {
                        // Scan tất cả home directories trong WSL distro
                        let wsl_home = distro_path.join("home");
                        if wsl_home.exists() && wsl_home.is_dir() {
                            if let Ok(home_entries) = std::fs::read_dir(&wsl_home) {
                                for home_entry in home_entries.flatten() {
                                    let user_home = home_entry.path();
                                    if user_home.is_dir() {
                                        let wsl_antigravity =
                                            user_home.join(".gemini").join("antigravity");
                                        if wsl_antigravity.exists()
                                            && !storage_paths.contains(&wsl_antigravity)
                                        {
                                            storage_paths.push(wsl_antigravity);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Self { storage_paths }
    }

    /// Tìm thư mục conversations
    fn find_conversations_dir(&self) -> Option<PathBuf> {
        for base_path in &self.storage_paths {
            let conversations_dir = base_path.join("conversations");
            if conversations_dir.exists() && conversations_dir.is_dir() {
                return Some(conversations_dir);
            }
        }
        None
    }

    /// Tìm thư mục brain (artifacts)
    fn find_brain_dir(&self) -> Option<PathBuf> {
        for base_path in &self.storage_paths {
            let brain_dir = base_path.join("brain");
            if brain_dir.exists() && brain_dir.is_dir() {
                return Some(brain_dir);
            }
        }
        None
    }

    /// Extract metadata từ conversation (.pb file)
    fn extract_conversation_metadata(&self, path: &PathBuf) -> Option<SessionMetadata> {
        // Lấy UUID từ filename (ví dụ: 9fc44156-3c5c-45fa-b245-514c9a86e09d.pb)
        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())?;

        // Lấy file size và modified time
        let metadata = std::fs::metadata(path).ok()?;
        let file_size = metadata.len();

        // Lấy modified time làm created_at (gần đúng)
        let created_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0));

        Some(SessionMetadata {
            id: session_id,
            source: "antigravity".to_string(),
            title: Some("Chat Conversation".to_string()), // Protobuf không dễ parse title
            created_at,
            vault_path: PathBuf::new(),
            original_path: path.clone(),
            file_size,
            workspace_name: None, // Antigravity không gắn với workspace cụ thể
        })
    }

    /// Extract metadata từ artifact folder
    fn extract_artifact_metadata(&self, artifact_dir: &Path) -> Vec<SessionMetadata> {
        let mut sessions = Vec::new();

        // Lấy UUID từ folder name
        let session_id = match artifact_dir
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
        {
            Some(id) => id,
            None => return sessions,
        };

        // Tìm các artifact files (.md) trong folder
        let entries = match std::fs::read_dir(artifact_dir) {
            Ok(e) => e,
            Err(_) => return sessions,
        };

        for entry in entries.flatten() {
            let path = entry.path();

            // Chỉ xử lý .md files (không phải .metadata.json hay .resolved)
            if !path.is_file() {
                continue;
            }

            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(f) => f.to_string(),
                None => continue,
            };

            // Bỏ qua metadata và resolved files
            if filename.contains(".metadata.json")
                || filename.contains(".resolved")
                || filename.ends_with(".png")
                || filename.ends_with(".jpg")
                || filename.ends_with(".webp")
            {
                continue;
            }

            // Parse markdown artifacts
            if filename.ends_with(".md") {
                let artifact_id = format!("{}_{}", session_id, filename);

                // Thử đọc metadata từ JSON file
                let metadata_path = artifact_dir.join(format!("{}.metadata.json", filename));
                let (title, created_at) = if metadata_path.exists() {
                    match std::fs::read_to_string(&metadata_path) {
                        Ok(content) => match serde_json::from_str::<ArtifactMetadata>(&content) {
                            Ok(meta) => {
                                let title = meta.summary.or(meta.artifact_type);
                                let created_at = meta.updated_at.and_then(|s| {
                                    DateTime::parse_from_rfc3339(&s)
                                        .ok()
                                        .map(|d| d.with_timezone(&Utc))
                                });
                                (title, created_at)
                            }
                            Err(_) => (None, None),
                        },
                        Err(_) => (None, None),
                    }
                } else {
                    (None, None)
                };

                let file_metadata = std::fs::metadata(&path).ok();
                let file_size = file_metadata.as_ref().map(|m| m.len()).unwrap_or(0);

                sessions.push(SessionMetadata {
                    id: artifact_id,
                    source: "antigravity-artifact".to_string(),
                    title: title.or(Some(filename.replace(".md", ""))),
                    created_at,
                    vault_path: PathBuf::new(),
                    original_path: path.clone(),
                    file_size,
                    workspace_name: None,
                });
            }
        }

        sessions
    }
}

impl Default for AntigravityExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for AntigravityExtractor {
    fn source_name(&self) -> &'static str {
        "antigravity"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let mut locations = Vec::new();

        // Thêm conversations directory nếu tồn tại và có files
        if let Some(conversations_dir) = self.find_conversations_dir() {
            // Kiểm tra có .pb files không
            if let Ok(entries) = std::fs::read_dir(&conversations_dir) {
                let has_pb = entries
                    .flatten()
                    .any(|e| e.path().extension().is_some_and(|ext| ext == "pb"));
                if has_pb {
                    locations.push(conversations_dir);
                }
            }
        }

        // Thêm brain directory nếu tồn tại và có subdirectories
        if let Some(brain_dir) = self.find_brain_dir() {
            if let Ok(entries) = std::fs::read_dir(&brain_dir) {
                let has_subdirs = entries.flatten().any(|e| e.path().is_dir());
                if has_subdirs {
                    locations.push(brain_dir);
                }
            }
        }

        Ok(locations)
    }

    fn get_workspace_name(&self, _location: &Path) -> String {
        // Antigravity không gắn với workspace cụ thể
        "Global".to_string()
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let mut sessions = Vec::new();

        // Kiểm tra loại location
        let location_name = location.file_name().and_then(|s| s.to_str()).unwrap_or("");

        if location_name == "conversations" {
            // Xử lý conversations (.pb files)
            let pb_files: Vec<PathBuf> = std::fs::read_dir(location)?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "pb"))
                .collect();

            let conversation_sessions: Vec<SessionFile> = pb_files
                .par_iter()
                .filter_map(|path| {
                    self.extract_conversation_metadata(path)
                        .map(|metadata| SessionFile {
                            source_path: path.clone(),
                            metadata,
                        })
                })
                .collect();

            sessions.extend(conversation_sessions);
        } else if location_name == "brain" {
            // Xử lý brain (artifact directories)
            let artifact_dirs: Vec<PathBuf> = std::fs::read_dir(location)?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();

            for artifact_dir in artifact_dirs {
                let artifact_sessions: Vec<SessionFile> = self
                    .extract_artifact_metadata(&artifact_dir)
                    .into_iter()
                    .map(|metadata| SessionFile {
                        source_path: metadata.original_path.clone(),
                        metadata,
                    })
                    .collect();
                sessions.extend(artifact_sessions);
            }
        }

        // Sắp xếp theo thời gian tạo (mới nhất trước)
        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));

        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        let location_name = location.file_name().and_then(|s| s.to_str()).unwrap_or("");

        if location_name == "conversations" {
            // Đếm .pb files
            let count = std::fs::read_dir(location)?
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "pb"))
                .count();
            Ok(count)
        } else if location_name == "brain" {
            // Đếm subdirectories
            let count = std::fs::read_dir(location)?
                .flatten()
                .filter(|e| e.path().is_dir())
                .count();
            Ok(count)
        } else {
            Ok(0)
        }
    }

    fn copy_to_vault(&self, session: &SessionFile, vault_dir: &Path) -> Result<Option<PathBuf>> {
        // Tạo thư mục con theo source
        let source_dir = vault_dir.join(&session.metadata.source);
        std::fs::create_dir_all(&source_dir)?;

        // Xử lý path khác nhau cho conversations và artifacts
        let dest_path = if session.metadata.source == "antigravity-artifact" {
            // Artifacts: giữ cấu trúc {uuid}/{filename}
            let parts: Vec<&str> = session.metadata.id.split('/').collect();
            if parts.len() == 2 {
                let subfolder = source_dir.join(parts[0]);
                std::fs::create_dir_all(&subfolder)?;
                subfolder.join(parts[1])
            } else {
                source_dir.join(&session.metadata.id)
            }
        } else {
            // Conversations: giữ nguyên filename gốc
            let filename = session
                .source_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            source_dir.join(&filename)
        };

        // Incremental: chỉ copy nếu file mới hoặc đã thay đổi
        let should_copy = if dest_path.exists() {
            let src_meta = session.source_path.metadata()?;
            let dest_meta = dest_path.metadata()?;

            let src_modified = src_meta.modified()?;
            let dest_modified = dest_meta.modified()?;

            src_modified > dest_modified || src_meta.len() != dest_meta.len()
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
