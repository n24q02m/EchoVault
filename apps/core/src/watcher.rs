//! File System Watcher
//!
//! Module cung cấp khả năng theo dõi thay đổi file system theo event-driven.
//! Thay thế polling để giảm RAM và CPU usage.

use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// Watcher cho file system events
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    rx: Receiver<Result<Event, notify::Error>>,
}

impl FileWatcher {
    /// Tạo watcher mới
    pub fn new() -> Result<Self> {
        let (tx, rx) = channel();

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        Ok(Self { watcher, rx })
    }

    /// Bắt đầu theo dõi một thư mục
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(())
    }

    /// Dừng theo dõi một thư mục
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.watcher.unwatch(path)?;
        Ok(())
    }

    /// Lấy event tiếp theo (blocking)
    pub fn next_event(&self) -> Option<Event> {
        match self.rx.recv() {
            Ok(Ok(event)) => Some(event),
            _ => None,
        }
    }

    /// Lấy event với timeout
    pub fn next_event_timeout(&self, timeout: Duration) -> Option<Event> {
        match self.rx.recv_timeout(timeout) {
            Ok(Ok(event)) => Some(event),
            _ => None,
        }
    }

    /// Kiểm tra có event pending không (non-blocking)
    pub fn try_next_event(&self) -> Option<Event> {
        match self.rx.try_recv() {
            Ok(Ok(event)) => Some(event),
            _ => None,
        }
    }
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new().expect("Failed to create FileWatcher")
    }
}
