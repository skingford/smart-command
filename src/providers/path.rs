//! Enhanced path completion provider
//!
//! Provides advanced path completions with filtering, bookmarks, and fuzzy matching.

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Duration;

/// Path filter configuration
#[derive(Debug, Clone, Default)]
pub struct PathFilter {
    /// Only show files with these extensions (e.g., [".rs", ".toml"])
    pub extensions: Option<Vec<String>>,
    /// Exclude paths matching these patterns
    pub exclude_patterns: Vec<String>,
    /// Include hidden files (starting with .)
    pub include_hidden: bool,
    /// Only show files
    pub files_only: bool,
    /// Only show directories
    pub dirs_only: bool,
}

/// Enhanced path completion provider
pub struct PathProvider {
    /// User-defined bookmarks (@name -> path)
    bookmarks: RwLock<HashMap<String, PathBuf>>,
    /// Default exclude patterns (node_modules, target, .git, etc.)
    default_excludes: Vec<String>,
    /// Maximum recursion depth for fuzzy path matching
    max_depth: usize,
}

impl PathProvider {
    pub fn new() -> Self {
        let mut bookmarks = HashMap::new();

        // Default bookmarks
        if let Some(home) = dirs::home_dir() {
            bookmarks.insert("home".to_string(), home.clone());
            bookmarks.insert("config".to_string(), home.join(".config"));
            bookmarks.insert("downloads".to_string(), home.join("Downloads"));
            bookmarks.insert("documents".to_string(), home.join("Documents"));
            bookmarks.insert("desktop".to_string(), home.join("Desktop"));
        }

        Self {
            bookmarks: RwLock::new(bookmarks),
            default_excludes: vec![
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                ".svn".to_string(),
                ".hg".to_string(),
                "__pycache__".to_string(),
                ".pytest_cache".to_string(),
                "venv".to_string(),
                ".venv".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".next".to_string(),
                ".nuxt".to_string(),
                "coverage".to_string(),
            ],
            max_depth: 3,
        }
    }

    /// Add a bookmark
    pub fn add_bookmark(&self, name: &str, path: PathBuf) {
        self.bookmarks
            .write()
            .unwrap()
            .insert(name.to_string(), path);
    }

    /// Remove a bookmark
    pub fn remove_bookmark(&self, name: &str) {
        self.bookmarks.write().unwrap().remove(name);
    }

    /// Get bookmark path
    fn get_bookmark(&self, name: &str) -> Option<PathBuf> {
        self.bookmarks.read().unwrap().get(name).cloned()
    }

    /// Check if a path should be excluded
    fn should_exclude(&self, name: &str, filter: &PathFilter) -> bool {
        // Hidden files
        if name.starts_with('.') && !filter.include_hidden {
            return true;
        }

        // Default excludes
        if self.default_excludes.iter().any(|e| name == e) {
            return true;
        }

        // Custom excludes
        if filter
            .exclude_patterns
            .iter()
            .any(|p| name.contains(p.as_str()))
        {
            return true;
        }

        false
    }

