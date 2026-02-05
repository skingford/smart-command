//! AI Command Documentation Generator
//!
//! Queries AI to learn about command usage and generates CommandSpec definitions
//! that can be saved to YAML files.

use crate::ai::llm::{AiContext, AiError};
use crate::ai_stream::StreamingAiGenerator;
use crate::command_def::{CommandSpec, Example, FlagSpec, I18nString};
use crate::config::AiConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Learn intent detection result
#[derive(Debug, Clone)]
pub struct LearnIntent {
    /// The command name to learn about (may be used for future features)
    #[allow(dead_code)]
    pub command: String,
}

/// Detect if user input is a request to learn about a command
pub fn detect_learn_intent(input: &str) -> Option<LearnIntent> {
    let input_lower = input.to_lowercase();
    let trimmed = input.trim();

    // English patterns
    let learn_patterns_en = [
        "learn ", "learn about ", "teach me ", "explain ", "what is ", "what does ",
        "how to use ", "how do i use ", "tell me about ", "show me how ", "usage of ",
        "help with ", "guide for ", "tutorial for ", "documentation for ",
    ];

    // Chinese patterns
    let learn_patterns_zh = [
        "学习", "学一下", "教我", "介绍", "解释", "什么是", "怎么用", "如何使用",
        "告诉我", "用法", "帮我了解", "详细说明", "命令用法", "整理",
    ];

    // Command usage patterns (e.g., "rg 用法", "git 详细介绍")
    let usage_patterns_zh = [
        "用法", "介绍", "怎么用", "如何使用", "详细", "说明", "教程", "帮助",
    ];

    // Check English patterns
    for pattern in learn_patterns_en {
        if input_lower.starts_with(pattern) {
            let rest = trimmed[pattern.len()..].trim();
            if let Some(cmd) = extract_command_name(rest) {
                return Some(LearnIntent { command: cmd });
            }
        }
    }

    // Check Chinese patterns
    for pattern in learn_patterns_zh {
        if input_lower.contains(pattern) {
            if let Some(cmd) = extract_command_from_chinese(trimmed) {
                return Some(LearnIntent { command: cmd });
            }
        }
    }

    // Check for "command 用法" style patterns
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() >= 2 {
        // Check if first word looks like a command and second is a usage keyword
        let first = words[0];
        let rest = &words[1..].join(" ");

        for pattern in usage_patterns_zh {
            if rest.contains(pattern) && looks_like_command(first) {
                return Some(LearnIntent {
                    command: first.to_string(),
                });
            }
        }
    }

    // Check for single command with "详细" etc
    if words.len() == 1 && looks_like_command(words[0]) {
        // Single command name is not enough to trigger learn mode
        return None;
    }

    None
}

/// Check if a string looks like a command name
fn looks_like_command(s: &str) -> bool {
    let known_commands = [
        "ls", "cd", "cp", "mv", "rm", "mkdir", "cat", "grep", "find", "git", "docker",
        "npm", "cargo", "python", "pip", "node", "curl", "wget", "tar", "chmod", "chown",
        "sudo", "apt", "brew", "yum", "dnf", "pacman", "ssh", "scp", "rsync", "echo",
        "export", "source", "gh", "jq", "awk", "sed", "make", "cmake", "gcc", "go",
        "rustc", "java", "ruby", "perl", "php", "dotnet", "kubectl", "helm", "terraform",
        "rg", "fd", "bat", "exa", "eza", "fzf", "tmux", "vim", "nvim", "emacs", "code",
        "az", "aws", "gcloud", "deno", "bun", "pnpm", "yarn", "ps", "kill", "top", "htop",
    ];

    known_commands.contains(&s.to_lowercase().as_str())
        || s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Extract command name from the rest of the input
fn extract_command_name(input: &str) -> Option<String> {
    let words: Vec<&str> = input.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    // First word should be the command
    let first = words[0].trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
    if first.is_empty() {
        return None;
    }

    Some(first.to_string())
}

/// Extract command name from Chinese input
fn extract_command_from_chinese(input: &str) -> Option<String> {
    // Look for English command names in the input
    let words: Vec<&str> = input.split(|c: char| c.is_whitespace() || is_chinese_char(c))
        .filter(|s| !s.is_empty())
        .collect();

    for word in words {
        if looks_like_command(word) {
            return Some(word.to_string());
        }
    }

    None
}

/// Check if a character is a Chinese character
fn is_chinese_char(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}')
}

