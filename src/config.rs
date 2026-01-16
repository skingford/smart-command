//! Configuration management with environment and file support
//!
//! Configuration sources (in order of priority):
//! 1. Environment variables (SMART_CMD_*)
//! 2. Config file (~/.config/smart-command/config.toml)
//! 3. Default values

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    /// Display language (en, zh)
    #[serde(default = "default_lang")]
    pub lang: String,

    /// History file path
    #[serde(default = "default_history_path")]
    pub history_path: PathBuf,

    /// Maximum history entries
    #[serde(default = "default_history_size")]
    pub history_size: usize,

    /// Enable dangerous command protection
    #[serde(default = "default_danger_protection")]
    pub danger_protection: bool,

    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Custom definitions directory
    #[serde(default)]
    pub definitions_dir: Option<PathBuf>,

    /// Shell prompt format
    #[serde(default)]
    pub prompt: PromptConfig,
}

/// Prompt configuration
#[derive(Debug, Deserialize, Clone)]
pub struct PromptConfig {
    /// Show git branch in prompt
    #[serde(default = "default_true")]
    pub show_git_branch: bool,

    /// Show current directory
    #[serde(default = "default_true")]
    pub show_cwd: bool,

    /// Prompt indicator character
    #[serde(default = "default_prompt_char")]
    pub indicator: String,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            show_git_branch: true,
            show_cwd: true,
            indicator: default_prompt_char(),
        }
    }
}

fn default_lang() -> String {
    std::env::var("LANG")
        .ok()
        .and_then(|l| {
            if l.starts_with("zh") {
                Some("zh".to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "en".to_string())
}

fn default_history_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".smart_command_history"))
        .unwrap_or_else(|| PathBuf::from(".smart_command_history"))
}

fn default_history_size() -> usize {
    1000
}

fn default_danger_protection() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_true() -> bool {
    true
}

fn default_prompt_char() -> String {
    "❯".to_string()
}

impl AppConfig {
    /// Load configuration from all sources
    pub fn load() -> Result<Self, ConfigError> {
        let config_dir = dirs::config_dir()
            .map(|d| d.join("smart-command"))
            .unwrap_or_else(|| PathBuf::from("."));

        let config_file = config_dir.join("config.toml");

        let builder = Config::builder()
            // Default values
            .set_default("lang", default_lang())?
            .set_default(
                "history_path",
                default_history_path().to_string_lossy().to_string(),
            )?
            .set_default("history_size", default_history_size() as i64)?
            .set_default("danger_protection", default_danger_protection())?
            .set_default("log_level", default_log_level())?
            .set_default("prompt.show_git_branch", true)?
            .set_default("prompt.show_cwd", true)?
            .set_default("prompt.indicator", default_prompt_char())?;

        // Add config file if it exists
        let builder = if config_file.exists() {
            builder.add_source(File::from(config_file))
        } else {
            builder
        };

        // Add environment variables (SMART_CMD_*)
        let builder = builder.add_source(
            Environment::with_prefix("SMART_CMD")
                .separator("_")
                .try_parsing(true),
        );

        builder.build()?.try_deserialize()
    }

    /// Create default configuration
    pub fn default_config() -> Self {
        Self {
            lang: default_lang(),
            history_path: default_history_path(),
            history_size: default_history_size(),
            danger_protection: default_danger_protection(),
            log_level: default_log_level(),
            definitions_dir: None,
            prompt: PromptConfig::default(),
        }
    }

    /// Get config file path for display
    pub fn config_file_path() -> PathBuf {
        dirs::config_dir()
            .map(|d| d.join("smart-command").join("config.toml"))
            .unwrap_or_else(|| PathBuf::from("./config.toml"))
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::default_config()
    }
}

/// Generate example configuration file content
pub fn generate_example_config() -> String {
    r#"# Smart Command Configuration
# Place this file at ~/.config/smart-command/config.toml

# Display language: "en" or "zh"
lang = "en"

# History file path (default: ~/.smart_command_history)
# history_path = "/path/to/history"

# Maximum history entries
history_size = 1000

# Enable dangerous command protection (prompts before rm -rf, etc.)
danger_protection = true

# Log level: trace, debug, info, warn, error
log_level = "info"

# Custom definitions directory (optional)
# definitions_dir = "/path/to/definitions"

[prompt]
# Show git branch in prompt
show_git_branch = true

# Show current working directory
show_cwd = true

# Prompt indicator character
indicator = "❯"
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default_config();
        assert!(!config.lang.is_empty());
        assert!(config.history_size > 0);
        assert!(config.danger_protection);
    }

    #[test]
    fn test_generate_example() {
        let example = generate_example_config();
        assert!(example.contains("lang"));
        assert!(example.contains("danger_protection"));
    }
}
