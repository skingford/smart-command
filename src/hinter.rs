//! Smart Hinter for Inline Suggestions
//!
//! Provides fish-style inline suggestions based on command history,
//! AI predictions, and contextual awareness.

#![allow(dead_code)]

use crate::ai::CommandPredictor;
use crate::context::tracker;
use nu_ansi_term::{Color, Style};
use reedline::{Hinter, History, SearchQuery};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Get the first token from a string (for incremental hint completion)
fn get_first_token(string: &str) -> String {
    // Skip leading whitespace and return the first word
    let trimmed = string.trim_start();
    trimmed
        .split_whitespace()
        .next()
        .map(|s| {
            // Include the original leading whitespace
            let whitespace_len = string.len() - string.trim_start().len();
            let whitespace: String = string.chars().take(whitespace_len).collect();
            format!("{}{}", whitespace, s)
        })
        .unwrap_or_default()
}

/// Smart hinter that combines history, AI predictions, and context
pub struct SmartHinter {
    /// Style for the hint text
    style: Style,
    /// Current hint text (unformatted)
    current_hint: String,
    /// Current working directory for context
    cwd: PathBuf,
    /// Command predictor for AI-based suggestions
    predictor: Arc<RwLock<CommandPredictor>>,
    /// Minimum input length before showing hints
    min_chars: usize,
    /// Last executed command (for bigram predictions)
    last_command: Arc<RwLock<Option<String>>>,
}

impl SmartHinter {
    pub fn new() -> Self {
        Self {
            style: Style::new().italic().fg(Color::DarkGray),
            current_hint: String::new(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            predictor: Arc::new(RwLock::new(CommandPredictor::default())),
            min_chars: 1,
            last_command: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new SmartHinter with a shared CommandPredictor
    pub fn with_predictor(predictor: Arc<RwLock<CommandPredictor>>) -> Self {
        Self {
            style: Style::new().italic().fg(Color::DarkGray),
            current_hint: String::new(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            predictor,
            min_chars: 1,
            last_command: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the style for hint text
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set minimum input length before showing hints
    pub fn with_min_chars(mut self, min_chars: usize) -> Self {
        self.min_chars = min_chars;
        self
    }

    /// Record a command execution for learning
    pub fn record_command(&self, command: &str) {
        let prev = self.last_command.read().unwrap().clone();
        self.predictor
            .write()
            .unwrap()
            .record(command, prev.as_deref());
        *self.last_command.write().unwrap() = Some(command.to_string());
    }

    /// Get hint from AI predictor
    fn get_prediction_hint(&self, line: &str) -> Option<String> {
        // Only suggest if we have a reasonable input
        if line.is_empty() || line.contains(' ') {
            return None;
        }

        let last_cmd = self.last_command.read().unwrap().clone();
        let predictor = self.predictor.read().unwrap();

        if let Some((predicted, confidence)) = predictor.most_likely(last_cmd.as_deref()) {
            // Only suggest if prediction starts with current input and confidence is high
            if confidence >= 0.3 && predicted.starts_with(line) && predicted.len() > line.len() {
                return Some(predicted[line.len()..].to_string());
            }
        }

        None
    }

    /// Get contextual hint based on project type and git state
    #[allow(dead_code)]
    fn get_contextual_hint(&self, line: &str) -> Option<String> {
        if !line.is_empty() {
            return None;
        }

        // Get contextual suggestions
        let suggestions = tracker().get_contextual_suggestions(&self.cwd);

        // Return the first suggestion if available
        suggestions.first().cloned()
    }
}

impl Default for SmartHinter {
    fn default() -> Self {
        Self::new()
    }
}

impl Hinter for SmartHinter {
    fn handle(
        &mut self,
        line: &str,
        _pos: usize,
        history: &dyn History,
        use_ansi_coloring: bool,
        _cwd: &str,
    ) -> String {
        // Update cwd
        self.cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // Build hint based on history and predictions
        self.current_hint = if line.chars().count() >= self.min_chars {
            // First try history-based hint
            let history_hint = history
                .search(SearchQuery::last_with_prefix(
                    line.to_string(),
                    history.session(),
                ))
                .ok()
                .and_then(|results| results.first().cloned())
                .and_then(|entry| {
                    entry
                        .command_line
                        .get(line.len()..)
                        .map(|s| s.to_string())
                });

            // If no history hint, try AI prediction
            history_hint.or_else(|| self.get_prediction_hint(line))
                .unwrap_or_default()
        } else {
            String::new()
        };

        if use_ansi_coloring && !self.current_hint.is_empty() {
            self.style.paint(&self.current_hint).to_string()
        } else {
            self.current_hint.clone()
        }
    }

    fn complete_hint(&self) -> String {
        self.current_hint.clone()
    }

    fn next_hint_token(&self) -> String {
        get_first_token(&self.current_hint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hinter_creation() {
        let hinter = SmartHinter::new();
        assert_eq!(hinter.min_chars, 1);
    }

    #[test]
    fn test_hinter_with_style() {
        let style = Style::new().bold().fg(Color::Blue);
        let hinter = SmartHinter::new().with_style(style);
        assert_eq!(hinter.style.foreground, Some(Color::Blue));
    }

    #[test]
    fn test_record_command() {
        let hinter = SmartHinter::new();
        hinter.record_command("git status");
        hinter.record_command("git commit");

        let last = hinter.last_command.read().unwrap().clone();
        assert_eq!(last, Some("git commit".to_string()));
    }
}
