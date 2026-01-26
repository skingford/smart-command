//! Git completion providers
//!
//! Provides dynamic completions for git branches, remotes, tags, stashes, and files.

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
use std::process::Command;
use std::time::Duration;

/// Helper to run git commands and capture output
fn git_command(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
}

/// Check if we're in a git repository
fn in_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the current branch name
fn current_branch() -> Option<String> {
    git_command(&["rev-parse", "--abbrev-ref", "HEAD"]).map(|s| s.trim().to_string())
}

// ============================================================================
// Git Branch Provider
// ============================================================================

/// Provides branch name completions for git commands
pub struct GitBranchProvider;

impl GitBranchProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_branches(&self, include_remote: bool) -> Vec<(String, bool)> {
        let mut branches = Vec::new();

        // Local branches
        if let Some(output) = git_command(&["branch", "--format=%(refname:short)"]) {
            for line in output.lines() {
                let branch = line.trim();
                if !branch.is_empty() {
                    branches.push((branch.to_string(), false));
                }
            }
        }

        // Remote branches (if requested)
        if include_remote {
            if let Some(output) = git_command(&["branch", "-r", "--format=%(refname:short)"]) {
                for line in output.lines() {
                    let branch = line.trim();
                    if !branch.is_empty() && !branch.contains("HEAD") {
                        branches.push((branch.to_string(), true));
                    }
                }
            }
        }

        branches
    }
}

impl Default for GitBranchProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for GitBranchProvider {
    fn name(&self) -> &str {
        "git_branch"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if !in_git_repo() {
            return false;
        }

        // git checkout <branch>
        // git merge <branch>
        // git rebase <branch>
        // git branch -d <branch>
        // git switch <branch>
        // git diff <branch>
        let branch_commands = [
            "checkout", "merge", "rebase", "switch", "diff", "cherry-pick", "reset",
        ];

        if cmd == "git" && arg_position >= 1 {
            let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");
            if branch_commands.contains(&subcommand) {
                return true;
            }
            // git branch -d <branch>
            if subcommand == "branch" && context.args.iter().any(|a| a == "-d" || a == "-D") {
                return true;
            }
        }

        false
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let current = current_branch();
        let branches = self.get_branches(true);
        let partial_lower = partial.to_lowercase();

        branches
            .into_iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&partial_lower))
            .map(|(name, is_remote)| {
                let is_current = current.as_ref().map(|c| c == &name).unwrap_or(false);
                let score = if is_current {
                    -10 // Lower score for current branch
                } else if is_remote {
                    50
                } else {
                    100 // Prefer local branches
                };

                let desc = if is_current {
                    "current branch".to_string()
                } else if is_remote {
                    "remote branch".to_string()
                } else {
                    "local branch".to_string()
                };

                ProviderSuggestion::new(&name)
                    .with_description(desc)
                    .with_category(if is_remote { "remote" } else { "local" })
                    .with_score(score)
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    fn priority(&self) -> i32 {
        100
    }
}

// ============================================================================
// Git Remote Provider
// ============================================================================

/// Provides remote name completions for git push/pull/fetch
pub struct GitRemoteProvider;

impl GitRemoteProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_remotes(&self) -> Vec<(String, String)> {
        let mut remotes = Vec::new();

        if let Some(output) = git_command(&["remote", "-v"]) {
            let mut seen = std::collections::HashSet::new();
            for line in output.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[0];
                    let url = parts[1];
                    if seen.insert(name.to_string()) {
                        remotes.push((name.to_string(), url.to_string()));
                    }
                }
            }
        }

        remotes
    }
}

impl Default for GitRemoteProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for GitRemoteProvider {
    fn name(&self) -> &str {
        "git_remote"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if !in_git_repo() {
            return false;
        }

        // git push <remote>
        // git pull <remote>
        // git fetch <remote>
        // git remote remove <remote>
        let remote_commands = ["push", "pull", "fetch"];

        if cmd == "git" && arg_position >= 1 {
            let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");

            // For push/pull/fetch, remote is the first arg after subcommand
            if remote_commands.contains(&subcommand) && context.args.len() == 1 {
                return true;
            }

            // git remote remove <remote>
            if subcommand == "remote"
                && context.args.get(1).map(|s| s.as_str()) == Some("remove")
            {
                return true;
            }
        }

        false
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let remotes = self.get_remotes();
        let partial_lower = partial.to_lowercase();

        remotes
            .into_iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&partial_lower))
            .map(|(name, url)| {
                // Give origin higher priority
                let score = if name == "origin" { 100 } else { 50 };

                ProviderSuggestion::new(&name)
                    .with_description(url)
                    .with_category("remote")
                    .with_score(score)
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }

    fn priority(&self) -> i32 {
        90
    }
}

// ============================================================================
// Git Tag Provider
// ============================================================================

/// Provides tag name completions
pub struct GitTagProvider;

