//! Session Context Tracking
//!
//! Maintains session history including commands, outputs, and context
//! for improved AI suggestions and error analysis.

#![allow(dead_code)]

use crate::active_ai::CommandResult;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Maximum number of commands to keep in session history
const DEFAULT_HISTORY_SIZE: usize = 50;

/// Maximum total size of captured output (in bytes)
const MAX_OUTPUT_SIZE: usize = 100_000; // 100KB

/// Session context entry
#[derive(Debug, Clone)]
pub struct SessionEntry {
    /// The command that was executed
    pub command: String,
    /// Exit code
    pub exit_code: Option<i32>,
    /// Captured stdout (truncated if too large)
    pub stdout: Option<String>,
    /// Captured stderr (truncated if too large)
    pub stderr: Option<String>,
    /// Working directory at time of execution
    pub cwd: String,
    /// Git branch at time of execution
    pub git_branch: Option<String>,
    /// Execution duration
    pub duration: Option<Duration>,
    /// Timestamp
    pub timestamp: Instant,
}

impl SessionEntry {
    pub fn new(result: &CommandResult, duration: Option<Duration>) -> Self {
        Self {
            command: result.command.clone(),
            exit_code: result.exit_code,
            stdout: result.stdout.clone().map(|s| truncate_output(&s, 5000)),
            stderr: result.stderr.clone().map(|s| truncate_output(&s, 2000)),
            cwd: result.cwd.clone(),
            git_branch: get_git_branch(),
            duration,
            timestamp: Instant::now(),
        }
    }

    /// Check if command was successful
    pub fn is_success(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Get a summary of this entry for AI context
    pub fn to_context_string(&self) -> String {
        let status = if self.is_success() { "✓" } else { "✗" };
        let code = self.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "?".to_string());

        let mut s = format!("[{}] {} (exit: {})", status, self.command, code);

        if let Some(ref branch) = self.git_branch {
            s.push_str(&format!(" [git: {}]", branch));
        }

        if let Some(ref stderr) = self.stderr {
            if !stderr.is_empty() && stderr.len() < 200 {
                s.push_str(&format!("\n  stderr: {}", stderr.trim()));
            }
        }

        s
    }
}

/// Truncate output to fit within size limit
fn truncate_output(output: &str, max_len: usize) -> String {
    if output.len() <= max_len {
        output.to_string()
    } else {
        format!("{}...(truncated {} bytes)", &output[..max_len], output.len() - max_len)
    }
}

/// Get current git branch
fn get_git_branch() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            } else {
                None
            }
        })
}

/// Session context manager
pub struct SessionContext {
    /// Command history with results
    history: VecDeque<SessionEntry>,
    /// Maximum history size
    max_size: usize,
    /// Session start time
    start_time: Instant,
    /// Total commands executed
    total_commands: usize,
    /// Failed commands count
    failed_commands: usize,
    /// Last error (for quick access)
    last_error: Option<SessionEntry>,
}

