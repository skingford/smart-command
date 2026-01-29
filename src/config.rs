//! Configuration management with environment and file support
//!
//! Configuration sources (in order of priority):
//! 1. Environment variables (SMART_CMD_*)
//! 2. Config file (~/.config/smart-command/config.toml)
//! 3. Default values

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

    /// Syntax highlighting theme (default, nord, dracula)
    #[serde(default)]
    pub theme: Option<String>,

    /// Shell prompt format
    #[serde(default)]
    pub prompt: PromptConfig,

    /// Upgrade configuration
    #[serde(default)]
    pub upgrade: UpgradeConfig,

    /// AI completion configuration
    #[serde(default)]
    pub ai: AiConfig,
}

/// Upgrade configuration
#[derive(Debug, Deserialize, Clone)]
pub struct UpgradeConfig {
    /// Enable automatic version check at startup
    #[serde(default = "default_true")]
    pub auto_check: bool,

    /// Check interval in hours (default: 24)
    #[serde(default = "default_check_interval")]
    pub check_interval_hours: u64,

    /// GitHub repository (owner/repo)
    #[serde(default = "default_repo")]
    pub repository: String,

    /// Include pre-release versions
    #[serde(default)]
    pub include_prerelease: bool,
}

impl Default for UpgradeConfig {
    fn default() -> Self {
        Self {
            auto_check: true,
            check_interval_hours: default_check_interval(),
            repository: default_repo(),
            include_prerelease: false,
        }
    }
}

/// AI provider type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Claude,
    OpenAI,
    Gemini,
    GLM,
    DeepSeek,
    Qwen,
    Ollama,
    OpenRouter,
    Custom,
}

impl Default for ProviderType {
    fn default() -> Self {
        ProviderType::Claude
    }
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Claude => write!(f, "claude"),
            ProviderType::OpenAI => write!(f, "openai"),
            ProviderType::Gemini => write!(f, "gemini"),
            ProviderType::GLM => write!(f, "glm"),
            ProviderType::DeepSeek => write!(f, "deepseek"),
            ProviderType::Qwen => write!(f, "qwen"),
            ProviderType::Ollama => write!(f, "ollama"),
            ProviderType::OpenRouter => write!(f, "openrouter"),
            ProviderType::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude" | "anthropic" => Ok(ProviderType::Claude),
            "openai" | "gpt" | "chatgpt" | "codex" => Ok(ProviderType::OpenAI),
            "gemini" | "google" => Ok(ProviderType::Gemini),
            "glm" | "zhipu" | "智谱" => Ok(ProviderType::GLM),
            "deepseek" => Ok(ProviderType::DeepSeek),
            "qwen" | "tongyi" | "通义" => Ok(ProviderType::Qwen),
            "ollama" | "local" => Ok(ProviderType::Ollama),
            "openrouter" => Ok(ProviderType::OpenRouter),
            "custom" => Ok(ProviderType::Custom),
            _ => Err(format!("Unknown provider: {}", s)),
        }
    }
}

/// Individual provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider type
    #[serde(default)]
    pub provider_type: ProviderType,

    /// API key (can reference env var like $ANTHROPIC_API_KEY)
    #[serde(default)]
    pub api_key: Option<String>,

    /// API endpoint (optional, uses default if not set)
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Model to use
    #[serde(default)]
    pub model: Option<String>,

    /// Max tokens for response (overrides global)
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Temperature for generation (overrides global)
    #[serde(default)]
    pub temperature: Option<f32>,

    /// Request timeout in seconds (overrides global)
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

