//! Context Intelligence Engine
//!
//! Tracks context (project type, git state, command history) and provides
//! intelligent ranking boosts for completions based on usage patterns.

#![allow(dead_code)]

use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Project type detection
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Java,
    Ruby,
    Unknown,
}

impl ProjectType {
    /// Detect project type from current directory
    pub fn detect(cwd: &PathBuf) -> Self {
        // Check for Rust
        if cwd.join("Cargo.toml").exists() {
            return ProjectType::Rust;
        }

        // Check for Node.js
        if cwd.join("package.json").exists() {
            return ProjectType::Node;
        }

        // Check for Python
        if cwd.join("pyproject.toml").exists()
            || cwd.join("setup.py").exists()
            || cwd.join("requirements.txt").exists()
        {
            return ProjectType::Python;
        }

        // Check for Go
        if cwd.join("go.mod").exists() {
            return ProjectType::Go;
        }

        // Check for Java/Maven/Gradle
        if cwd.join("pom.xml").exists() || cwd.join("build.gradle").exists() {
            return ProjectType::Java;
        }

        // Check for Ruby
        if cwd.join("Gemfile").exists() {
            return ProjectType::Ruby;
        }

        ProjectType::Unknown
    }

    /// Get relevant commands for this project type
    pub fn relevant_commands(&self) -> &[&str] {
        match self {
            ProjectType::Rust => &["cargo", "rustc", "rustup", "clippy"],
            ProjectType::Node => &["npm", "yarn", "pnpm", "node", "npx"],
            ProjectType::Python => &["python", "pip", "poetry", "pipenv", "pytest"],
            ProjectType::Go => &["go"],
            ProjectType::Java => &["java", "javac", "mvn", "gradle"],
            ProjectType::Ruby => &["ruby", "gem", "bundle", "rake"],
            ProjectType::Unknown => &[],
        }
    }
}

/// Git repository state
#[derive(Debug, Clone, Default)]
pub struct GitState {
    /// Current branch name
    pub branch: Option<String>,
    /// Whether there are uncommitted changes
    pub is_dirty: bool,
    /// Number of commits ahead of upstream
    pub ahead: u32,
    /// Number of commits behind upstream
    pub behind: u32,
    /// Whether in a rebase/merge state
    pub in_progress: Option<GitOperation>,
    /// Recent branches (for quick switching)
    pub recent_branches: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum GitOperation {
    Merge,
    Rebase,
    CherryPick,
    Bisect,
}

impl GitState {
    /// Get current git state
    pub fn current() -> Option<Self> {
        // Check if in git repo
        let git_dir = Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .output()
            .ok()?;

        if !git_dir.status.success() {
            return None;
        }

        let branch = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());

        // Check for dirty state
        let is_dirty = Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .ok()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

        // Check ahead/behind
        let (ahead, behind) = Self::get_ahead_behind();

        // Check for in-progress operations
        let git_dir_path = String::from_utf8(git_dir.stdout)
            .ok()
            .map(|s| PathBuf::from(s.trim()));

        let in_progress = git_dir_path.and_then(|p| {
            if p.join("MERGE_HEAD").exists() {
                Some(GitOperation::Merge)
            } else if p.join("rebase-merge").exists() || p.join("rebase-apply").exists() {
                Some(GitOperation::Rebase)
            } else if p.join("CHERRY_PICK_HEAD").exists() {
                Some(GitOperation::CherryPick)
            } else if p.join("BISECT_LOG").exists() {
                Some(GitOperation::Bisect)
            } else {
                None
            }
        });

        // Get recent branches from reflog
        let recent_branches = Self::get_recent_branches();

        Some(GitState {
            branch,
            is_dirty,
            ahead,
            behind,
            in_progress,
            recent_branches,
        })
    }

    fn get_ahead_behind() -> (u32, u32) {
        Command::new("git")
            .args(["rev-list", "--count", "--left-right", "@{upstream}...HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| {
                let parts: Vec<&str> = s.trim().split('\t').collect();
                let behind = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                let ahead = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                (ahead, behind)
            })
            .unwrap_or((0, 0))
    }

