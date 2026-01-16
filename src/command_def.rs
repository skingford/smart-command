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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagSpec {
    pub long: Option<String>,
    pub short: Option<char>,
    pub description: I18nString,
    #[serde(default)]
    pub takes_value: bool,
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
        }
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