impl ProviderConfig {
    /// Create a new provider config with defaults for the given type
    pub fn new(provider_type: ProviderType) -> Self {
        let (api_key_env, endpoint, model) = match provider_type {
            ProviderType::Claude => (
                Some("$ANTHROPIC_API_KEY".to_string()),
                None,
                Some("claude-sonnet-4-20250514".to_string()),
            ),
            ProviderType::OpenAI => (
                Some("$OPENAI_API_KEY".to_string()),
                Some("https://api.openai.com/v1/chat/completions".to_string()),
                Some("gpt-4o-mini".to_string()),
            ),
            ProviderType::Gemini => (
                Some("$GOOGLE_API_KEY".to_string()),
                None,
                Some("gemini-2.0-flash".to_string()),
            ),
            ProviderType::GLM => (
                Some("$ZHIPU_API_KEY".to_string()),
                Some("https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string()),
                Some("glm-4-plus".to_string()),
            ),
            ProviderType::DeepSeek => (
                Some("$DEEPSEEK_API_KEY".to_string()),
                Some("https://api.deepseek.com/v1/chat/completions".to_string()),
                Some("deepseek-chat".to_string()),
            ),
            ProviderType::Qwen => (
                Some("$DASHSCOPE_API_KEY".to_string()),
                Some("https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions".to_string()),
                Some("qwen-max".to_string()),
            ),
            ProviderType::Ollama => (
                None,
                Some("http://localhost:11434/api/chat".to_string()),
                Some("qwen2.5:7b".to_string()),
            ),
            ProviderType::OpenRouter => (
                Some("$OPENROUTER_API_KEY".to_string()),
                Some("https://openrouter.ai/api/v1/chat/completions".to_string()),
                Some("anthropic/claude-sonnet-4".to_string()),
            ),
            ProviderType::Custom => (
                Some("$CUSTOM_API_KEY".to_string()),
                None,
                None,
            ),
        };

        Self {
            provider_type,
            api_key: api_key_env,
            endpoint,
            model,
            max_tokens: None,
            temperature: None,
            timeout_secs: None,
        }
    }
}

/// Global AI settings that apply to all providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAiSettings {
    /// System prompt for command generation
    #[serde(default = "default_ai_system_prompt")]
    pub system_prompt: String,

    /// Default max tokens for response
    #[serde(default = "default_ai_max_tokens")]
    pub max_tokens: u32,

    /// Default temperature for generation
    #[serde(default = "default_ai_temperature")]
    pub temperature: f32,

    /// Default request timeout in seconds
    #[serde(default = "default_ai_timeout")]
    pub timeout_secs: u64,

    /// Enable dangerous command warning
    #[serde(default = "default_true")]
    pub warn_dangerous: bool,
}

impl Default for GlobalAiSettings {
    fn default() -> Self {
        Self {
            system_prompt: default_ai_system_prompt(),
            max_tokens: default_ai_max_tokens(),
            temperature: default_ai_temperature(),
            timeout_secs: default_ai_timeout(),
            warn_dangerous: true,
        }
    }
}

/// AI completion configuration with multi-provider support
#[derive(Debug, Deserialize, Clone)]
pub struct AiConfig {
    /// Enable AI completion (Alt+L)
    #[serde(default)]
    pub enabled: bool,

    /// Currently active provider name
    #[serde(default = "default_ai_provider")]
    pub active: String,

    /// Pre-configured providers
    #[serde(default = "default_providers")]
    pub providers: HashMap<String, ProviderConfig>,

    /// Global settings
    #[serde(default)]
    pub global: GlobalAiSettings,

    // Legacy fields for backward compatibility
    /// AI provider: claude, gemini, openai, glm, custom (deprecated, use active)
    #[serde(default)]
    pub provider: Option<String>,

    /// API key (deprecated, use providers.<name>.api_key)
    #[serde(default)]
    pub api_key: Option<String>,

    /// API endpoint (deprecated, use providers.<name>.endpoint)
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Model to use (deprecated, use providers.<name>.model)
    #[serde(default)]
    pub model: Option<String>,

    /// System prompt (deprecated, use global.system_prompt)
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Max tokens (deprecated, use global.max_tokens)
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Temperature (deprecated, use global.temperature)
    #[serde(default)]
    pub temperature: Option<f32>,

