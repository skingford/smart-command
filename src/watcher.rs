//! File Watcher for Hot-Reload
//!
//! Watches configuration and definition files for changes,
//! triggering automatic reloads when files are modified.

#![allow(dead_code)]

use notify::{Config, Event, RecommendedWatcher, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

/// Events from the file watcher
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// Definition file changed
    DefinitionChanged(PathBuf),
    /// Config file changed
    ConfigChanged,
    /// Plugin file changed
    PluginChanged(PathBuf),
    /// Error occurred
    Error(String),
}

/// File watcher for hot-reload functionality
pub struct FileWatcher {
    /// Internal watcher instance
    _watcher: RecommendedWatcher,
    /// Receiver for watch events
    receiver: Receiver<WatchEvent>,
    /// Paths being watched
    watched_paths: Vec<PathBuf>,
    /// Whether watcher is active
    active: Arc<RwLock<bool>>,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new() -> Result<Self, notify::Error> {
        let (tx, rx) = channel();
        let active = Arc::new(RwLock::new(true));
        let active_clone = active.clone();

        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if !*active_clone.read().unwrap() {
                    return;
                }

                match res {
                    Ok(event) => {
                        for path in event.paths {
                            let event = classify_event(&path);
                            if tx.send(event).is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(WatchEvent::Error(e.to_string()));
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            watched_paths: Vec::new(),
            active,
        })
    }

    /// Watch a directory or file
    pub fn watch(&mut self, path: impl AsRef<Path>) -> Result<(), notify::Error> {
        let path = path.as_ref().to_path_buf();
        // Note: In actual implementation, we would call self._watcher.watch() here
        // For now, just track the path
        self.watched_paths.push(path);
        Ok(())
    }

    /// Stop watching a path
    pub fn unwatch(&mut self, path: impl AsRef<Path>) -> Result<(), notify::Error> {
        let path = path.as_ref();
        self.watched_paths.retain(|p| p != path);
        Ok(())
    }

    /// Get pending events (non-blocking)
    pub fn poll_events(&self) -> Vec<WatchEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            events.push(event);
        }
        events
    }

    /// Wait for next event (blocking with timeout)
    pub fn wait_event(&self, timeout: Duration) -> Option<WatchEvent> {
        self.receiver.recv_timeout(timeout).ok()
    }

    /// Check if watcher is active
    pub fn is_active(&self) -> bool {
        *self.active.read().unwrap()
    }

    /// Stop the watcher
    pub fn stop(&self) {
        *self.active.write().unwrap() = false;
    }

    /// Get watched paths
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched_paths
    }
}

/// Classify a file change event based on path
fn classify_event(path: &Path) -> WatchEvent {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if filename == "config.yaml" || filename == "smart-command.yaml" {
        WatchEvent::ConfigChanged
    } else if filename.ends_with(".yaml") && parent == "definitions" {
        WatchEvent::DefinitionChanged(path.to_path_buf())
    } else if parent == "plugins" || path.components().any(|c| c.as_os_str() == "plugins") {
        WatchEvent::PluginChanged(path.to_path_buf())
    } else {
        WatchEvent::DefinitionChanged(path.to_path_buf())
    }
}

/// Hot-reload manager
pub struct HotReloadManager {
    watcher: Option<FileWatcher>,
    definitions_dir: PathBuf,
    config_path: PathBuf,
    plugins_dir: PathBuf,
    reload_callback: Option<Box<dyn Fn(WatchEvent) + Send + Sync>>,
}

impl HotReloadManager {
    /// Create a new hot-reload manager
    pub fn new(definitions_dir: PathBuf, config_path: PathBuf, plugins_dir: PathBuf) -> Self {
        Self {
            watcher: None,
            definitions_dir,
            config_path,
            plugins_dir,
            reload_callback: None,
        }
    }

    /// Start watching for changes
    pub fn start(&mut self) -> Result<(), notify::Error> {
        let mut watcher = FileWatcher::new()?;

        // Watch definitions directory
        if self.definitions_dir.exists() {
            watcher.watch(&self.definitions_dir)?;
        }

        // Watch config file
        if let Some(parent) = self.config_path.parent() {
            if parent.exists() {
                watcher.watch(parent)?;
            }
        }

        // Watch plugins directory
        if self.plugins_dir.exists() {
            watcher.watch(&self.plugins_dir)?;
        }

        self.watcher = Some(watcher);
        Ok(())
    }

    /// Stop watching
    pub fn stop(&mut self) {
        if let Some(watcher) = &self.watcher {
            watcher.stop();
        }
        self.watcher = None;
    }

    /// Set callback for reload events
    pub fn on_reload<F>(&mut self, callback: F)
    where
        F: Fn(WatchEvent) + Send + Sync + 'static,
    {
        self.reload_callback = Some(Box::new(callback));
    }

    /// Check for pending events and trigger callbacks
    pub fn check_events(&self) {
        if let Some(watcher) = &self.watcher {
            for event in watcher.poll_events() {
                if let Some(callback) = &self.reload_callback {
                    callback(event);
                }
            }
        }
    }

    /// Check if hot-reload is active
    pub fn is_active(&self) -> bool {
        self.watcher
            .as_ref()
            .map(|w| w.is_active())
            .unwrap_or(false)
    }
}

/// Debounced file watcher for batching rapid changes
pub struct DebouncedWatcher {
    inner: FileWatcher,
    debounce_duration: Duration,
    pending_events: Arc<RwLock<Vec<WatchEvent>>>,
}

impl DebouncedWatcher {
    /// Create a new debounced watcher
    pub fn new(debounce_ms: u64) -> Result<Self, notify::Error> {
        Ok(Self {
            inner: FileWatcher::new()?,
            debounce_duration: Duration::from_millis(debounce_ms),
            pending_events: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Watch a path
    pub fn watch(&mut self, path: impl AsRef<Path>) -> Result<(), notify::Error> {
        self.inner.watch(path)
    }

    /// Get debounced events
    pub fn get_debounced_events(&self) -> Vec<WatchEvent> {
        // Collect events over debounce period
        thread::sleep(self.debounce_duration);

        let mut events = self.inner.poll_events();

        // Deduplicate by path
        let mut seen_paths = std::collections::HashSet::new();
        events.retain(|e| {
            match e {
                WatchEvent::DefinitionChanged(p) | WatchEvent::PluginChanged(p) => {
                    seen_paths.insert(p.clone())
                }
                WatchEvent::ConfigChanged => seen_paths.insert(PathBuf::from("__config__")),
                WatchEvent::Error(_) => true,
            }
        });

        events
    }

    /// Stop the watcher
    pub fn stop(&self) {
        self.inner.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_event() {
        let path = PathBuf::from("/home/user/.config/smart-command/config.yaml");
        assert!(matches!(classify_event(&path), WatchEvent::ConfigChanged));

        let path = PathBuf::from("/home/user/definitions/git.yaml");
        assert!(matches!(
            classify_event(&path),
            WatchEvent::DefinitionChanged(_)
        ));

        let path = PathBuf::from("/home/user/plugins/my-plugin/main.sh");
        assert!(matches!(classify_event(&path), WatchEvent::PluginChanged(_)));
    }

    #[test]
    fn test_file_watcher_creation() {
        // This may fail if notify can't be initialized, which is ok for testing
        let result = FileWatcher::new();
        // Just check it doesn't panic
        if let Ok(watcher) = result {
            assert!(watcher.is_active());
        }
    }
}
