//! Error types for Smart Command
//!
//! Using thiserror for ergonomic error handling

use std::path::PathBuf;
use thiserror::Error;

/// Application-level errors
#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Command definition error in {path}: {message}")]
    DefinitionParse { path: PathBuf, message: String },

    #[error("Directory not found: {0}")]
    DirectoryNotFound(PathBuf),

    #[error("Command execution failed: {0}")]
    CommandExecution(String),

    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    #[error("History error: {0}")]
    History(String),

    #[error("Shell completion generation failed: {0}")]
    CompletionGeneration(String),
}

/// Command execution errors
#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Command not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Command timed out after {0} seconds")]
    Timeout(u64),

    #[error("Command was interrupted")]
    Interrupted,

    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),

    #[error("Working directory error: {0}")]
    WorkingDir(String),
}

/// Result type alias for AppError
#[allow(dead_code)]
pub type AppResult<T> = Result<T, AppError>;

/// Result type alias for CommandError
#[allow(dead_code)]
pub type CmdResult<T> = Result<T, CommandError>;

#[allow(dead_code)]
impl AppError {
    /// Create a definition parse error
    pub fn definition_parse(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::DefinitionParse {
            path: path.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AppError::DirectoryNotFound(PathBuf::from("/test/path"));
        assert!(err.to_string().contains("/test/path"));

        let err = CommandError::NotFound("foo".to_string());
        assert!(err.to_string().contains("foo"));
    }
}
