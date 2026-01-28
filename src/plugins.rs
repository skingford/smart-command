//! Plugin System for Smart Command
//!
//! Provides a framework for extending shell functionality through:
//! - Custom command providers
//! - Custom completers
//! - Event hooks
//! - Command transformers

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: Option<String>,
    /// Author
    pub author: Option<String>,
    /// Plugin type
    pub plugin_type: PluginType,
    /// Entry point (script path relative to plugin dir)
    pub entry: String,
    /// Commands this plugin provides/enhances
    #[serde(default)]
    pub commands: Vec<String>,
    /// Events this plugin hooks into
    #[serde(default)]
    pub hooks: Vec<String>,
}

/// Types of plugins
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    /// Provides completions for commands
    Completer,
    /// Transforms commands before execution
    Transformer,
    /// Hooks into shell events
    Hook,
    /// Provides custom shell commands
    Command,
    /// Combination of multiple types
    Mixed,
}

/// Plugin state
#[derive(Debug)]
pub struct Plugin {
    pub meta: PluginMeta,
    pub path: PathBuf,
    pub enabled: bool,
}

impl Plugin {
    /// Load a plugin from its directory
    pub fn load(path: &Path) -> Option<Self> {
        let manifest_path = path.join("plugin.yaml");
        if !manifest_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&manifest_path).ok()?;
        let meta: PluginMeta = serde_yaml::from_str(&content).ok()?;

        Some(Self {
            meta,
            path: path.to_path_buf(),
            enabled: true,
        })
    }

    /// Get the full path to the plugin entry point
    pub fn entry_path(&self) -> PathBuf {
        self.path.join(&self.meta.entry)
    }
}

/// Plugin manager
#[derive(Debug)]
pub struct PluginManager {
    plugins: HashMap<String, Plugin>,
    plugins_dir: PathBuf,
    disabled_plugins: Vec<String>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new() -> Self {
        let plugins_dir = dirs::config_dir()
            .map(|p| p.join("smart-command").join("plugins"))
            .unwrap_or_else(|| PathBuf::from("~/.config/smart-command/plugins"));

        let mut manager = Self {
            plugins: HashMap::new(),
            plugins_dir,
            disabled_plugins: Vec::new(),
        };

        manager.scan_plugins();
        manager
    }

    /// Scan plugins directory for available plugins
    pub fn scan_plugins(&mut self) {
        if !self.plugins_dir.exists() {
            // Create plugins directory if it doesn't exist
            let _ = fs::create_dir_all(&self.plugins_dir);
            return;
        }

        if let Ok(entries) = fs::read_dir(&self.plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(plugin) = Plugin::load(&path) {
                        let name = plugin.meta.name.clone();
                        let enabled = !self.disabled_plugins.contains(&name);
                        let mut plugin = plugin;
                        plugin.enabled = enabled;
                        self.plugins.insert(name, plugin);
                    }
                }
            }
        }
    }

    /// Get all loaded plugins
    pub fn list(&self) -> Vec<&Plugin> {
        self.plugins.values().collect()
    }

    /// Get enabled plugins of a specific type
    pub fn get_by_type(&self, plugin_type: PluginType) -> Vec<&Plugin> {
        self.plugins
            .values()
            .filter(|p| p.enabled && (p.meta.plugin_type == plugin_type || p.meta.plugin_type == PluginType::Mixed))
            .collect()
    }

    /// Enable a plugin
    pub fn enable(&mut self, name: &str) -> bool {
        if let Some(plugin) = self.plugins.get_mut(name) {
            plugin.enabled = true;
            self.disabled_plugins.retain(|n| n != name);
            true
        } else {
            false
        }
    }

    /// Disable a plugin
    pub fn disable(&mut self, name: &str) -> bool {
        if let Some(plugin) = self.plugins.get_mut(name) {
            plugin.enabled = false;
            if !self.disabled_plugins.contains(&name.to_string()) {
                self.disabled_plugins.push(name.to_string());
            }
            true
        } else {
            false
        }
    }

    /// Get a plugin by name
    pub fn get(&self, name: &str) -> Option<&Plugin> {
        self.plugins.get(name)
    }

    /// Reload all plugins
    pub fn reload(&mut self) {
        self.plugins.clear();
        self.scan_plugins();
    }

    /// Get the plugins directory path
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Shell event types for hooks
#[derive(Debug, Clone, PartialEq)]
pub enum ShellEvent {
    /// Before command execution
    PreExec { command: String },
    /// After command execution
    PostExec { command: String, exit_code: i32 },
    /// Directory change
    DirectoryChange { from: PathBuf, to: PathBuf },
    /// Shell startup
    Startup,
    /// Shell shutdown
    Shutdown,
    /// Completion requested
    Complete { input: String, position: usize },
}