    /// List directory contents with filtering
    fn list_dir(&self, dir: &Path, filter: &PathFilter) -> Vec<(String, bool)> {
        let Ok(entries) = fs::read_dir(dir) else {
            return vec![];
        };

        entries
            .filter_map(|e| e.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

                // Apply exclusions
                if self.should_exclude(&name, filter) {
                    return None;
                }

                // Apply file/dir filter
                if filter.files_only && is_dir {
                    return None;
                }
                if filter.dirs_only && !is_dir {
                    return None;
                }

                // Apply extension filter
                if !is_dir {
                    if let Some(ref exts) = filter.extensions {
                        let has_ext = exts.iter().any(|ext| name.ends_with(ext));
                        if !has_ext {
                            return None;
                        }
                    }
                }

                Some((name, is_dir))
            })
            .collect()
    }

    /// Fuzzy match path segments
    fn fuzzy_path_match(&self, base: &Path, partial: &str, filter: &PathFilter) -> Vec<String> {
        let matcher = SkimMatcherV2::default();
        let mut results = Vec::new();

        // Split partial into segments
        let segments: Vec<&str> = partial.split('/').collect();
        if segments.is_empty() {
            return results;
        }

        // If partial starts with /, use absolute path
        let search_base = if partial.starts_with('/') {
            PathBuf::from("/")
        } else {
            base.to_path_buf()
        };

        self.fuzzy_search_recursive(&search_base, &segments, 0, filter, &matcher, &mut results);

        results
    }

    fn fuzzy_search_recursive(
        &self,
        current: &Path,
        segments: &[&str],
        depth: usize,
        filter: &PathFilter,
        matcher: &SkimMatcherV2,
        results: &mut Vec<String>,
    ) {
        if depth >= self.max_depth || segments.is_empty() {
            return;
        }

        let pattern = segments[0];
        let remaining = &segments[1..];

        let entries = self.list_dir(current, filter);

        for (name, is_dir) in entries {
            // Check if name matches pattern (fuzzy)
            let matches = if pattern.is_empty() {
                true
            } else {
                matcher.fuzzy_match(&name, pattern).is_some()
                    || name.to_lowercase().starts_with(&pattern.to_lowercase())
            };

            if !matches {
                continue;
            }

            let full_path = current.join(&name);

            if remaining.is_empty() {
                // This is the final segment
                let path_str = full_path.to_string_lossy().to_string();
                results.push(if is_dir {
                    format!("{}/", path_str)
                } else {
                    path_str
                });
            } else if is_dir {
                // Continue searching in subdirectory
                self.fuzzy_search_recursive(
                    &full_path,
                    remaining,
                    depth + 1,
                    filter,
                    matcher,
                    results,
                );
            }
        }
    }

    /// Parse path and handle special prefixes
    fn parse_path(&self, partial: &str, cwd: &Path) -> (PathBuf, String) {
        // Handle @ bookmark prefix
        if partial.starts_with('@') {
            let parts: Vec<&str> = partial[1..].splitn(2, '/').collect();
            let bookmark_name = parts[0];

            if let Some(bookmark_path) = self.get_bookmark(bookmark_name) {
                let remaining = parts.get(1).unwrap_or(&"");
                return (bookmark_path, remaining.to_string());
            }
        }

        // Handle ~ home directory
        if partial.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                let remaining = if partial.len() > 1 {
                    &partial[2..] // Skip ~/
                } else {
                    ""
                };
                return (home, remaining.to_string());
            }
        }

        // Handle absolute paths
        if partial.starts_with('/') {
            let path = PathBuf::from(partial);
            if let Some(parent) = path.parent() {
                let filename = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                return (parent.to_path_buf(), filename);
            }
            return (PathBuf::from("/"), partial[1..].to_string());
        }

        // Relative path
        if partial.contains('/') {
            let path = cwd.join(partial);
            if let Some(parent) = path.parent() {
                let filename = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                return (parent.to_path_buf(), filename);
            }
        }

        (cwd.to_path_buf(), partial.to_string())
    }
}

impl Default for PathProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for PathProvider {
    fn name(&self) -> &str {
        "path"
    }

    fn matches(&self, _cmd: &str, _arg_position: usize, context: &ProviderContext) -> bool {
        // Path provider is a fallback - it matches when other providers don't
        // Or when the partial looks like a path

        let partial = &context.partial_input;

        // Explicit path indicators
        if partial.starts_with('/')
            || partial.starts_with('~')
            || partial.starts_with('@')
            || partial.starts_with('.')
            || partial.contains('/')
        {
            return true;
        }

        // Also match for common file-related commands
        let file_commands = [
            "cat", "less", "more", "head", "tail", "vim", "nvim", "nano", "code", "subl", "cp",
            "mv", "rm", "chmod", "chown", "stat", "file", "touch", "mkdir", "source", ".",
        ];

        file_commands.contains(&context.command.as_str())
    }

