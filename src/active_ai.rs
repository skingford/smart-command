//! Active AI - Proactive Error Assistance
//!
//! Provides intelligent error detection and proactive AI suggestions
//! after command failures, similar to Warp's Active AI feature.

#![allow(dead_code)]

use crate::ai_stream::StreamingAiGenerator;
use crate::config::{ActiveAiConfig, AiConfig};
use crate::output::Output;
use std::process::Output as ProcessOutput;

/// Result of command execution with captured output
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// The command that was executed
    pub command: String,
    /// Exit code (None if terminated by signal)
    pub exit_code: Option<i32>,
    /// Standard output (captured if available)
    pub stdout: Option<String>,
    /// Standard error (captured if available)
    pub stderr: Option<String>,
    /// Working directory at time of execution
    pub cwd: String,
    /// Whether the command succeeded
    pub success: bool,
}

impl CommandResult {
    /// Create a new command result
    pub fn new(command: &str, exit_code: Option<i32>, stdout: Option<String>, stderr: Option<String>) -> Self {
        let success = exit_code == Some(0);
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        Self {
            command: command.to_string(),
            exit_code,
            stdout,
            stderr,
            cwd,
            success,
        }
    }

    /// Create from process output
    pub fn from_process_output(command: &str, output: &ProcessOutput) -> Self {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Self::new(
            command,
            output.status.code(),
            if stdout.is_empty() { None } else { Some(stdout) },
            if stderr.is_empty() { None } else { Some(stderr) },
        )
    }

    /// Check if this is an error that Active AI can help with
    pub fn is_ai_helpable_error(&self) -> bool {
        if self.success {
            return false;
        }

        // Check if we have error information to analyze
        if self.stderr.is_some() || self.exit_code.is_some() {
            return true;
        }

        false
    }

    /// Get error context for AI prompt
    pub fn get_error_context(&self) -> String {
        let mut context = String::new();

        context.push_str(&format!("Command: {}\n", self.command));
        context.push_str(&format!("Working directory: {}\n", self.cwd));

        if let Some(code) = self.exit_code {
            context.push_str(&format!("Exit code: {}\n", code));
        }

        if let Some(ref stderr) = self.stderr {
            let truncated = if stderr.len() > 2000 {
                format!("{}...(truncated)", &stderr[..2000])
            } else {
                stderr.clone()
            };
            context.push_str(&format!("\nError output:\n{}\n", truncated));
        }

        if let Some(ref stdout) = self.stdout {
            let truncated = if stdout.len() > 1000 {
                format!("{}...(truncated)", &stdout[..1000])
            } else {
                stdout.clone()
            };
            context.push_str(&format!("\nStandard output:\n{}\n", truncated));
        }

        context
    }
}


/// Active AI handler for proactive error assistance
pub struct ActiveAi {
    config: ActiveAiConfig,
}

impl ActiveAi {
    pub fn new(config: ActiveAiConfig) -> Self {
        Self { config }
    }

