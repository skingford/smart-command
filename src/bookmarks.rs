//! Directory Bookmarks System
//!
//! Provides quick navigation to frequently used directories:
//! - Save bookmarks with custom names
//! - Jump to bookmarks with `@name` syntax
//! - Track visit frequency for smart suggestions

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A directory bookmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    /// Bookmark name
    pub name: String,
    /// The directory path
    pub path: PathBuf,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Number of times visited
    #[serde(default)]
    pub visit_count: u64,
    /// Last visited timestamp
    #[serde(default)]
    pub last_visited: u64,
    /// Created timestamp
    #[serde(default)]
    pub created_at: u64,
}

impl Bookmark {
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            name: name.into(),
            path: path.into(),
            description: None,
            visit_count: 0,
            last_visited: now,
            created_at: now,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Record a visit to this bookmark
    pub fn record_visit(&mut self) {
        self.visit_count += 1;
        self.last_visited = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
}

/// Bookmark manager
#[derive(Debug)]
pub struct BookmarkManager {
    bookmarks: HashMap<String, Bookmark>,
    config_path: PathBuf,
}

impl BookmarkManager {
    /// Create a new bookmark manager
    pub fn new() -> Self {
        let config_path = dirs::config_dir()
            .map(|p| p.join("smart-command").join("bookmarks.yaml"))
            .unwrap_or_else(|| PathBuf::from("~/.config/smart-command/bookmarks.yaml"));

        let mut manager = Self {
            bookmarks: HashMap::new(),
            config_path,
        };

        manager.load();
        manager.add_default_bookmarks();
        manager
    }

    /// Add default useful bookmarks
    fn add_default_bookmarks(&mut self) {
        // Only add home if not already present
        if !self.bookmarks.contains_key("home") {
            if let Some(home) = dirs::home_dir() {
                self.bookmarks.insert(
                    "home".to_string(),
                    Bookmark::new("home", &home).with_description("Home directory"),
                );
            }
        }

        // Desktop
        if !self.bookmarks.contains_key("desktop") {
            if let Some(desktop) = dirs::desktop_dir() {
                self.bookmarks.insert(
                    "desktop".to_string(),
                    Bookmark::new("desktop", &desktop).with_description("Desktop"),
                );
            }
        }

        // Documents
        if !self.bookmarks.contains_key("docs") {
            if let Some(docs) = dirs::document_dir() {
                self.bookmarks.insert(
                    "docs".to_string(),
                    Bookmark::new("docs", &docs).with_description("Documents"),
                );
            }
        }

        // Downloads
        if !self.bookmarks.contains_key("downloads") {
            if let Some(downloads) = dirs::download_dir() {
                self.bookmarks.insert(
                    "downloads".to_string(),
                    Bookmark::new("downloads", &downloads).with_description("Downloads"),
                );
            }
        }

        // Config
        if !self.bookmarks.contains_key("config") {
            if let Some(config) = dirs::config_dir() {
                self.bookmarks.insert(
                    "config".to_string(),
                    Bookmark::new("config", &config).with_description("Config directory"),
                );
            }
        }
    }

    /// Load bookmarks from config file
    pub fn load(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.config_path) {
            if let Ok(bookmarks) = serde_yaml::from_str::<Vec<Bookmark>>(&content) {
                for bookmark in bookmarks {
                    self.bookmarks.insert(bookmark.name.clone(), bookmark);
                }
            }
        }
    }

    /// Save bookmarks to config file
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let bookmarks: Vec<&Bookmark> = self.bookmarks.values().collect();
        let content = serde_yaml::to_string(&bookmarks)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&self.config_path, content)
    }

    /// Add a bookmark
    pub fn add(&mut self, name: &str, path: PathBuf, description: Option<&str>) {
        let mut bookmark = Bookmark::new(name, path);
        if let Some(desc) = description {
            bookmark = bookmark.with_description(desc);
        }
        self.bookmarks.insert(name.to_string(), bookmark);
    }

    /// Remove a bookmark
    pub fn remove(&mut self, name: &str) -> bool {
        self.bookmarks.remove(name).is_some()
    }

    /// Get a bookmark by name
    pub fn get(&self, name: &str) -> Option<&Bookmark> {
        self.bookmarks.get(name)
    }

    /// Get a mutable bookmark by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Bookmark> {
        self.bookmarks.get_mut(name)
    }

    /// List all bookmarks
    pub fn list(&self) -> Vec<&Bookmark> {
        let mut bookmarks: Vec<_> = self.bookmarks.values().collect();
        bookmarks.sort_by(|a, b| b.visit_count.cmp(&a.visit_count).then_with(|| a.name.cmp(&b.name)));
        bookmarks
    }

    /// Get bookmark suggestions for completion
    pub fn get_suggestions(&self, partial: &str) -> Vec<(&str, &PathBuf, Option<&str>)> {
        self.bookmarks
            .values()
            .filter(|b| {
                partial.is_empty()
                    || b.name.to_lowercase().starts_with(&partial.to_lowercase())
            })
            .map(|b| (b.name.as_str(), &b.path, b.description.as_deref()))
            .collect()
    }

    /// Check if input is a bookmark reference (@name)
    pub fn try_resolve(&self, input: &str) -> Option<&PathBuf> {
        if input.starts_with('@') {
            let name = &input[1..];
            self.bookmarks.get(name).map(|b| &b.path)
        } else {
            None
        }
    }

    /// Record a visit and save
    pub fn record_visit(&mut self, name: &str) {
        if let Some(bookmark) = self.bookmarks.get_mut(name) {
            bookmark.record_visit();
            let _ = self.save();
        }
    }
}