impl SessionContext {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(DEFAULT_HISTORY_SIZE),
            max_size: DEFAULT_HISTORY_SIZE,
            start_time: Instant::now(),
            total_commands: 0,
            failed_commands: 0,
            last_error: None,
        }
    }

    /// Record a command execution
    pub fn record(&mut self, result: &CommandResult, duration: Option<Duration>) {
        let entry = SessionEntry::new(result, duration);

        self.total_commands += 1;
        if !entry.is_success() {
            self.failed_commands += 1;
            self.last_error = Some(entry.clone());
        }

        // Add to history, removing oldest if full
        if self.history.len() >= self.max_size {
            self.history.pop_front();
        }
        self.history.push_back(entry);

        // Trim output if total size exceeds limit
        self.trim_output_size();
    }

    /// Get the last command result
    pub fn last(&self) -> Option<&SessionEntry> {
        self.history.back()
    }

    /// Get the last failed command
    pub fn last_error(&self) -> Option<&SessionEntry> {
        self.last_error.as_ref()
    }

    /// Get recent commands (last N)
    pub fn recent(&self, n: usize) -> Vec<&SessionEntry> {
        self.history.iter().rev().take(n).collect()
    }

    /// Get recent failed commands
    pub fn recent_errors(&self, n: usize) -> Vec<&SessionEntry> {
        self.history
            .iter()
            .rev()
            .filter(|e| !e.is_success())
            .take(n)
            .collect()
    }

    /// Get context string for AI prompts
    pub fn get_ai_context(&self, max_entries: usize) -> String {
        let entries: Vec<_> = self.recent(max_entries);
        if entries.is_empty() {
            return String::new();
        }

        let mut context = String::from("Recent command history:\n");
        for entry in entries.iter().rev() {
            context.push_str(&entry.to_context_string());
            context.push('\n');
        }
        context
    }

    /// Get session statistics
    pub fn stats(&self) -> SessionStats {
        SessionStats {
            duration: self.start_time.elapsed(),
            total_commands: self.total_commands,
            failed_commands: self.failed_commands,
            success_rate: if self.total_commands > 0 {
                (self.total_commands - self.failed_commands) as f64 / self.total_commands as f64
            } else {
                1.0
            },
        }
    }

    /// Clear session history
    pub fn clear(&mut self) {
        self.history.clear();
        self.last_error = None;
        // Keep stats
    }

    /// Trim output to stay within memory limits
    fn trim_output_size(&mut self) {
        let mut total_size: usize = self.history.iter()
            .map(|e| {
                e.stdout.as_ref().map(|s| s.len()).unwrap_or(0) +
                e.stderr.as_ref().map(|s| s.len()).unwrap_or(0)
            })
            .sum();

        // Remove output from oldest entries if over limit
        if total_size > MAX_OUTPUT_SIZE {
            for entry in self.history.iter_mut() {
                if total_size <= MAX_OUTPUT_SIZE {
                    break;
                }
                if let Some(ref stdout) = entry.stdout {
                    total_size -= stdout.len();
                    entry.stdout = None;
                }
                if let Some(ref stderr) = entry.stderr {
                    total_size -= stderr.len();
                    entry.stderr = None;
                }
            }
        }
    }
}

impl Default for SessionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    /// Session duration
    pub duration: Duration,
    /// Total commands executed
    pub total_commands: usize,
    /// Failed commands
    pub failed_commands: usize,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
}

impl SessionStats {
    /// Format duration as human-readable string
    pub fn format_duration(&self) -> String {
        let secs = self.duration.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }
}


/// Next command predictor using session context
pub struct NextCommandPredictor {
    /// Common command sequences (bigrams)
    sequences: Vec<(String, String, f64)>, // (prev, next, weight)
}

impl NextCommandPredictor {
    pub fn new() -> Self {
        Self {
            sequences: Self::default_sequences(),
        }
    }

    /// Get default command sequences
    fn default_sequences() -> Vec<(String, String, f64)> {
        vec![
            // Git workflows
            ("git status".into(), "git add".into(), 0.8),
            ("git add".into(), "git commit".into(), 0.9),
            ("git commit".into(), "git push".into(), 0.7),
            ("git pull".into(), "git status".into(), 0.6),
            ("git checkout".into(), "git pull".into(), 0.5),
            ("git fetch".into(), "git merge".into(), 0.6),
            ("git stash".into(), "git pull".into(), 0.7),

            // Cargo workflows
            ("cargo build".into(), "cargo run".into(), 0.6),
            ("cargo build".into(), "cargo test".into(), 0.5),
            ("cargo test".into(), "cargo build --release".into(), 0.4),
            ("cargo check".into(), "cargo build".into(), 0.7),
            ("cargo fmt".into(), "cargo clippy".into(), 0.6),
            ("cargo clippy".into(), "cargo test".into(), 0.5),

            // npm workflows
            ("npm install".into(), "npm run".into(), 0.5),
            ("npm test".into(), "npm run build".into(), 0.4),
            ("npm run build".into(), "npm start".into(), 0.5),

            // Docker workflows
            ("docker build".into(), "docker run".into(), 0.8),
            ("docker ps".into(), "docker logs".into(), 0.5),
            ("docker-compose up".into(), "docker-compose logs".into(), 0.4),

            // Directory navigation
            ("ls".into(), "cd".into(), 0.4),
            ("cd".into(), "ls".into(), 0.6),
            ("mkdir".into(), "cd".into(), 0.7),

            // Error recovery
            // After failed command, often retry with sudo or fix
        ]
    }

