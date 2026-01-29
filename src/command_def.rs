use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum I18nString {
    Simple(String),
    Map(HashMap<String, String>),
}

impl I18nString {
    pub fn get(&self, lang: &str) -> &str {
        match self {
            I18nString::Simple(s) => s,
            I18nString::Map(m) => m
                .get(lang)
                .or_else(|| m.get("en"))
                .map(|s| s.as_str())
                .unwrap_or(""),
        }
    }
}

// Implement From<String> for easier construction
impl From<String> for I18nString {
    fn from(s: String) -> Self {
        I18nString::Simple(s)
    }
}

impl From<&str> for I18nString {
    fn from(s: &str) -> Self {
        I18nString::Simple(s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub scenario: I18nString,
    pub cmd: String,
}

/// Argument type for validation and completion
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArgumentType {
    /// Plain string argument
    String,
    /// Numeric argument with optional range
    Number {
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
    },
    /// Boolean argument
    Boolean,
    /// Fixed set of choices
    Choice { values: Vec<String> },
    /// Regex pattern
    Pattern { regex: String },
    /// File/directory path
    Path {
        #[serde(default)]
        filter: PathFilterConfig,
    },
    /// Dynamic completion from provider
    Provider { name: String },
    /// URL
    Url,
    /// Email address
    Email,
    /// JSON string
    Json,
    /// Any value (default, no validation)
    #[default]
    Any,
}

/// Path filter configuration for YAML
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PathFilterConfig {
    /// Only show files with these extensions
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
    /// Exclude paths matching these patterns
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    /// Include hidden files
    #[serde(default)]
    pub include_hidden: bool,
    /// Only show files
    #[serde(default)]
    pub files_only: bool,
    /// Only show directories
    #[serde(default)]
    pub dirs_only: bool,
}

/// Common flag combination for quick completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagCombo {
    /// The combination string without leading dash, e.g., "zxvf"
    pub combo: String,
    /// Description of what this combination does
    pub description: I18nString,
}

/// Argument specification for commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentSpec {
    /// Position of this argument (0-indexed, after command/subcommand)
    #[serde(default)]
    pub position: Option<usize>,
    /// Argument name for documentation
    #[serde(default)]
    pub name: Option<String>,
    /// Description of this argument
    #[serde(default)]
    pub description: Option<I18nString>,
    /// Type of argument for validation and completion
    #[serde(default, rename = "type")]
    pub arg_type: ArgumentType,
    /// Whether this argument is required
    #[serde(default)]
    pub required: bool,
    /// Whether this argument can accept multiple values
    #[serde(default)]
    pub variadic: bool,
    /// Provider name for dynamic completion (shorthand for type: provider)
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String,
    pub description: I18nString,
    #[serde(default)]
    pub subcommands: Vec<CommandSpec>,
    #[serde(default)]
    pub flags: Vec<FlagSpec>,
    #[serde(default)]
    pub examples: Vec<Example>,
    #[serde(default)]
    pub is_path_completion: bool,
    /// Arguments specification for validation and dynamic completion
    #[serde(default)]
    pub arguments: Vec<ArgumentSpec>,
    /// Common flag combinations for quick completion (e.g., "zxvf" for tar)
    #[serde(default)]
    pub common_flag_combos: Vec<FlagCombo>,
}

/// Flag category for grouping in help display
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FlagCategory {
    /// Common/frequently used options
    #[default]
    Common,
    /// File-related options
    File,
    /// Output formatting options
    Output,
    /// Network-related options
    Network,
    /// Debug/verbose options
    Debug,
    /// Advanced/rarely used options
    Advanced,
    /// Configuration options
    Config,
    /// Filter/selection options
    Filter,
}

impl FlagCategory {
    /// Get the display name for this category
    pub fn display_name(&self, lang: &str) -> &'static str {
        match (self, lang) {
            (FlagCategory::Common, "zh") => "常用选项",
            (FlagCategory::Common, _) => "Common Options",
            (FlagCategory::File, "zh") => "文件选项",
            (FlagCategory::File, _) => "File Options",
            (FlagCategory::Output, "zh") => "输出选项",
            (FlagCategory::Output, _) => "Output Options",
            (FlagCategory::Network, "zh") => "网络选项",
            (FlagCategory::Network, _) => "Network Options",
            (FlagCategory::Debug, "zh") => "调试选项",
            (FlagCategory::Debug, _) => "Debug Options",
            (FlagCategory::Advanced, "zh") => "高级选项",
            (FlagCategory::Advanced, _) => "Advanced Options",
            (FlagCategory::Config, "zh") => "配置选项",
            (FlagCategory::Config, _) => "Config Options",
            (FlagCategory::Filter, "zh") => "过滤选项",
            (FlagCategory::Filter, _) => "Filter Options",
        }
    }

    /// Get the sort order for categories (lower = first)
    pub fn sort_order(&self) -> u8 {
        match self {
            FlagCategory::Common => 0,
            FlagCategory::File => 1,
            FlagCategory::Output => 2,
            FlagCategory::Filter => 3,
            FlagCategory::Network => 4,
            FlagCategory::Config => 5,
            FlagCategory::Debug => 6,
            FlagCategory::Advanced => 7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagSpec {
    pub long: Option<String>,
    pub short: Option<char>,
    pub description: I18nString,
    #[serde(default)]
    pub takes_value: bool,
    /// Type of the flag's value for validation
    #[serde(default)]
    pub value_type: Option<ArgumentType>,
    /// Category for grouping in help display
    #[serde(default)]
    pub category: FlagCategory,
}

impl CommandSpec {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: I18nString::Simple(description.to_string()),
            subcommands: vec![],
            flags: vec![],
            examples: vec![],
            is_path_completion: false,
            arguments: vec![],
            common_flag_combos: vec![],
        }
    }

    /// Get the provider name for a given argument position
    #[allow(dead_code)]
    pub fn get_provider_for_position(&self, position: usize) -> Option<&str> {
        self.arguments
            .iter()
            .find(|arg| arg.position == Some(position) || (arg.variadic && arg.position.map(|p| position >= p).unwrap_or(false)))
            .and_then(|arg| {
                // Check explicit provider field first
                if let Some(ref provider) = arg.provider {
                    return Some(provider.as_str());
                }
                // Then check argument type
                if let ArgumentType::Provider { ref name } = arg.arg_type {
                    return Some(name.as_str());
                }
                None
            })
    }

    #[allow(dead_code)]
    pub fn subcommand(mut self, sub: CommandSpec) -> Self {
        self.subcommands.push(sub);
        self
    }

    pub fn flag(mut self, flag: FlagSpec) -> Self {
        self.flags.push(flag);
        self
    }

    #[allow(dead_code)]
    pub fn example(mut self, example: Example) -> Self {
        self.examples.push(example);
        self
    }

    pub fn field(mut self, name: &str, val: bool) -> Self {
        if name == "is_path_completion" {
            self.is_path_completion = val;
        }
        self
    }
}