/// Clean markdown formatting from AI response for terminal display
pub fn clean_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // Remove ** bold markers
            '*' if chars.peek() == Some(&'*') => {
                chars.next(); // consume second *
            }
            // Remove single * (italic) at word boundaries
            '*' => {
                // Keep * if it looks like a bullet point (start of line or after newline)
                if result.is_empty() || result.ends_with('\n') || result.ends_with("  ") {
                    result.push(c);
                }
                // Otherwise skip it (likely italic marker)
            }
            // Remove backticks but keep the content
            '`' => {
                // Skip backticks entirely
            }
            // Convert ### headers to plain text
            '#' if result.is_empty() || result.ends_with('\n') => {
                // Skip all # at start of line
                while chars.peek() == Some(&'#') {
                    chars.next();
                }
                // Skip space after #
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
            }
            _ => result.push(c),
        }
    }

    result
}

/// System prompt for conversational command explanation (terminal-friendly format)
const EXPLAIN_SYSTEM_PROMPT: &str = r#"You are a helpful command-line expert assistant.

CRITICAL FORMATTING RULES - MUST FOLLOW:
1. NO MARKDOWN - never use **, `, #, or any markdown syntax
2. Plain text only - this displays in a terminal
3. Use CAPS for section headers
4. Use indentation (2 spaces) for hierarchy
5. Use - or • for bullet points
6. Use $ prefix for command examples
7. Keep lines under 80 chars

RESPONSE FORMAT:

[Command Name] - Brief description

OPTIONS:
  -x, --long-name    Description here
  -y, --another      Another description

EXAMPLES:
  $ command --flag value
    Explanation of what this does

  $ command subcommand
    Another example explanation

NOTES:
  - Important note 1
  - Important note 2

Respond in the same language as the user (Chinese/English)."#;

/// System prompt for command documentation generation (JSON output)
const DOCS_SYSTEM_PROMPT: &str = r#"You are a command-line expert assistant. Your task is to provide detailed documentation about shell commands.

When the user asks about a command, respond with a JSON object containing:
1. name: The command name
2. description_en: Brief description in English
3. description_zh: Brief description in Chinese
4. flags: Array of flag objects with:
   - long: Long flag name (without --)
   - short: Short flag character (without -, can be null)
   - description_en: Description in English
   - description_zh: Description in Chinese
   - takes_value: Boolean, whether flag accepts a value
5. subcommands: Array of subcommand objects (same structure as flags but with name instead of long/short)
6. examples: Array of example objects with:
   - cmd: The example command
   - scenario_en: What this example does (English)
   - scenario_zh: What this example does (Chinese)
7. is_path_completion: Boolean, whether this command typically takes file/directory paths as arguments

IMPORTANT:
- Output ONLY valid JSON, no markdown, no explanations
- Include the most commonly used flags and options
- Provide 3-5 practical examples
- Flag names should NOT include the leading dashes

Example output format:
{
  "name": "ls",
  "description_en": "List directory contents",
  "description_zh": "列出目录内容",
  "flags": [
    {"long": "all", "short": "a", "description_en": "Show hidden files", "description_zh": "显示隐藏文件", "takes_value": false},
    {"long": "long", "short": "l", "description_en": "Use long listing format", "description_zh": "使用长列表格式", "takes_value": false}
  ],
  "subcommands": [],
  "examples": [
    {"cmd": "ls -la", "scenario_en": "List all files in long format", "scenario_zh": "以长格式列出所有文件"}
  ],
  "is_path_completion": true
}"#;

/// Parsed command documentation from AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCommandDoc {
    pub name: String,
    pub description_en: String,
    pub description_zh: String,
    #[serde(default)]
    pub flags: Vec<AiFlagDoc>,
    #[serde(default)]
    pub subcommands: Vec<AiSubcommandDoc>,
    #[serde(default)]
    pub examples: Vec<AiExampleDoc>,
    #[serde(default)]
    pub is_path_completion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiFlagDoc {
    pub long: Option<String>,
    pub short: Option<String>,
    pub description_en: String,
    pub description_zh: String,
    #[serde(default)]
    pub takes_value: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSubcommandDoc {
    pub name: String,
    pub description_en: String,
    pub description_zh: String,
    #[serde(default)]
    pub flags: Vec<AiFlagDoc>,
    #[serde(default)]
    pub subcommands: Vec<AiSubcommandDoc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExampleDoc {
    pub cmd: String,
    pub scenario_en: String,
    pub scenario_zh: String,
}

impl AiCommandDoc {
    /// Maximum allowed lengths for various fields
    const MAX_NAME_LEN: usize = 50;
    const MAX_DESCRIPTION_LEN: usize = 500;
    const MAX_FLAG_NAME_LEN: usize = 50;
    const MAX_EXAMPLE_CMD_LEN: usize = 500;
    const MAX_FLAGS: usize = 100;
    const MAX_EXAMPLES: usize = 20;

    /// Validate the command documentation from AI response
    pub fn validate(&self) -> Result<(), String> {
        // Validate name
        if self.name.is_empty() {
            return Err("Command name cannot be empty".to_string());
        }
        if self.name.len() > Self::MAX_NAME_LEN {
            return Err(format!(
                "Command name too long: {} chars (max {})",
                self.name.len(),
                Self::MAX_NAME_LEN
            ));
        }
        if !self
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(format!(
                "Command name '{}' contains invalid characters",
                self.name
            ));
        }

        // Validate descriptions
        if self.description_en.len() > Self::MAX_DESCRIPTION_LEN {
            return Err("English description too long".to_string());
        }
        if self.description_zh.len() > Self::MAX_DESCRIPTION_LEN {
            return Err("Chinese description too long".to_string());
        }

        // Validate flags
        if self.flags.len() > Self::MAX_FLAGS {
            return Err(format!(
                "Too many flags: {} (max {})",
                self.flags.len(),
                Self::MAX_FLAGS
            ));
        }
        for flag in &self.flags {
            if let Some(ref long) = flag.long {
                if long.len() > Self::MAX_FLAG_NAME_LEN {
                    return Err(format!("Flag name '{}' too long", long));
                }
            }
        }

        // Validate examples
        if self.examples.len() > Self::MAX_EXAMPLES {
            return Err(format!(
                "Too many examples: {} (max {})",
                self.examples.len(),
                Self::MAX_EXAMPLES
            ));
        }
        for example in &self.examples {
            if example.cmd.len() > Self::MAX_EXAMPLE_CMD_LEN {
                return Err("Example command too long".to_string());
            }
            // Check for obvious shell injection patterns (but allow backticks in examples)
            if example.cmd.contains("$(") {
                return Err(
                    "Example command contains potential shell injection: $()".to_string()
                );
            }
        }

        Ok(())
    }

    /// Convert to CommandSpec for internal use
    pub fn to_command_spec(&self) -> CommandSpec {
        let description = I18nString::Map({
            let mut map = HashMap::new();
            map.insert("en".to_string(), self.description_en.clone());
            map.insert("zh".to_string(), self.description_zh.clone());
            map
        });

        let flags: Vec<FlagSpec> = self
            .flags
            .iter()
            .map(|f| FlagSpec {
                long: f.long.clone(),
                short: f.short.as_ref().and_then(|s| s.chars().next()),
                description: I18nString::Map({
                    let mut map = HashMap::new();
                    map.insert("en".to_string(), f.description_en.clone());
                    map.insert("zh".to_string(), f.description_zh.clone());
                    map
                }),
                takes_value: f.takes_value,
                value_type: None,
                category: crate::command_def::FlagCategory::Common,
            })
            .collect();

        let examples: Vec<Example> = self
            .examples
            .iter()
            .map(|e| Example {
                cmd: e.cmd.clone(),
                scenario: I18nString::Map({
                    let mut map = HashMap::new();
                    map.insert("en".to_string(), e.scenario_en.clone());
                    map.insert("zh".to_string(), e.scenario_zh.clone());
                    map
                }),
            })
            .collect();

        let subcommands: Vec<CommandSpec> = self
            .subcommands
            .iter()
            .map(|s| convert_subcommand(s))
            .collect();

        CommandSpec {
            name: self.name.clone(),
            description,
            subcommands,
            flags,
            examples,
            is_path_completion: self.is_path_completion,
            arguments: vec![],
            common_flag_combos: vec![],
        }
    }
}

fn convert_subcommand(sub: &AiSubcommandDoc) -> CommandSpec {
    let description = I18nString::Map({
        let mut map = HashMap::new();
        map.insert("en".to_string(), sub.description_en.clone());
        map.insert("zh".to_string(), sub.description_zh.clone());
        map
    });

    let flags: Vec<FlagSpec> = sub
        .flags
        .iter()
        .map(|f| FlagSpec {
            long: f.long.clone(),
            short: f.short.as_ref().and_then(|s| s.chars().next()),
            description: I18nString::Map({
                let mut map = HashMap::new();
                map.insert("en".to_string(), f.description_en.clone());
                map.insert("zh".to_string(), f.description_zh.clone());
                map
            }),
            takes_value: f.takes_value,
            value_type: None,
            category: crate::command_def::FlagCategory::Common,
        })
        .collect();

    let subcommands: Vec<CommandSpec> = sub
        .subcommands
        .iter()
        .map(|s| convert_subcommand(s))
        .collect();

    CommandSpec {
        name: sub.name.clone(),
        description,
        subcommands,
        flags,
        examples: vec![],
        is_path_completion: false,
        arguments: vec![],
        common_flag_combos: vec![],
    }
}

/// AI Documentation Generator
pub struct AiDocsGenerator {
    config: AiConfig,
}

impl AiDocsGenerator {
    pub fn new(config: &AiConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Explain a command in conversational mode (for AI chat mode)
    /// Returns the AI explanation text (not structured JSON)
    /// Output is cleaned of markdown formatting for terminal display
    #[allow(dead_code)]
    pub fn explain_command_streaming(
        &self,
        user_query: &str,
        session: Option<&crate::ai_stream::AiSession>,
    ) -> Result<String, AiError> {
        // Create a modified config with conversational system prompt
        let mut config = self.config.clone();
        config.global.system_prompt = EXPLAIN_SYSTEM_PROMPT.to_string();

        let generator = StreamingAiGenerator::new(&config);
        let context = AiContext::default();

        // Get raw response (already streamed to terminal)
        let raw_response = generator.generate_streaming(user_query, &context, session)?;

        // Clean markdown from the response for session history
        Ok(clean_markdown(&raw_response))
    }

    /// Explain command with cleaned output (non-streaming, for when clean display is critical)
    pub fn explain_command_clean(
        &self,
        user_query: &str,
        _session: Option<&crate::ai_stream::AiSession>,
    ) -> Result<String, AiError> {
        // Create a modified config with conversational system prompt
        let mut config = self.config.clone();
        config.global.system_prompt = EXPLAIN_SYSTEM_PROMPT.to_string();

        // Use non-streaming API and clean the result
        let generator = crate::ai::llm::AiCommandGenerator::new(&config);
        let context = AiContext::default();

        let raw = generator.generate(user_query, &context)?;
        let cleaned = clean_markdown(&raw);

        // Print cleaned output
        println!("{}", cleaned);

        Ok(cleaned)
    }

    /// Query AI for command documentation (streaming mode, returns structured JSON)
    pub fn query_command_streaming(&self, command_name: &str) -> Result<String, AiError> {
        // Create a modified config with our specialized system prompt
        let mut config = self.config.clone();
        config.global.system_prompt = DOCS_SYSTEM_PROMPT.to_string();

        let generator = StreamingAiGenerator::new(&config);
        let context = AiContext::default();

        let query = format!(
            "Please provide detailed documentation for the '{}' command in JSON format.",
            command_name
        );

        generator.generate_streaming(&query, &context, None)
    }

    /// Parse AI response into AiCommandDoc
    pub fn parse_response(&self, response: &str) -> Result<AiCommandDoc, String> {
        // Try to extract JSON from the response
        let json_str = extract_json(response)?;

        // Parse the JSON
        serde_json::from_str(&json_str).map_err(|e| format!("Failed to parse JSON: {}", e))
    }

    /// Query and parse in one step
    pub fn learn_command(&self, command_name: &str) -> Result<AiCommandDoc, String> {
        let response = self
            .query_command_streaming(command_name)
            .map_err(|e| e.to_string())?;

        let doc = self.parse_response(&response)?;

        // Validate the parsed document
        doc.validate()?;

        Ok(doc)
    }
}

/// Extract JSON from AI response (handles markdown code blocks, etc.)
fn extract_json(response: &str) -> Result<String, String> {
    let response = response.trim();

    // Try to extract from markdown code block first (most reliable)
    if let Some(json) = extract_from_code_block(response) {
        // Validate it's actually valid JSON
        if serde_json::from_str::<serde_json::Value>(&json).is_ok() {
            return Ok(json);
        }
    }

    // Try to find and validate JSON object in the response
    if let Some(start) = response.find('{') {
        // Try progressively larger substrings ending with }
        let remaining = &response[start..];
        for (i, c) in remaining.char_indices() {
            if c == '}' {
                let candidate = &remaining[..=i];
                if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                    return Ok(candidate.to_string());
                }
            }
        }
    }

    Err("Could not find valid JSON in response".to_string())
}

/// Extract content from markdown code block
fn extract_from_code_block(response: &str) -> Option<String> {
    // Try ```json block
    if let Some(start) = response.find("```json") {
        let json_start = start + 7;
        if let Some(end) = response[json_start..].find("```") {
            return Some(response[json_start..json_start + end].trim().to_string());
        }
    }

    // Try generic ``` block
    if let Some(start) = response.find("```") {
        let content_start = response[start + 3..].find('\n').map(|i| start + 3 + i + 1)?;
        if let Some(end) = response[content_start..].find("```") {
            return Some(response[content_start..content_start + end].trim().to_string());
        }
    }

    None
}

