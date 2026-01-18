//! File System Watcher
//!
//! This module provides event-driven file system monitoring.
//! Replaces polling with native OS notifications for reduced RAM and CPU usage.

use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// File system event watcher.
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    rx: Receiver<Result<Event, notify::Error>>,
}

impl FileWatcher {
    /// Create a new file watcher.
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

    /// Start watching a directory recursively.
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(())
    }

    /// Stop watching a directory.
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.watcher.unwatch(path)?;
        Ok(())
    }

    /// Get the next event (blocking).
    pub fn next_event(&self) -> Option<Event> {
        match self.rx.recv() {
            Ok(Ok(event)) => Some(event),
            _ => None,
        }
    }

    /// Get the next event with timeout.
    pub fn next_event_timeout(&self, timeout: Duration) -> Option<Event> {
        match self.rx.recv_timeout(timeout) {
            Ok(Ok(event)) => Some(event),
            _ => None,
        }
    }

    /// Check for pending events (non-blocking).
    pub fn try_next_event(&self) -> Option<Event> {
        match self.rx.try_recv() {
            Ok(Ok(event)) => Some(event),
            _ => None,
        }
    }
}

impl Default for FileWatcher {
    /// Create a default FileWatcher.
    ///
    /// # Panics
    /// Panics if the watcher cannot be created. Use `FileWatcher::new()` for
    /// fallible construction.
    fn default() -> Self {
        Self::new().expect("Failed to create FileWatcher - check OS support for file watching")
    }
}
// Trigger