/// Hook result
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Continue normal execution
    Continue,
    /// Skip the action
    Skip,
    /// Replace with different command
    Replace(String),
    /// Error occurred
    Error(String),
}

/// Handle plugin-related commands
pub fn handle_plugin_command(
    manager: &mut PluginManager,
    cmd: &str,
    args: &[&str],
) -> Option<String> {
    match cmd {
        "plugin" | "plugins" => {
            if args.is_empty() || args[0] == "list" {
                let plugins = manager.list();
                if plugins.is_empty() {
                    Some(format!(
                        "No plugins installed.\nPlugins directory: {}",
                        manager.plugins_dir().display()
                    ))
                } else {
                    let output: Vec<String> = plugins
                        .iter()
                        .map(|p| {
                            let status = if p.enabled { "✓" } else { "✗" };
                            let desc = p
                                .meta
                                .description
                                .as_ref()
                                .map(|d| format!(" - {}", d))
                                .unwrap_or_default();
                            format!(
                                "[{}] {} v{} ({:?}){}",
                                status, p.meta.name, p.meta.version, p.meta.plugin_type, desc
                            )
                        })
                        .collect();
                    Some(output.join("\n"))
                }
            } else if args[0] == "enable" {
                if let Some(name) = args.get(1) {
                    if manager.enable(name) {
                        Some(format!("Enabled plugin: {}", name))
                    } else {
                        Some(format!("Plugin not found: {}", name))
                    }
                } else {
                    Some("Usage: plugin enable <name>".to_string())
                }
            } else if args[0] == "disable" {
                if let Some(name) = args.get(1) {
                    if manager.disable(name) {
                        Some(format!("Disabled plugin: {}", name))
                    } else {
                        Some(format!("Plugin not found: {}", name))
                    }
                } else {
                    Some("Usage: plugin disable <name>".to_string())
                }
            } else if args[0] == "reload" {
                manager.reload();
                Some(format!("Reloaded {} plugins", manager.list().len()))
            } else if args[0] == "path" {
                Some(format!("Plugins directory: {}", manager.plugins_dir().display()))
            } else {
                Some("Usage: plugin [list|enable|disable|reload|path]".to_string())
            }
        }
        _ => None,
    }
}

/// Create an example plugin template
pub fn create_plugin_template(plugins_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let plugin_dir = plugins_dir.join(name);
    fs::create_dir_all(&plugin_dir)?;

    // Create plugin.yaml
    let manifest = format!(
        r#"name: {}
version: "0.1.0"
description: "My custom plugin"
author: "Your Name"
plugin_type: completer
entry: "main.sh"
commands:
  - mycommand
hooks: []
"#,
        name
    );
    fs::write(plugin_dir.join("plugin.yaml"), manifest)?;

    // Create main.sh
    let script = r#"#!/bin/bash
# Plugin entry point
# This script receives JSON input on stdin and outputs JSON

# Example: Read input
# input=$(cat)
# echo "$input" | jq .

echo "Hello from plugin!"
"#;
    fs::write(plugin_dir.join("main.sh"), script)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(plugin_dir.join("main.sh"))?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(plugin_dir.join("main.sh"), perms)?;
    }

    Ok(plugin_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_meta_deserialize() {
        let yaml = r#"
name: test-plugin
version: "1.0.0"
description: "Test plugin"
plugin_type: completer
entry: main.sh
commands:
  - test
"#;
        let meta: PluginMeta = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(meta.name, "test-plugin");
        assert_eq!(meta.plugin_type, PluginType::Completer);
    }

    #[test]
    fn test_plugin_manager_creation() {
        // Just test that it doesn't panic
        let _manager = PluginManager::new();
    }
}