    fn complete(&self, partial: &str, context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let filter = PathFilter::default();

        // Handle bookmark completion
        if partial.starts_with('@') && !partial.contains('/') {
            let bookmark_partial = &partial[1..];
            let bookmarks = self.bookmarks.read().unwrap();

            return bookmarks
                .iter()
                .filter(|(name, _)| name.starts_with(bookmark_partial))
                .map(|(name, path)| {
                    ProviderSuggestion::new(format!("@{}/", name))
                        .with_description(path.to_string_lossy().to_string())
                        .with_category("bookmark")
                        .with_score(150)
                        .no_whitespace()
                })
                .collect();
        }

        let (base_dir, filename_partial) = self.parse_path(partial, &context.cwd);

        // If the partial contains multiple /, try fuzzy path matching
        if partial.matches('/').count() > 1 && !filename_partial.is_empty() {
            let results = self.fuzzy_path_match(&context.cwd, partial, &filter);
            return results
                .into_iter()
                .map(|path| {
                    let is_dir = path.ends_with('/');
                    ProviderSuggestion::new(&path)
                        .with_description(if is_dir { "directory" } else { "file" })
                        .with_category(if is_dir { "dir" } else { "file" })
                        .with_score(100)
                })
                .collect();
        }

        // Standard directory listing
        let entries = self.list_dir(&base_dir, &filter);
        let partial_lower = filename_partial.to_lowercase();

        entries
            .into_iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&partial_lower))
            .map(|(name, is_dir)| {
                // Reconstruct full path for completion
                let completion = if partial.starts_with('/') {
                    let full = base_dir.join(&name);
                    full.to_string_lossy().to_string()
                } else if partial.contains('/') {
                    // Preserve the relative path prefix
                    let prefix = partial.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
                    format!("{}/{}", prefix, name)
                } else {
                    name.clone()
                };

                let display = if is_dir {
                    format!("{}/", completion)
                } else {
                    completion
                };

                ProviderSuggestion::new(display)
                    .with_description(if is_dir { "directory" } else { "file" })
                    .with_category(if is_dir { "dir" } else { "file" })
                    .with_score(if is_dir { 90 } else { 80 })
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        // Don't cache path completions - filesystem changes frequently
        None
    }

    fn priority(&self) -> i32 {
        10 // Low priority - acts as fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_provider_matches() {
        let provider = PathProvider::new();

        let ctx = ProviderContext::new(PathBuf::from("."), "cat", vec![], "./sr");
        assert!(provider.matches("cat", 1, &ctx));

        let ctx2 = ProviderContext::new(PathBuf::from("."), "cat", vec![], "~/.config");
        assert!(provider.matches("cat", 1, &ctx2));
    }

    #[test]
    fn test_bookmarks() {
        let provider = PathProvider::new();

        provider.add_bookmark("test", PathBuf::from("/tmp/test"));
        assert_eq!(
            provider.get_bookmark("test"),
            Some(PathBuf::from("/tmp/test"))
        );

        provider.remove_bookmark("test");
        assert_eq!(provider.get_bookmark("test"), None);
    }

    #[test]
    fn test_parse_path_relative() {
        let provider = PathProvider::new();
        let cwd = PathBuf::from("/home/user");

        let (base, partial) = provider.parse_path("src/main", &cwd);
        assert_eq!(base, PathBuf::from("/home/user/src"));
        assert_eq!(partial, "main");
    }

    #[test]
    fn test_default_excludes() {
        let provider = PathProvider::new();
        let filter = PathFilter::default();

        assert!(provider.should_exclude("node_modules", &filter));
        assert!(provider.should_exclude(".git", &filter));
        assert!(!provider.should_exclude("src", &filter));
    }
}
