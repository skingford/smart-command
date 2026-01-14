use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub subcommands: Vec<CommandSpec>,
    #[serde(default)]
    pub flags: Vec<FlagSpec>,
    #[serde(default)]
    pub is_path_completion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagSpec {
    pub long: Option<String>,
    pub short: Option<char>,
    pub description: String,
    #[serde(default)]
    pub takes_value: bool,
}

impl CommandSpec {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            subcommands: vec![],
            flags: vec![],
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
    
    pub fn field(mut self, name: &str, val: bool) -> Self {
        if name == "is_path_completion" {
            self.is_path_completion = val;
        }
        self
    }
}