impl Default for BookmarkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle bookmark-related commands
pub fn handle_bookmark_command(
    manager: &mut BookmarkManager,
    cmd: &str,
    args: &[&str],
    cwd: &std::path::Path,
) -> Option<String> {
    match cmd {
        "bookmark" | "bm" => {
            if args.is_empty() {
                // List all bookmarks
                let bookmarks = manager.list();
                if bookmarks.is_empty() {
                    Some("No bookmarks defined.".to_string())
                } else {
                    let output: Vec<String> = bookmarks
                        .iter()
                        .map(|b| {
                            let desc = b
                                .description
                                .as_ref()
                                .map(|d| format!("  # {}", d))
                                .unwrap_or_default();
                            let visits = if b.visit_count > 0 {
                                format!(" (visited {} times)", b.visit_count)
                            } else {
                                String::new()
                            };
                            format!("@{} -> {}{}{}", b.name, b.path.display(), visits, desc)
                        })
                        .collect();
                    Some(output.join("\n"))
                }
            } else if args.len() == 1 {
                let arg = args[0];
                if arg == "." || arg == "here" {
                    Some("Usage: bookmark <name> to save current directory".to_string())
                } else {
                    // Save current directory with given name
                    let name = arg.trim_start_matches('@');
                    manager.add(name, cwd.to_path_buf(), None);
                    let _ = manager.save();
                    Some(format!("Bookmarked current directory as @{}", name))
                }
            } else {
                // bookmark <name> <path> [description]
                let name = args[0].trim_start_matches('@');
                let path = PathBuf::from(args[1]);
                let desc = if args.len() > 2 {
                    Some(args[2..].join(" "))
                } else {
                    None
                };

                let canonical = path.canonicalize().unwrap_or(path);
                manager.add(name, canonical.clone(), desc.as_deref());
                let _ = manager.save();
                Some(format!("Bookmarked {} as @{}", canonical.display(), name))
            }
        }
        "unbookmark" | "unbm" => {
            if args.is_empty() {
                Some("Usage: unbookmark <name>".to_string())
            } else {
                let name = args[0].trim_start_matches('@');
                if manager.remove(name) {
                    let _ = manager.save();
                    Some(format!("Removed bookmark @{}", name))
                } else {
                    Some(format!("Bookmark @{} not found", name))
                }
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_bookmark_creation() {
        let bookmark = Bookmark::new("test", "/tmp/test").with_description("Test bookmark");

        assert_eq!(bookmark.name, "test");
        assert_eq!(bookmark.path, PathBuf::from("/tmp/test"));
        assert_eq!(bookmark.description, Some("Test bookmark".to_string()));
    }

    #[test]
    fn test_bookmark_visit() {
        let mut bookmark = Bookmark::new("test", "/tmp");
        assert_eq!(bookmark.visit_count, 0);

        bookmark.record_visit();
        assert_eq!(bookmark.visit_count, 1);

        bookmark.record_visit();
        assert_eq!(bookmark.visit_count, 2);
    }

    #[test]
    fn test_bookmark_manager() {
        let mut manager = BookmarkManager {
            bookmarks: HashMap::new(),
            config_path: PathBuf::from("/tmp/test-bookmarks.yaml"),
        };

        manager.add("test", PathBuf::from("/tmp/test"), Some("Test"));
        assert!(manager.get("test").is_some());
        assert_eq!(
            manager.get("test").unwrap().path,
            PathBuf::from("/tmp/test")
        );

        manager.remove("test");
        assert!(manager.get("test").is_none());
    }

    #[test]
    fn test_try_resolve() {
        let mut manager = BookmarkManager {
            bookmarks: HashMap::new(),
            config_path: PathBuf::from("/tmp/test-bookmarks.yaml"),
        };

        manager.add("proj", PathBuf::from("/home/user/project"), None);

        assert_eq!(
            manager.try_resolve("@proj"),
            Some(&PathBuf::from("/home/user/project"))
        );
        assert!(manager.try_resolve("proj").is_none());
        assert!(manager.try_resolve("@unknown").is_none());
    }
}