/// Save CommandSpec to YAML file
#[allow(dead_code)]
pub fn save_command_spec(spec: &CommandSpec, dir: &PathBuf) -> Result<PathBuf, String> {
    // Ensure directory exists
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    let file_path = dir.join(format!("{}.yaml", spec.name));

    // Serialize to YAML
    let yaml =
        serde_yaml::to_string(spec).map_err(|e| format!("Failed to serialize to YAML: {}", e))?;

    // Write to file
    fs::write(&file_path, yaml).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(file_path)
}

// Use loader::get_user_definitions_dir() instead of duplicating here

/// Format CommandSpec as a preview string
pub fn format_command_preview(spec: &CommandSpec, lang: &str) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "\n{} - {}\n",
        spec.name,
        spec.description.get(lang)
    ));
    output.push_str(&"─".repeat(60));
    output.push('\n');

    // Flags
    if !spec.flags.is_empty() {
        output.push_str("\nFlags:\n");
        for flag in &spec.flags {
            let short = flag
                .short
                .map(|c| format!("-{}, ", c))
                .unwrap_or_default();
            let long = flag
                .long
                .as_ref()
                .map(|l| format!("--{}", l))
                .unwrap_or_default();
            let value_hint = if flag.takes_value { " <value>" } else { "" };
            output.push_str(&format!(
                "  {}{}{}\n    {}\n",
                short,
                long,
                value_hint,
                flag.description.get(lang)
            ));
        }
    }

    // Subcommands
    if !spec.subcommands.is_empty() {
        output.push_str("\nSubcommands:\n");
        for sub in &spec.subcommands {
            output.push_str(&format!("  {}  {}\n", sub.name, sub.description.get(lang)));
        }
    }

    // Examples
    if !spec.examples.is_empty() {
        output.push_str("\nExamples:\n");
        for example in &spec.examples {
            output.push_str(&format!("  $ {}\n", example.cmd));
            output.push_str(&format!("    {}\n", example.scenario.get(lang)));
        }
    }

    output.push_str(&"─".repeat(60));
    output.push('\n');

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_pure() {
        let response = r#"{"name": "ls", "description_en": "List files"}"#;
        let result = extract_json(response);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_json_with_embedded_braces() {
        let response = r#"{"name": "test", "description_en": "Use {braces} here"}"#;
        let result = extract_json(response);
        assert!(result.is_ok());
        // Verify the JSON is actually valid
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&result.unwrap());
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_validate_valid_command() {
        let doc = AiCommandDoc {
            name: "test-cmd".to_string(),
            description_en: "Test command".to_string(),
            description_zh: "测试命令".to_string(),
            flags: vec![],
            subcommands: vec![],
            examples: vec![],
            is_path_completion: false,
        };
        assert!(doc.validate().is_ok());
    }

    #[test]
    fn test_validate_rejects_empty_name() {
        let doc = AiCommandDoc {
            name: "".to_string(),
            description_en: "Test".to_string(),
            description_zh: "测试".to_string(),
            flags: vec![],
            subcommands: vec![],
            examples: vec![],
            is_path_completion: false,
        };
        assert!(doc.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_invalid_chars_in_name() {
        let doc = AiCommandDoc {
            name: "test/cmd".to_string(),
            description_en: "Test".to_string(),
            description_zh: "测试".to_string(),
            flags: vec![],
            subcommands: vec![],
            examples: vec![],
            is_path_completion: false,
        };
        assert!(doc.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_shell_injection() {
        let doc = AiCommandDoc {
            name: "test".to_string(),
            description_en: "Test".to_string(),
            description_zh: "测试".to_string(),
            flags: vec![],
            subcommands: vec![],
            examples: vec![AiExampleDoc {
                cmd: "echo $(whoami)".to_string(),
                scenario_en: "Dangerous".to_string(),
                scenario_zh: "危险".to_string(),
            }],
            is_path_completion: false,
        };
        assert!(doc.validate().is_err());
    }

    #[test]
    fn test_detect_learn_intent_english() {
        let intent = detect_learn_intent("learn rg");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().command, "rg");

        let intent = detect_learn_intent("explain git");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().command, "git");

        let intent = detect_learn_intent("how to use docker");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().command, "docker");
    }

    #[test]
    fn test_detect_learn_intent_chinese() {
        let intent = detect_learn_intent("学习 rg 命令");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().command, "rg");

        let intent = detect_learn_intent("介绍 git 用法");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().command, "git");

        let intent = detect_learn_intent("rg 用法");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().command, "rg");

        let intent = detect_learn_intent("整理 curl 详细用法");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().command, "curl");
    }

    #[test]
    fn test_clean_markdown_bold() {
        assert_eq!(clean_markdown("**bold**"), "bold");
        assert_eq!(clean_markdown("This is **bold** text"), "This is bold text");
    }

    #[test]
    fn test_clean_markdown_backticks() {
        assert_eq!(clean_markdown("`code`"), "code");
        assert_eq!(clean_markdown("Run `git status` command"), "Run git status command");
    }

    #[test]
    fn test_clean_markdown_headers() {
        assert_eq!(clean_markdown("### Header"), "Header");
        assert_eq!(clean_markdown("# Title\nContent"), "Title\nContent");
    }

    #[test]
    fn test_clean_markdown_preserves_bullets() {
        assert_eq!(clean_markdown("* item"), "* item");
        assert_eq!(clean_markdown("  * nested"), "  * nested");
    }

    #[test]
    fn test_detect_learn_intent_negative() {
        // Should not trigger for plain commands
        let intent = detect_learn_intent("git status");
        assert!(intent.is_none());

        let intent = detect_learn_intent("ls -la");
        assert!(intent.is_none());
    }

    #[test]
    fn test_to_command_spec() {
        let doc = AiCommandDoc {
            name: "mytest".to_string(),
            description_en: "My test command".to_string(),
            description_zh: "我的测试命令".to_string(),
            flags: vec![AiFlagDoc {
                long: Some("verbose".to_string()),
                short: Some("v".to_string()),
                description_en: "Verbose output".to_string(),
                description_zh: "详细输出".to_string(),
                takes_value: false,
            }],
            subcommands: vec![],
            examples: vec![AiExampleDoc {
                cmd: "mytest --verbose".to_string(),
                scenario_en: "Run with verbose".to_string(),
                scenario_zh: "详细运行".to_string(),
            }],
            is_path_completion: true,
        };

        let spec = doc.to_command_spec();
        assert_eq!(spec.name, "mytest");
        assert_eq!(spec.flags.len(), 1);
        assert_eq!(spec.flags[0].short, Some('v'));
        assert_eq!(spec.examples.len(), 1);
        assert!(spec.is_path_completion);
    }

    #[test]
    fn test_extract_json_markdown() {
        let response = r#"Here's the documentation:
```json
{"name": "ls", "description_en": "List files"}
```"#;
        let result = extract_json(response);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("name"));
    }

    #[test]
    fn test_extract_json_embedded() {
        let response = r#"The command is: {"name": "ls"} end"#;
        let result = extract_json(response);
        assert!(result.is_ok());
    }
}
