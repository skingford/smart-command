//! Alias System for Smart Command
//!
//! Provides shell-like alias functionality with:
//! - Simple aliases (e.g., `ll` -> `ls -la`)
//! - Parameterized aliases (e.g., `gco $1` -> `git checkout $1`)
//! - Persistent storage in config file

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Alias definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    /// The alias name
    pub name: String,
    /// The command to expand to
    pub command: String,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
}

/// Alias manager
#[derive(Debug)]
pub struct AliasManager {
    aliases: HashMap<String, Alias>,
    config_path: PathBuf,
}

impl AliasManager {
    /// Create a new alias manager
    pub fn new() -> Self {
        let config_path = dirs::config_dir()
            .map(|p| p.join("smart-command").join("aliases.yaml"))
            .unwrap_or_else(|| PathBuf::from("~/.config/smart-command/aliases.yaml"));

        let mut manager = Self {
            aliases: HashMap::new(),
            config_path,
        };

        manager.load();
        manager.add_default_aliases();
        manager
    }

    /// Add default useful aliases
    fn add_default_aliases(&mut self) {
        let defaults = vec![
            ("ll", "ls -la", "Long listing with hidden files"),
            ("la", "ls -A", "List all except . and .."),
            ("l", "ls -CF", "List in columns"),
            ("...", "cd ../..", "Go up two directories"),
            ("....", "cd ../../..", "Go up three directories"),
            ("gs", "git status", "Git status"),
            ("ga", "git add", "Git add"),
            ("gc", "git commit", "Git commit"),
            ("gp", "git push", "Git push"),
            ("gl", "git pull", "Git pull"),
            ("gd", "git diff", "Git diff"),
            ("gco", "git checkout", "Git checkout"),
            ("gb", "git branch", "Git branch"),
            ("glog", "git log --oneline --graph --decorate", "Git log graph"),
            ("dc", "docker compose", "Docker compose shortcut"),
            ("dps", "docker ps", "Docker process list"),
            ("k", "kubectl", "Kubectl shortcut"),
            ("tf", "terraform", "Terraform shortcut"),
            ("py", "python3", "Python 3"),
            ("cls", "clear", "Clear screen"),
            ("h", "history", "Show history"),
            ("ports", "lsof -i -P -n | grep LISTEN", "Show listening ports"),
            ("myip", "curl -s ifconfig.me", "Show public IP"),
            ("weather", "curl -s wttr.in", "Show weather"),
        ];

        for (name, command, description) in defaults {
            if !self.aliases.contains_key(name) {
                self.aliases.insert(
                    name.to_string(),
                    Alias {
                        name: name.to_string(),
                        command: command.to_string(),
                        description: Some(description.to_string()),
                    },
                );
            }
        }
    }

    /// Load aliases from config file
    pub fn load(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.config_path) {
            if let Ok(aliases) = serde_yaml::from_str::<Vec<Alias>>(&content) {
                for alias in aliases {
                    self.aliases.insert(alias.name.clone(), alias);
                }
            }
        }
    }

    /// Save aliases to config file
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let aliases: Vec<&Alias> = self.aliases.values().collect();
        let content = serde_yaml::to_string(&aliases)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&self.config_path, content)
    }

    /// Add or update an alias
    pub fn add(&mut self, name: &str, command: &str, description: Option<&str>) {
        self.aliases.insert(
            name.to_string(),
            Alias {
                name: name.to_string(),
                command: command.to_string(),
                description: description.map(|s| s.to_string()),
            },
        );
    }

    /// Remove an alias
    pub fn remove(&mut self, name: &str) -> bool {
        self.aliases.remove(name).is_some()
    }

    /// Get an alias by name
    pub fn get(&self, name: &str) -> Option<&Alias> {
        self.aliases.get(name)
    }

    /// List all aliases
    pub fn list(&self) -> Vec<&Alias> {
        let mut aliases: Vec<_> = self.aliases.values().collect();
        aliases.sort_by(|a, b| a.name.cmp(&b.name));
        aliases
    }

    /// Expand a command if it starts with an alias
    pub fn expand(&self, input: &str) -> String {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0];
        let args = parts.get(1).unwrap_or(&"");

        if let Some(alias) = self.aliases.get(cmd) {
            let expanded = &alias.command;

            // Handle parameter substitution ($1, $2, etc.)
            if expanded.contains('$') && !args.is_empty() {
                let arg_list: Vec<&str> = args.split_whitespace().collect();
                let mut result = expanded.clone();

                // Replace $@ with all arguments
                result = result.replace("$@", args);

                // Replace $1, $2, etc. with specific arguments
                for (i, arg) in arg_list.iter().enumerate() {
                    result = result.replace(&format!("${}", i + 1), arg);
                }

                // Clean up any remaining parameter references
                for i in 1..=9 {
                    result = result.replace(&format!("${}", i), "");
                }

                result.trim().to_string()
            } else if args.is_empty() {
                expanded.clone()
            } else {
                format!("{} {}", expanded, args)
            }
        } else {
            input.to_string()
        }
    }

    /// Check if a command is an alias
    pub fn is_alias(&self, name: &str) -> bool {
        self.aliases.contains_key(name)
    }

    /// Get alias suggestions for completion
    pub fn get_suggestions(&self, partial: &str) -> Vec<(&str, Option<&str>)> {
        self.aliases
            .values()
            .filter(|a| {
                partial.is_empty()
                    || a.name.to_lowercase().starts_with(&partial.to_lowercase())
            })
            .map(|a| (a.name.as_str(), a.description.as_deref()))
            .collect()
    }
}