    /// Timeout (deprecated, use global.timeout_secs)
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

impl AiConfig {
    /// Get the active provider configuration
    pub fn get_active_provider(&self) -> Option<&ProviderConfig> {
        // First try the new active field
        if let Some(provider) = self.providers.get(&self.active) {
            return Some(provider);
        }

        // Fall back to legacy provider field
        if let Some(ref provider_name) = self.provider {
            return self.providers.get(provider_name);
        }

        // Return first available provider
        self.providers.values().next()
    }

    /// Get the active provider configuration (mutable)
    #[allow(dead_code)]
    pub fn get_active_provider_mut(&mut self) -> Option<&mut ProviderConfig> {
        let active = self.active.clone();
        self.providers.get_mut(&active)
    }

    /// Switch to a different provider
    pub fn switch_provider(&mut self, name: &str) -> Result<(), String> {
        if self.providers.contains_key(name) {
            self.active = name.to_string();
            Ok(())
        } else {
            Err(format!("Provider '{}' not found. Available: {:?}",
                name, self.providers.keys().collect::<Vec<_>>()))
        }
    }

    /// Add a new provider configuration
    #[allow(dead_code)]
    pub fn add_provider(&mut self, name: &str, config: ProviderConfig) {
        self.providers.insert(name.to_string(), config);
    }

    /// Remove a provider configuration
    #[allow(dead_code)]
    pub fn remove_provider(&mut self, name: &str) -> Option<ProviderConfig> {
        if name == self.active {
            return None; // Cannot remove active provider
        }
        self.providers.remove(name)
    }

    /// List all configured providers
    pub fn list_providers(&self) -> Vec<(&String, &ProviderConfig)> {
        self.providers.iter().collect()
    }

    /// Get effective settings for the active provider (merges provider + global)
    pub fn get_effective_settings(&self) -> EffectiveAiSettings {
        let provider = self.get_active_provider();

        EffectiveAiSettings {
            enabled: self.enabled,
            provider_type: provider.map(|p| p.provider_type.clone()).unwrap_or_default(),
            api_key: provider.and_then(|p| p.api_key.clone())
                .or_else(|| self.api_key.clone()),
            endpoint: provider.and_then(|p| p.endpoint.clone())
                .or_else(|| self.endpoint.clone()),
            model: provider.and_then(|p| p.model.clone())
                .or_else(|| self.model.clone()),
            system_prompt: self.system_prompt.clone()
                .unwrap_or_else(|| self.global.system_prompt.clone()),
            max_tokens: provider.and_then(|p| p.max_tokens)
                .or(self.max_tokens)
                .unwrap_or(self.global.max_tokens),
            temperature: provider.and_then(|p| p.temperature)
                .or(self.temperature)
                .unwrap_or(self.global.temperature),
            timeout_secs: provider.and_then(|p| p.timeout_secs)
                .or(self.timeout_secs)
                .unwrap_or(self.global.timeout_secs),
        }
    }
}

/// Effective AI settings after merging provider and global configs
#[derive(Debug, Clone)]
pub struct EffectiveAiSettings {
    pub enabled: bool,
    pub provider_type: ProviderType,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub model: Option<String>,
    pub system_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_secs: u64,
}

fn default_providers() -> HashMap<String, ProviderConfig> {
    let mut providers = HashMap::new();

    // Pre-configure common providers with environment variable references
    providers.insert("claude".to_string(), ProviderConfig::new(ProviderType::Claude));
    providers.insert("openai".to_string(), ProviderConfig::new(ProviderType::OpenAI));
    providers.insert("gemini".to_string(), ProviderConfig::new(ProviderType::Gemini));
    providers.insert("deepseek".to_string(), ProviderConfig::new(ProviderType::DeepSeek));
    providers.insert("glm".to_string(), ProviderConfig::new(ProviderType::GLM));
    providers.insert("qwen".to_string(), ProviderConfig::new(ProviderType::Qwen));
    providers.insert("ollama".to_string(), ProviderConfig::new(ProviderType::Ollama));
    providers.insert("openrouter".to_string(), ProviderConfig::new(ProviderType::OpenRouter));

    providers
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            active: default_ai_provider(),
            providers: default_providers(),
            global: GlobalAiSettings::default(),
            // Legacy fields
            provider: None,
            api_key: None,
            endpoint: None,
            model: None,
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            timeout_secs: None,
        }
    }
}

