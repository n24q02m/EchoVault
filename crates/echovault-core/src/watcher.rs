//! File Watcher - Theo dõi thay đổi trong IDE directories.
//!
//! Sử dụng `notify` crate để emit events khi có file changes.
//! Thay thế polling 5 phút bằng realtime watching.

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// File watcher để theo dõi IDE directories
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
    paths: Vec<PathBuf>,
}

impl FileWatcher {
    /// Tạo watcher mới cho danh sách paths
    pub fn new(paths: Vec<PathBuf>) -> notify::Result<Self> {
        let (tx, rx) = channel();

        // Tạo watcher với debounce 2 giây để tránh spam events
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        // Thêm paths để watch
        for path in &paths {
            if path.exists() {
                if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                    eprintln!("[FileWatcher] Cannot watch {:?}: {}", path, e);
                } else {
                    println!("[FileWatcher] Watching: {:?}", path);
                }
            }
        }

        Ok(Self {
            watcher,
            receiver: rx,
            paths,
        })
    }

    /// Thêm path mới để watch (nếu path tồn tại)
    pub fn add_path(&mut self, path: &PathBuf) -> anyhow::Result<()> {
        if path.exists() && !self.paths.contains(path) {
            self.watcher.watch(path, RecursiveMode::Recursive)?;
            self.paths.push(path.clone());
            println!("[FileWatcher] Added watch: {:?}", path);
        }
        Ok(())
    }

    /// Poll cho events (non-blocking)
    /// Trả về true nếu có file changes xảy ra
    pub fn has_changes(&self) -> bool {
        // Drain tất cả pending events
        let mut has_changes = false;
        while let Ok(result) = self.receiver.try_recv() {
            if let Ok(event) = result {
                // Chỉ quan tâm đến modify/create/remove events
                if matches!(
                    event.kind,
                    notify::EventKind::Modify(_)
                        | notify::EventKind::Create(_)
                        | notify::EventKind::Remove(_)
                ) {
                    // Skip nếu là .git directory
                    let is_git = event.paths.iter().any(|p| {
                        p.to_string_lossy().contains(".git")
                            || p.to_string_lossy().contains(".DS_Store")
                    });

                    if !is_git {
                        has_changes = true;
                        println!("[FileWatcher] Change detected: {:?}", event.paths);
                    }
                }
            }
        }
        has_changes
    }

    /// Block chờ cho đến khi có event hoặc timeout
    pub fn wait_for_change(&self, timeout: Duration) -> bool {
        match self.receiver.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                // Kiểm tra event loại gì
                matches!(
                    event.kind,
                    notify::EventKind::Modify(_)
                        | notify::EventKind::Create(_)
                        | notify::EventKind::Remove(_)
                )
            }
            _ => false,
        }
    }

    /// Lấy danh sách paths đang được watch
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.paths
    }
}

/// Lấy danh sách IDE storage paths cần watch
pub fn get_ide_storage_paths() -> Vec<PathBuf> {
    use crate::extractors::vscode_copilot::VSCodeCopilotExtractor;
    use crate::extractors::Extractor;

    let mut paths = Vec::new();

    // VS Code Copilot paths
    let copilot = VSCodeCopilotExtractor::new();
    if let Ok(locations) = copilot.find_storage_locations() {
        for loc in locations {
            paths.push(loc);
        }
    }

    // TODO: Thêm paths cho các IDE khác (Antigravity, Cursor,...)

    paths
}