impl Default for AliasManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle alias-related commands (alias, unalias)
pub fn handle_alias_command(
    manager: &mut AliasManager,
    cmd: &str,
    args: &[&str],
) -> Option<String> {
    match cmd {
        "alias" => {
            if args.is_empty() {
                // List all aliases
                let aliases = manager.list();
                if aliases.is_empty() {
                    Some("No aliases defined.".to_string())
                } else {
                    let output: Vec<String> = aliases
                        .iter()
                        .map(|a| {
                            let desc = a
                                .description
                                .as_ref()
                                .map(|d| format!("  # {}", d))
                                .unwrap_or_default();
                            format!("alias {}='{}'{}", a.name, a.command, desc)
                        })
                        .collect();
                    Some(output.join("\n"))
                }
            } else if args.len() == 1 {
                // Show specific alias
                let name = args[0];
                if let Some(alias) = manager.get(name) {
                    Some(format!("alias {}='{}'", alias.name, alias.command))
                } else {
                    Some(format!("alias: {} not found", name))
                }
            } else {
                // Set alias: alias name=value or alias name value
                let first = args[0];
                if let Some(pos) = first.find('=') {
                    let name = &first[..pos];
                    let value = &first[pos + 1..];
                    // Handle quoted values
                    let value = value.trim_matches(|c| c == '\'' || c == '"');
                    manager.add(name, value, None);
                    let _ = manager.save();
                    Some(format!("alias {}='{}'", name, value))
                } else {
                    let name = args[0];
                    let value = args[1..].join(" ");
                    let value = value.trim_matches(|c| c == '\'' || c == '"');
                    manager.add(name, value, None);
                    let _ = manager.save();
                    Some(format!("alias {}='{}'", name, value))
                }
            }
        }
        "unalias" => {
            if args.is_empty() {
                Some("unalias: usage: unalias name".to_string())
            } else {
                let name = args[0];
                if manager.remove(name) {
                    let _ = manager.save();
                    Some(format!("unalias: {} removed", name))
                } else {
                    Some(format!("unalias: {} not found", name))
                }
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_expansion() {
        let mut manager = AliasManager {
            aliases: HashMap::new(),
            config_path: PathBuf::from("/tmp/test-aliases.yaml"),
        };

        manager.add("ll", "ls -la", None);
        manager.add("gco", "git checkout $1", None);

        assert_eq!(manager.expand("ll"), "ls -la");
        assert_eq!(manager.expand("ll /tmp"), "ls -la /tmp");
        assert_eq!(manager.expand("gco main"), "git checkout main");
        assert_eq!(manager.expand("unknown"), "unknown");
    }

    #[test]
    fn test_alias_management() {
        let mut manager = AliasManager {
            aliases: HashMap::new(),
            config_path: PathBuf::from("/tmp/test-aliases.yaml"),
        };

        manager.add("test", "echo test", Some("Test alias"));
        assert!(manager.is_alias("test"));
        assert_eq!(manager.get("test").unwrap().command, "echo test");

        manager.remove("test");
        assert!(!manager.is_alias("test"));
    }
}