    fn get_recent_branches() -> Vec<String> {
        Command::new("git")
            .args(["reflog", "show", "--format=%gs", "-n", "100"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|output| {
                let mut branches = Vec::new();
                for line in output.lines() {
                    // Look for "checkout: moving from X to Y"
                    if line.contains("checkout:") {
                        if let Some(to_branch) = line.split(" to ").last() {
                            let branch = to_branch.trim().to_string();
                            if !branches.contains(&branch) && branches.len() < 10 {
                                branches.push(branch);
                            }
                        }
                    }
                }
                branches
            })
            .unwrap_or_default()
    }

    /// Get suggested commands based on git state
    pub fn suggested_commands(&self) -> Vec<&'static str> {
        let mut suggestions = Vec::new();

        if self.is_dirty {
            suggestions.extend(&["git status", "git diff", "git add", "git commit"]);
        }

        if self.ahead > 0 {
            suggestions.push("git push");
        }

        if self.behind > 0 {
            suggestions.push("git pull");
        }

        match &self.in_progress {
            Some(GitOperation::Merge) => {
                suggestions.extend(&["git merge --continue", "git merge --abort"]);
            }
            Some(GitOperation::Rebase) => {
                suggestions.extend(&["git rebase --continue", "git rebase --abort"]);
            }
            Some(GitOperation::CherryPick) => {
                suggestions.extend(&["git cherry-pick --continue", "git cherry-pick --abort"]);
            }
            Some(GitOperation::Bisect) => {
                suggestions.extend(&["git bisect good", "git bisect bad", "git bisect reset"]);
            }
            None => {}
        }

        suggestions
    }
}

/// Command usage tracking
#[derive(Debug, Clone)]
pub struct CommandUsage {
    /// Number of times used
    pub count: u32,
    /// Last used timestamp
    pub last_used: Instant,
    /// Common flags used with this command
    pub common_flags: HashMap<String, u32>,
    /// Common arguments used
    pub common_args: HashMap<String, u32>,
}

impl Default for CommandUsage {
    fn default() -> Self {
        Self {
            count: 0,
            last_used: Instant::now(),
            common_flags: HashMap::new(),
            common_args: HashMap::new(),
        }
    }
}

/// Context tracker for intelligent completions
pub struct ContextTracker {
    /// Detected project type
    project_type: RwLock<Option<(PathBuf, ProjectType)>>,
    /// Git state (cached)
    git_state: RwLock<Option<(Instant, GitState)>>,
    /// Command usage tracking
    command_usage: RwLock<LruCache<String, CommandUsage>>,
    /// Directory-specific command patterns
    dir_patterns: RwLock<HashMap<PathBuf, Vec<String>>>,
    /// Recent paths accessed
    recent_paths: RwLock<LruCache<PathBuf, Instant>>,
    /// Recent branches used
    recent_branches: RwLock<LruCache<String, Instant>>,
    /// Session memory (transient values)
    session_memory: RwLock<HashMap<String, String>>,
}