impl GitTagProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_tags(&self) -> Vec<String> {
        git_command(&["tag", "-l"])
            .map(|output| {
                output
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for GitTagProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for GitTagProvider {
    fn name(&self) -> &str {
        "git_tag"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if !in_git_repo() {
            return false;
        }

        if cmd == "git" && arg_position >= 1 {
            let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");

            // git checkout <tag> (also handled by branch provider, but tags are valid)
            // git tag -d <tag>
            if subcommand == "tag" && context.args.iter().any(|a| a == "-d") {
                return true;
            }
        }

        false
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let tags = self.get_tags();
        let partial_lower = partial.to_lowercase();

        tags.into_iter()
            .filter(|tag| tag.to_lowercase().starts_with(&partial_lower))
            .map(|tag| {
                ProviderSuggestion::new(&tag)
                    .with_description("tag")
                    .with_category("tag")
                    .with_score(80)
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }

    fn priority(&self) -> i32 {
        80
    }
}

// ============================================================================
// Git Stash Provider
// ============================================================================

/// Provides stash completions for git stash apply/pop/drop
pub struct GitStashProvider;

impl GitStashProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_stashes(&self) -> Vec<(String, String)> {
        git_command(&["stash", "list"])
            .map(|output| {
                output
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.splitn(2, ':').collect();
                        if parts.len() >= 2 {
                            Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for GitStashProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for GitStashProvider {
    fn name(&self) -> &str {
        "git_stash"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if !in_git_repo() {
            return false;
        }

        if cmd == "git" && arg_position >= 2 {
            let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");
            let stash_subcommand = context.args.get(1).map(|s| s.as_str()).unwrap_or("");

            // git stash apply|pop|drop|show <stash>
            if subcommand == "stash"
                && ["apply", "pop", "drop", "show"].contains(&stash_subcommand)
            {
                return true;
            }
        }

        false
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let stashes = self.get_stashes();
        let partial_lower = partial.to_lowercase();

        stashes
            .into_iter()
            .enumerate()
            .filter(|(_, (name, _))| name.to_lowercase().starts_with(&partial_lower))
            .map(|(idx, (name, desc))| {
                ProviderSuggestion::new(&name)
                    .with_description(desc)
                    .with_category("stash")
                    .with_score(100 - idx as i64) // Newer stashes first
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(2))
    }

    fn priority(&self) -> i32 {
        85
    }
}

// ============================================================================
// Git File Provider
// ============================================================================

/// Provides changed file completions for git add/restore/diff
pub struct GitFileProvider;

impl GitFileProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_changed_files(&self) -> Vec<(String, String)> {
        let mut files = Vec::new();

        // Untracked files
        if let Some(output) = git_command(&["ls-files", "--others", "--exclude-standard"]) {
            for line in output.lines() {
                let file = line.trim();
                if !file.is_empty() {
                    files.push((file.to_string(), "untracked".to_string()));
                }
            }
        }

        // Modified files
        if let Some(output) = git_command(&["diff", "--name-only"]) {
            for line in output.lines() {
                let file = line.trim();
                if !file.is_empty() {
                    files.push((file.to_string(), "modified".to_string()));
                }
            }
        }

        // Staged files (for restore --staged)
        if let Some(output) = git_command(&["diff", "--name-only", "--cached"]) {
            for line in output.lines() {
                let file = line.trim();
                if !file.is_empty() {
                    files.push((file.to_string(), "staged".to_string()));
                }
            }
        }

        files
    }
}

impl Default for GitFileProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for GitFileProvider {
    fn name(&self) -> &str {
        "git_file"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if !in_git_repo() {
            return false;
        }

        if cmd == "git" && arg_position >= 1 {
            let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");

            // git add <file>
            // git restore <file>
            // git diff <file>
            // git checkout -- <file>
            if ["add", "restore", "diff"].contains(&subcommand) {
                return true;
            }

            // git checkout -- <file>
            if subcommand == "checkout" && context.args.iter().any(|a| a == "--") {
                return true;
            }
        }

        false
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let files = self.get_changed_files();
        let partial_lower = partial.to_lowercase();

        files
            .into_iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&partial_lower))
            .map(|(name, status)| {
                let score = match status.as_str() {
                    "modified" => 100,
                    "staged" => 90,
                    "untracked" => 80,
                    _ => 50,
                };

                ProviderSuggestion::new(&name)
                    .with_description(status)
                    .with_category("file")
                    .with_score(score)
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(2))
    }

    fn priority(&self) -> i32 {
        70 // Lower than branch provider so branches show first for checkout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_branch_provider_matches() {
        let provider = GitBranchProvider::new();
        let ctx = ProviderContext::new(
            std::path::PathBuf::from("."),
            "git",
            vec!["checkout".to_string()],
            "",
        );

        // Only matches if in git repo
        if in_git_repo() {
            assert!(provider.matches("git", 1, &ctx));
        }
    }

    #[test]
    fn test_git_remote_provider_matches() {
        let provider = GitRemoteProvider::new();
        let ctx = ProviderContext::new(
            std::path::PathBuf::from("."),
            "git",
            vec!["push".to_string()],
            "",
        );

        // Only matches if in git repo
        if in_git_repo() {
            assert!(provider.matches("git", 1, &ctx));
        }
    }
}