    /// Check if Active AI should handle this error
    pub fn should_handle(&self, result: &CommandResult) -> bool {
        if !self.config.enabled || !self.config.show_on_error {
            return false;
        }

        if result.success {
            return false;
        }

        // Check minimum exit code
        if let Some(code) = result.exit_code {
            if code.abs() < self.config.min_exit_code {
                return false;
            }
        }

        // Check ignore list
        if self.config.should_ignore(&result.command) {
            return false;
        }

        true
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Show proactive suggestion prompt after error
    pub fn show_error_prompt(&self, result: &CommandResult) {
        if !self.should_handle(result) {
            return;
        }

        Output::active_ai_hint();
    }

    /// Handle user input for Active AI actions
    /// Returns: Some(action) if user wants AI help, None otherwise
    pub fn handle_input(&self, input: &str, result: &CommandResult) -> Option<ActiveAiAction> {
        let trimmed = input.trim().to_lowercase();

        match trimmed.as_str() {
            "e" | "explain" => Some(ActiveAiAction::Explain(result.clone())),
            "f" | "fix" => Some(ActiveAiAction::Fix(result.clone())),
            "r" | "retry" => Some(ActiveAiAction::Retry(result.command.clone())),
            _ => None,
        }
    }

    /// Generate AI explanation for an error
    pub fn explain_error(&self, result: &CommandResult, ai_config: &AiConfig) -> Result<String, String> {
        let prompt = format!(
            "Explain this command error in simple terms. Be concise (2-3 sentences max).\n\n{}",
            result.get_error_context()
        );

        self.query_ai(&prompt, ai_config)
    }

    /// Generate AI fix suggestion for an error
    pub fn suggest_fix(&self, result: &CommandResult, ai_config: &AiConfig) -> Result<String, String> {
        let prompt = format!(
            "Suggest a fix for this command error. Provide ONLY the corrected command, no explanation.\n\n{}",
            result.get_error_context()
        );

        self.query_ai(&prompt, ai_config)
    }

    /// Query AI with a prompt
    fn query_ai(&self, prompt: &str, ai_config: &AiConfig) -> Result<String, String> {
        let generator = StreamingAiGenerator::new(ai_config);
        let context = crate::ai::llm::AiContext::default();

        match generator.generate_streaming(prompt, &context, None) {
            Ok(response) => Ok(response),
            Err(e) => Err(format!("AI error: {}", e)),
        }
    }
}

/// Actions that Active AI can perform
#[derive(Debug, Clone)]
pub enum ActiveAiAction {
    /// Explain the error
    Explain(CommandResult),
    /// Suggest a fix command
    Fix(CommandResult),
    /// Retry the original command
    Retry(String),
}

/// Common error patterns for quick detection
pub struct ErrorPatterns;

impl ErrorPatterns {
    /// Detect error type from exit code and stderr
    pub fn detect_error_type(result: &CommandResult) -> Option<ErrorType> {
        let stderr = result.stderr.as_deref().unwrap_or("");
        let code = result.exit_code;

        // Command not found
        if code == Some(127) || stderr.contains("command not found") || stderr.contains("not found") {
            return Some(ErrorType::CommandNotFound);
        }

        // Permission denied
        if stderr.contains("Permission denied") || stderr.contains("permission denied") || code == Some(126) {
            return Some(ErrorType::PermissionDenied);
        }

        // File/directory not found
        if stderr.contains("No such file or directory") || stderr.contains("cannot find") {
            return Some(ErrorType::FileNotFound);
        }

        // Syntax error
        if stderr.contains("syntax error") || stderr.contains("unexpected token") {
            return Some(ErrorType::SyntaxError);
        }

        // Git errors
        if stderr.contains("fatal:") && (stderr.contains("git") || result.command.starts_with("git")) {
            return Some(ErrorType::GitError);
        }

        // Package manager errors
        if stderr.contains("npm ERR!") || stderr.contains("cargo error") || stderr.contains("pip error") {
            return Some(ErrorType::PackageError);
        }

        // Network errors
        if stderr.contains("Could not resolve host") || stderr.contains("Connection refused")
           || stderr.contains("Network is unreachable") {
            return Some(ErrorType::NetworkError);
        }

        // Build/compile errors
        if stderr.contains("error[E") || stderr.contains("error:") || stderr.contains("compilation failed") {
            return Some(ErrorType::BuildError);
        }

        None
    }

    /// Get quick hint for common error types (no AI needed)
    pub fn get_quick_hint(error_type: &ErrorType, result: &CommandResult) -> Option<String> {
        match error_type {
            ErrorType::CommandNotFound => {
                let cmd = result.command.split_whitespace().next()?;
                Some(format!("Command '{}' not found. Try: which {} or brew install {}", cmd, cmd, cmd))
            }
            ErrorType::PermissionDenied => {
                Some("Permission denied. Try: sudo or check file permissions with ls -la".to_string())
            }
            ErrorType::FileNotFound => {
                Some("File not found. Check the path with ls or find.".to_string())
            }
            ErrorType::SyntaxError => {
                Some("Syntax error in command. Check quotes, brackets, and special characters.".to_string())
            }
            _ => None,
        }
    }
}

/// Types of errors that can be detected
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorType {
    CommandNotFound,
    PermissionDenied,
    FileNotFound,
    SyntaxError,
    GitError,
    PackageError,
    NetworkError,
    BuildError,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_result_success() {
        let result = CommandResult::new("ls", Some(0), None, None);
        assert!(result.success);
        assert!(!result.is_ai_helpable_error());
    }

    #[test]
    fn test_command_result_error() {
        let result = CommandResult::new("invalid_cmd", Some(127), None, Some("command not found".to_string()));
        assert!(!result.success);
        assert!(result.is_ai_helpable_error());
    }

    #[test]
    fn test_error_detection() {
        let result = CommandResult::new("foo", Some(127), None, Some("foo: command not found".to_string()));
        assert_eq!(ErrorPatterns::detect_error_type(&result), Some(ErrorType::CommandNotFound));

        let result = CommandResult::new("cat /etc/shadow", Some(1), None, Some("Permission denied".to_string()));
        assert_eq!(ErrorPatterns::detect_error_type(&result), Some(ErrorType::PermissionDenied));
    }

    #[test]
    fn test_ignore_commands() {
        let config = ActiveAiConfig::default();
        assert!(config.should_ignore("grep pattern file"));
        assert!(config.should_ignore("diff file1 file2"));
        assert!(!config.should_ignore("cargo build"));
    }
}