impl ContextTracker {
    pub fn new() -> Self {
        Self {
            project_type: RwLock::new(None),
            git_state: RwLock::new(None),
            command_usage: RwLock::new(LruCache::new(NonZeroUsize::new(500).unwrap())),
            dir_patterns: RwLock::new(HashMap::new()),
            recent_paths: RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap())),
            recent_branches: RwLock::new(LruCache::new(NonZeroUsize::new(50).unwrap())),
            session_memory: RwLock::new(HashMap::new()),
        }
    }

    /// Get project type for current directory
    pub fn get_project_type(&self, cwd: &PathBuf) -> ProjectType {
        let cached = self.project_type.read().unwrap();
        if let Some((ref cached_cwd, ref ptype)) = *cached {
            if cached_cwd == cwd {
                return ptype.clone();
            }
        }
        drop(cached);

        let ptype = ProjectType::detect(cwd);
        *self.project_type.write().unwrap() = Some((cwd.clone(), ptype.clone()));
        ptype
    }

    /// Get git state (with caching)
    pub fn get_git_state(&self) -> Option<GitState> {
        let cached = self.git_state.read().unwrap();
        if let Some((ref time, ref state)) = *cached {
            // Cache for 2 seconds
            if time.elapsed() < Duration::from_secs(2) {
                return Some(state.clone());
            }
        }
        drop(cached);

        let state = GitState::current()?;
        *self.git_state.write().unwrap() = Some((Instant::now(), state.clone()));
        Some(state)
    }

    /// Record command execution
    pub fn record_command(&self, command: &str, args: &[String]) {
        let mut usage = self.command_usage.write().unwrap();

        let entry = usage.get_or_insert_mut(command.to_string(), CommandUsage::default);
        entry.count += 1;
        entry.last_used = Instant::now();

        // Track flags and args
        for arg in args {
            if arg.starts_with('-') {
                *entry.common_flags.entry(arg.clone()).or_insert(0) += 1;
            } else {
                *entry.common_args.entry(arg.clone()).or_insert(0) += 1;
            }
        }
    }

    /// Record directory pattern
    pub fn record_dir_pattern(&self, cwd: &PathBuf, command: &str) {
        let mut patterns = self.dir_patterns.write().unwrap();
        let entry = patterns.entry(cwd.clone()).or_default();

        if !entry.contains(&command.to_string()) {
            entry.push(command.to_string());
            // Keep only last 20 commands per directory
            if entry.len() > 20 {
                entry.remove(0);
            }
        }
    }

    /// Record path access
    pub fn record_path(&self, path: PathBuf) {
        self.recent_paths.write().unwrap().put(path, Instant::now());
    }

    /// Record branch usage
    pub fn record_branch(&self, branch: &str) {
        self.recent_branches
            .write()
            .unwrap()
            .put(branch.to_string(), Instant::now());
    }

    /// Set session memory value
    pub fn set_memory(&self, key: &str, value: &str) {
        self.session_memory
            .write()
            .unwrap()
            .insert(key.to_string(), value.to_string());
    }

    /// Get session memory value
    pub fn get_memory(&self, key: &str) -> Option<String> {
        self.session_memory.read().unwrap().get(key).cloned()
    }

    /// Calculate score boost for a suggestion based on context
    pub fn score_boost(&self, suggestion: &str, cwd: &PathBuf) -> i64 {
        let mut boost = 0i64;

        // Boost for project-relevant commands
        let project_type = self.get_project_type(cwd);
        if project_type
            .relevant_commands()
            .iter()
            .any(|c| suggestion.starts_with(c))
        {
            boost += 20;
        }

        // Boost for frequently used commands
        if let Some(usage) = self.command_usage.read().unwrap().peek(suggestion) {
            boost += (usage.count.min(100) / 10) as i64; // Max +10 for frequency

            // Boost for recency
            if usage.last_used.elapsed() < Duration::from_secs(300) {
                boost += 15; // Used in last 5 minutes
            } else if usage.last_used.elapsed() < Duration::from_secs(3600) {
                boost += 5; // Used in last hour
            }
        }

        // Boost for directory patterns
        if let Some(patterns) = self.dir_patterns.read().unwrap().get(cwd) {
            if patterns.contains(&suggestion.to_string()) {
                boost += 25;
            }
        }

        // Boost for git-related commands when in dirty state
        if let Some(ref git_state) = self.get_git_state() {
            if git_state.is_dirty
                && ["git status", "git add", "git diff", "git commit"]
                    .iter()
                    .any(|c| suggestion.starts_with(c))
            {
                boost += 30;
            }

            if git_state.ahead > 0 && suggestion.starts_with("git push") {
                boost += 25;
            }

            if git_state.behind > 0 && suggestion.starts_with("git pull") {
                boost += 25;
            }
        }

        boost
    }

    /// Get recent paths for completion
    pub fn get_recent_paths(&self, limit: usize) -> Vec<PathBuf> {
        self.recent_paths
            .read()
            .unwrap()
            .iter()
            .take(limit)
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// Get recent branches for completion
    pub fn get_recent_branches(&self, limit: usize) -> Vec<String> {
        self.recent_branches
            .read()
            .unwrap()
            .iter()
            .take(limit)
            .map(|(b, _)| b.clone())
            .collect()
    }

    /// Get suggested commands based on current context
    pub fn get_contextual_suggestions(&self, cwd: &PathBuf) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Add git suggestions
        if let Some(ref git_state) = self.get_git_state() {
            suggestions.extend(git_state.suggested_commands().iter().map(|s| s.to_string()));
        }

        // Add project-type suggestions
        let project_type = self.get_project_type(cwd);
        match project_type {
            ProjectType::Rust => {
                suggestions.extend(vec![
                    "cargo build".to_string(),
                    "cargo run".to_string(),
                    "cargo test".to_string(),
                    "cargo check".to_string(),
                ]);
            }
            ProjectType::Node => {
                suggestions.extend(vec![
                    "npm install".to_string(),
                    "npm run".to_string(),
                    "npm test".to_string(),
                ]);
            }
            ProjectType::Python => {
                suggestions.extend(vec![
                    "python".to_string(),
                    "pip install".to_string(),
                    "pytest".to_string(),
                ]);
            }
            _ => {}
        }

        // Add directory patterns
        if let Some(patterns) = self.dir_patterns.read().unwrap().get(cwd) {
            suggestions.extend(patterns.clone());
        }

        // Deduplicate
        suggestions.sort();
        suggestions.dedup();

        suggestions
    }
}