    /// Predict next command based on last command
    pub fn predict(&self, last_command: &str, session: &SessionContext) -> Option<(String, f64)> {
        // Normalize the last command (remove arguments for matching)
        let normalized = Self::normalize_command(last_command);

        // Find best matching sequence
        let mut best: Option<(String, f64)> = None;

        for (prev, next, weight) in &self.sequences {
            if normalized.starts_with(prev) || prev.starts_with(&normalized) {
                let confidence = *weight;
                if best.is_none() || confidence > best.as_ref().unwrap().1 {
                    best = Some((next.clone(), confidence));
                }
            }
        }

        // Boost confidence if we have session history showing this pattern
        if let Some((ref cmd, confidence)) = best {
            let recent = session.recent(10);
            let pattern_count = recent.windows(2)
                .filter(|w| {
                    Self::normalize_command(&w[1].command).starts_with(&normalized) &&
                    Self::normalize_command(&w[0].command).starts_with(cmd)
                })
                .count();

            if pattern_count > 0 {
                let boosted = (confidence + 0.1 * pattern_count as f64).min(0.95);
                return Some((cmd.clone(), boosted));
            }
        }

        best
    }

    /// Predict next command after an error
    pub fn predict_after_error(&self, result: &CommandResult) -> Option<(String, f64)> {
        let cmd = &result.command;
        let stderr = result.stderr.as_deref().unwrap_or("");

        // Permission denied -> suggest sudo
        if stderr.contains("Permission denied") && !cmd.starts_with("sudo") {
            return Some((format!("sudo {}", cmd), 0.8));
        }

        // File not found -> suggest creating or checking path
        if stderr.contains("No such file or directory") {
            if cmd.starts_with("cd ") {
                let path = cmd.strip_prefix("cd ").unwrap_or("");
                return Some((format!("mkdir -p {}", path), 0.6));
            }
        }

        // Git not a repository -> suggest git init
        if stderr.contains("not a git repository") {
            return Some(("git init".to_string(), 0.7));
        }

        // npm not found package.json
        if stderr.contains("package.json") && cmd.starts_with("npm") {
            return Some(("npm init -y".to_string(), 0.6));
        }

        None
    }

    /// Normalize command for matching (extract base command)
    fn normalize_command(cmd: &str) -> String {
        // Get first two words for better matching
        let words: Vec<&str> = cmd.split_whitespace().take(2).collect();
        words.join(" ")
    }
}

impl Default for NextCommandPredictor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_context() {
        let mut ctx = SessionContext::new();
        let result = CommandResult::new("ls -la", Some(0), Some("file1\nfile2".to_string()), None);
        ctx.record(&result, Some(Duration::from_millis(100)));

        assert_eq!(ctx.total_commands, 1);
        assert_eq!(ctx.failed_commands, 0);
        assert!(ctx.last().is_some());
    }

    #[test]
    fn test_session_error_tracking() {
        let mut ctx = SessionContext::new();
        let error = CommandResult::new("invalid", Some(127), None, Some("not found".to_string()));
        ctx.record(&error, None);

        assert_eq!(ctx.failed_commands, 1);
        assert!(ctx.last_error().is_some());
    }

    #[test]
    fn test_next_command_predictor() {
        let predictor = NextCommandPredictor::new();
        let ctx = SessionContext::new();

        let prediction = predictor.predict("git add .", &ctx);
        assert!(prediction.is_some());
        assert!(prediction.unwrap().0.contains("commit"));
    }

    #[test]
    fn test_error_recovery_prediction() {
        let predictor = NextCommandPredictor::new();
        let result = CommandResult::new(
            "cat /etc/shadow",
            Some(1),
            None,
            Some("Permission denied".to_string()),
        );

        let prediction = predictor.predict_after_error(&result);
        assert!(prediction.is_some());
        assert!(prediction.unwrap().0.starts_with("sudo"));
    }
}