fn default_ai_provider() -> String {
    "openai".to_string()
}

fn default_ai_system_prompt() -> String {
    r#"You are a shell command expert. Generate a single shell command based on the user's natural language description.
Rules:
- Output ONLY the command, no explanations
- Use common Unix/Linux commands
- Prefer portable solutions
- If multiple commands needed, use && or pipes
- Never use dangerous commands without explicit request"#.to_string()
}

fn default_ai_max_tokens() -> u32 {
    256
}

fn default_ai_temperature() -> f32 {
    0.3
}

fn default_ai_timeout() -> u64 {
    30
}

fn default_check_interval() -> u64 {
    24
}

fn default_repo() -> String {
    "skingford/smart-command".to_string()
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
            theme: None,
            prompt: PromptConfig::default(),
            upgrade: UpgradeConfig::default(),
            ai: AiConfig::default(),
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

# Syntax highlighting theme: "default", "nord", or "dracula"
# theme = "dracula"

[prompt]
# Show git branch in prompt
show_git_branch = true

# Show current working directory
show_cwd = true

# Prompt indicator character
indicator = "❯"

[upgrade]
# Enable automatic version check at startup
auto_check = true

# Check interval in hours
check_interval_hours = 24

# GitHub repository for updates
repository = "skingford/smart-command"

# Include pre-release versions
include_prerelease = false

#==============================================================================
# AI Configuration
#==============================================================================
# Smart Command supports multiple AI providers for command generation.
# Use Alt+L or type "?ai <query>" to generate commands from natural language.
#
# Supported providers:
#   - claude    : Anthropic Claude (recommended)
#   - openai    : OpenAI GPT models
#   - gemini    : Google Gemini
#   - deepseek  : DeepSeek (Chinese provider, good for coding)
#   - glm       : 智谱AI GLM-4
#   - qwen      : 阿里通义千问
#   - ollama    : Local models via Ollama
#   - openrouter: OpenRouter (access multiple providers)
#   - custom    : Any OpenAI-compatible API

[ai]
# Enable AI completion
enabled = true

# Active provider name (must match a key in [ai.providers.*])
active = "openai"

#------------------------------------------------------------------------------
# Global Settings (apply to all providers unless overridden)
#------------------------------------------------------------------------------
[ai.global]
# System prompt for command generation
system_prompt = """
You are a shell command expert. Generate a single shell command based on the user's natural language description.
Rules:
- Output ONLY the command, no explanations
- Use common Unix/Linux commands
- Prefer portable solutions
- If multiple commands needed, use && or pipes
- Never use dangerous commands without explicit request
"""

# Default max tokens
max_tokens = 256

# Default temperature (0.0 = deterministic, 1.0 = creative)
temperature = 0.3

# Request timeout in seconds
timeout_secs = 30

# Warn about potentially dangerous AI-generated commands
warn_dangerous = true

#------------------------------------------------------------------------------
# Provider Configurations
#------------------------------------------------------------------------------
# Each provider can be configured independently. API keys should use
# environment variable references (e.g., $ANTHROPIC_API_KEY) for security.
#
# All providers support custom endpoints for proxy/relay services:
#   endpoint = "https://your-proxy.example.com/v1/chat/completions"

# Claude (Anthropic) - Recommended for best results
[ai.providers.claude]
provider_type = "claude"
api_key = "$ANTHROPIC_API_KEY"
model = "claude-sonnet-4-20250514"
# Other models: claude-3-5-haiku-20241022, claude-3-5-sonnet-20241022, claude-opus-4-20250514
# Custom proxy example:
# endpoint = "https://your-claude-proxy.com/v1/messages"

# OpenAI
[ai.providers.openai]
provider_type = "openai"
api_key = "$OPENAI_API_KEY"
endpoint = "https://api.openai.com/v1/chat/completions"
model = "gpt-4o-mini"
# Other models: gpt-4o, gpt-4-turbo, o1-mini, o1-preview
# Custom proxy example:
# endpoint = "https://your-openai-proxy.com/v1/chat/completions"

# Google Gemini
[ai.providers.gemini]
provider_type = "gemini"
api_key = "$GOOGLE_API_KEY"
model = "gemini-2.0-flash"
# Other models: gemini-1.5-pro, gemini-1.5-flash, gemini-2.0-flash-thinking
# Note: Gemini API requires API key in URL (Google's design)

# DeepSeek (Chinese AI, excellent for coding and reasoning)
[ai.providers.deepseek]
provider_type = "deepseek"
api_key = "$DEEPSEEK_API_KEY"
endpoint = "https://api.deepseek.com/v1/chat/completions"
model = "deepseek-chat"
# Other models: deepseek-coder, deepseek-reasoner

# 智谱AI GLM
[ai.providers.glm]
provider_type = "glm"
api_key = "$ZHIPU_API_KEY"
endpoint = "https://open.bigmodel.cn/api/paas/v4/chat/completions"
model = "glm-4-plus"
# Other models: glm-4-flash, glm-4-air, glm-4

# 阿里通义千问
[ai.providers.qwen]
provider_type = "qwen"
api_key = "$DASHSCOPE_API_KEY"
endpoint = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
model = "qwen-max"
# Other models: qwen-plus, qwen-turbo, qwen-coder-plus

# Ollama (Local models - no API key required)
[ai.providers.ollama]
provider_type = "ollama"
endpoint = "http://localhost:11434/api/chat"
model = "qwen2.5:7b"
# Other models: llama3.2, deepseek-r1, codellama, mistral

# OpenRouter (Access multiple providers through one API)
[ai.providers.openrouter]
provider_type = "openrouter"
api_key = "$OPENROUTER_API_KEY"
endpoint = "https://openrouter.ai/api/v1/chat/completions"
model = "anthropic/claude-sonnet-4"
# See https://openrouter.ai/models for available models

# Custom provider (any OpenAI-compatible API)
# [ai.providers.my_custom]
# provider_type = "custom"
# api_key = "$MY_CUSTOM_API_KEY"
# endpoint = "https://my-api.example.com/v1/chat/completions"
# model = "my-model"

#------------------------------------------------------------------------------
# Proxy/Relay Service Examples (中转服务配置示例)
#------------------------------------------------------------------------------
# You can use your own proxy/relay service for any provider by setting
# a custom endpoint. This is useful for:
# - Bypassing network restrictions
# - Using a unified API gateway
# - Cost optimization through relay services
#
# Example: Using a Claude proxy service
# [ai.providers.claude-proxy]
# provider_type = "claude"
# api_key = "$MY_PROXY_API_KEY"
# endpoint = "https://my-relay.example.com/claude/v1/messages"
# model = "claude-3-haiku-20240307"
#
# Example: Using an OpenAI-compatible relay (e.g., one-api, new-api)
# [ai.providers.openai-relay]
# provider_type = "openai"
# api_key = "$RELAY_API_KEY"
# endpoint = "https://relay.example.com/v1/chat/completions"
# model = "gpt-4"

#------------------------------------------------------------------------------
# REPL Commands for AI Management
#------------------------------------------------------------------------------
# In the shell, you can use these commands:
#
#   ai list              - List all configured providers
#   ai use <provider>    - Switch to a different provider
#   ai test              - Test the current provider connection
#   ai status            - Show current AI configuration status
#
# Example:
#   > ai list
#   > ai use deepseek
#   > ai test
#   > ?ai list all files modified today
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