impl Default for ContextTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Global context tracker instance
static CONTEXT_TRACKER: once_cell::sync::Lazy<ContextTracker> =
    once_cell::sync::Lazy::new(ContextTracker::new);

/// Get the global context tracker
pub fn tracker() -> &'static ContextTracker {
    &CONTEXT_TRACKER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_type_detection() {
        // Test with current directory (should be Rust since we have Cargo.toml)
        let cwd = std::env::current_dir().unwrap();
        let ptype = ProjectType::detect(&cwd);

        // This should be Rust since we're in a Rust project
        assert_eq!(ptype, ProjectType::Rust);
    }

    #[test]
    fn test_project_relevant_commands() {
        assert!(ProjectType::Rust.relevant_commands().contains(&"cargo"));
        assert!(ProjectType::Node.relevant_commands().contains(&"npm"));
        assert!(ProjectType::Python.relevant_commands().contains(&"python"));
    }

    #[test]
    fn test_context_tracker_creation() {
        let tracker = ContextTracker::new();
        let cwd = std::env::current_dir().unwrap();
        let ptype = tracker.get_project_type(&cwd);
        assert_eq!(ptype, ProjectType::Rust);
    }

    #[test]
    fn test_command_recording() {
        let tracker = ContextTracker::new();
        tracker.record_command("git", &["commit".to_string(), "-m".to_string()]);
        tracker.record_command("git", &["push".to_string()]);

        // The command should be recorded
        let usage = tracker.command_usage.read().unwrap();
        assert!(usage.peek(&"git".to_string()).is_some());
    }

    #[test]
    fn test_session_memory() {
        let tracker = ContextTracker::new();
        tracker.set_memory("last_branch", "feature-xyz");
        assert_eq!(
            tracker.get_memory("last_branch"),
            Some("feature-xyz".to_string())
        );
    }

    #[test]
    fn test_score_boost() {
        let tracker = ContextTracker::new();
        let cwd = std::env::current_dir().unwrap();

        // Record some usage
        tracker.record_command("cargo", &["build".to_string()]);
        tracker.record_command("cargo", &["build".to_string()]);
        tracker.record_command("cargo", &["build".to_string()]);

        // Cargo should get a boost in a Rust project
        let boost = tracker.score_boost("cargo build", &cwd);
        assert!(boost > 0);
    }
}
