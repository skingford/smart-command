//! Snippets System for Smart Command
//!
//! Provides quick command templates with placeholders:
//! - Text snippets with cursor positions
//! - Parameterized templates with default values
//! - Category-based organization

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// A command snippet with placeholders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    /// Unique trigger keyword
    pub trigger: String,
    /// The command template
    pub template: String,
    /// Description of the snippet
    #[serde(default)]
    pub description: Option<String>,
    /// Category for organization
    #[serde(default)]
    pub category: Option<String>,
    /// Placeholders with default values
    #[serde(default)]
    pub placeholders: HashMap<String, String>,
}

impl Snippet {
    /// Expand the snippet with provided values
    pub fn expand(&self, values: &HashMap<String, String>) -> String {
        let mut result = self.template.clone();

        // First, apply provided values
        for (key, value) in values {
            result = result.replace(&format!("${{{}}}", key), value);
            result = result.replace(&format!("${}", key), value);
        }

        // Then, apply defaults for remaining placeholders
        for (key, default) in &self.placeholders {
            result = result.replace(&format!("${{{}}}", key), default);
            result = result.replace(&format!("${}", key), default);
        }

        result
    }

    /// Get list of placeholder names
    pub fn get_placeholders(&self) -> Vec<String> {
        let mut placeholders = Vec::new();
        let re_braced = regex::Regex::new(r"\$\{(\w+)\}").ok();
        let re_simple = regex::Regex::new(r"\$(\w+)").ok();

        if let Some(re) = re_braced {
            for cap in re.captures_iter(&self.template) {
                let name = cap[1].to_string();
                if !placeholders.contains(&name) {
                    placeholders.push(name);
                }
            }
        }

        if let Some(re) = re_simple {
            for cap in re.captures_iter(&self.template) {
                let name = cap[1].to_string();
                if !placeholders.contains(&name) && !name.chars().all(|c| c.is_numeric()) {
                    placeholders.push(name);
                }
            }
        }

        placeholders
    }
}

/// Snippet manager
#[derive(Debug)]
pub struct SnippetManager {
    snippets: HashMap<String, Snippet>,
    config_path: PathBuf,
}

impl SnippetManager {
    /// Create a new snippet manager
    pub fn new() -> Self {
        let config_path = dirs::config_dir()
            .map(|p| p.join("smart-command").join("snippets.yaml"))
            .unwrap_or_else(|| PathBuf::from("~/.config/smart-command/snippets.yaml"));

        let mut manager = Self {
            snippets: HashMap::new(),
            config_path,
        };

        manager.load();
        manager.add_default_snippets();
        manager
    }

    /// Add default useful snippets
    fn add_default_snippets(&mut self) {
        let defaults = vec![
            // Git snippets
            (
                "gcommit",
                "git commit -m \"${message}\"",
                "Git commit with message",
                "git",
                vec![("message", "feat: ")],
            ),
            (
                "gstash",
                "git stash push -m \"${message}\"",
                "Git stash with message",
                "git",
                vec![("message", "WIP")],
            ),
            (
                "grebase",
                "git rebase -i HEAD~${count}",
                "Interactive rebase",
                "git",
                vec![("count", "3")],
            ),
            (
                "gclone",
                "git clone ${url} ${dir}",
                "Clone repository",
                "git",
                vec![("url", ""), ("dir", "")],
            ),
            // Docker snippets
            (
                "drun",
                "docker run -it --rm ${image} ${cmd}",
                "Run interactive container",
                "docker",
                vec![("image", "ubuntu"), ("cmd", "/bin/bash")],
            ),
            (
                "dbuild",
                "docker build -t ${tag} ${path}",
                "Build docker image",
                "docker",
                vec![("tag", "myapp:latest"), ("path", ".")],
            ),
            (
                "dexec",
                "docker exec -it ${container} ${cmd}",
                "Execute in container",
                "docker",
                vec![("container", ""), ("cmd", "/bin/bash")],
            ),
            (
                "dlogs",
                "docker logs -f --tail ${lines} ${container}",
                "Follow container logs",
                "docker",
                vec![("lines", "100"), ("container", "")],
            ),
            // Kubernetes snippets
            (
                "kpods",
                "kubectl get pods -n ${namespace}",
                "List pods in namespace",
                "kubernetes",
                vec![("namespace", "default")],
            ),
            (
                "klogs",
                "kubectl logs -f ${pod} -n ${namespace}",
                "Follow pod logs",
                "kubernetes",
                vec![("pod", ""), ("namespace", "default")],
            ),
            (
                "kexec",
                "kubectl exec -it ${pod} -n ${namespace} -- ${cmd}",
                "Execute in pod",
                "kubernetes",
                vec![("pod", ""), ("namespace", "default"), ("cmd", "/bin/sh")],
            ),
            (
                "kapply",
                "kubectl apply -f ${file}",
                "Apply manifest",
                "kubernetes",
                vec![("file", "")],
            ),
            // File operations
            (
                "tgz",
                "tar -czvf ${archive}.tar.gz ${source}",
                "Create tar.gz archive",
                "archive",
                vec![("archive", "archive"), ("source", ".")],
            ),
            (
                "untgz",
                "tar -xzvf ${archive}",
                "Extract tar.gz archive",
                "archive",
                vec![("archive", "")],
            ),
            (
                "findtext",
                "grep -rn \"${pattern}\" ${path}",
                "Find text in files",
                "search",
                vec![("pattern", ""), ("path", ".")],
            ),
            (
                "findfile",
                "find ${path} -name \"${pattern}\"",
                "Find files by name",
                "search",
                vec![("path", "."), ("pattern", "*")],
            ),
            // SSH snippets
            (
                "sshkey",
                "ssh-keygen -t ed25519 -C \"${email}\"",
                "Generate SSH key",
                "ssh",
                vec![("email", "")],
            ),
            (
                "sshtunnel",
                "ssh -L ${local_port}:${remote_host}:${remote_port} ${server}",
                "SSH tunnel",
                "ssh",
                vec![
                    ("local_port", "8080"),
                    ("remote_host", "localhost"),
                    ("remote_port", "80"),
                    ("server", ""),
                ],
            ),
            // Development
            (
                "httpserve",
                "python3 -m http.server ${port}",
                "Start HTTP server",
                "dev",
                vec![("port", "8000")],
            ),
            (
                "watchfiles",
                "fswatch -o ${path} | xargs -n1 ${command}",
                "Watch files for changes",
                "dev",
                vec![("path", "."), ("command", "echo changed")],
            ),
        ];

        for (trigger, template, description, category, placeholders) in defaults {
            if !self.snippets.contains_key(trigger) {
                let mut ph = HashMap::new();
                for (k, v) in placeholders {
                    ph.insert(k.to_string(), v.to_string());
                }

                self.snippets.insert(
                    trigger.to_string(),
                    Snippet {
                        trigger: trigger.to_string(),
                        template: template.to_string(),
                        description: Some(description.to_string()),
                        category: Some(category.to_string()),
                        placeholders: ph,
                    },
                );
            }
        }
    }

    /// Load snippets from config file
    pub fn load(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.config_path) {
            if let Ok(snippets) = serde_yaml::from_str::<Vec<Snippet>>(&content) {
                for snippet in snippets {
                    self.snippets.insert(snippet.trigger.clone(), snippet);
                }
            }
        }
    }

    /// Save snippets to config file
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let snippets: Vec<&Snippet> = self.snippets.values().collect();
        let content = serde_yaml::to_string(&snippets)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&self.config_path, content)
    }

    /// Add a snippet
    pub fn add(&mut self, snippet: Snippet) {
        self.snippets.insert(snippet.trigger.clone(), snippet);
    }

    /// Remove a snippet
    pub fn remove(&mut self, trigger: &str) -> bool {
        self.snippets.remove(trigger).is_some()
    }

    /// Get a snippet by trigger
    pub fn get(&self, trigger: &str) -> Option<&Snippet> {
        self.snippets.get(trigger)
    }

    /// List all snippets
    pub fn list(&self) -> Vec<&Snippet> {
        let mut snippets: Vec<_> = self.snippets.values().collect();
        snippets.sort_by(|a, b| {
            a.category
                .cmp(&b.category)
                .then_with(|| a.trigger.cmp(&b.trigger))
        });
        snippets
    }

    /// List snippets by category
    pub fn list_by_category(&self, category: &str) -> Vec<&Snippet> {
        self.snippets
            .values()
            .filter(|s| s.category.as_deref() == Some(category))
            .collect()
    }

    /// Get snippet suggestions for completion
    pub fn get_suggestions(&self, partial: &str) -> Vec<(&str, Option<&str>)> {
        self.snippets
            .values()
            .filter(|s| {
                partial.is_empty()
                    || s.trigger.to_lowercase().starts_with(&partial.to_lowercase())
            })
            .map(|s| (s.trigger.as_str(), s.description.as_deref()))
            .collect()
    }

    /// Expand a snippet if input starts with `:` prefix
    pub fn try_expand(&self, input: &str) -> Option<String> {
        if !input.starts_with(':') {
            return None;
        }

        let parts: Vec<&str> = input[1..].splitn(2, ' ').collect();
        let trigger = parts[0];

        self.snippets.get(trigger).map(|snippet| {
            // Parse any provided values (key=value format)
            let mut values = HashMap::new();
            if let Some(args) = parts.get(1) {
                for arg in args.split_whitespace() {
                    if let Some(pos) = arg.find('=') {
                        let key = &arg[..pos];
                        let value = &arg[pos + 1..];
                        values.insert(key.to_string(), value.to_string());
                    }
                }
            }

            snippet.expand(&values)
        })
    }
}

impl Default for SnippetManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle snippet-related commands
pub fn handle_snippet_command(
    manager: &mut SnippetManager,
    cmd: &str,
    args: &[&str],
) -> Option<String> {
    match cmd {
        "snippet" | "snip" => {
            if args.is_empty() {
                // List all snippets
                let snippets = manager.list();
                if snippets.is_empty() {
                    Some("No snippets defined.".to_string())
                } else {
                    let mut output = String::new();
                    let mut current_category: Option<&str> = None;

                    for snippet in snippets {
                        let cat = snippet.category.as_deref().unwrap_or("other");
                        if current_category != Some(cat) {
                            output.push_str(&format!("\n[{}]\n", cat));
                            current_category = Some(cat);
                        }

                        let desc = snippet
                            .description
                            .as_ref()
                            .map(|d| format!(" - {}", d))
                            .unwrap_or_default();
                        output.push_str(&format!("  :{}{}\n", snippet.trigger, desc));
                        output.push_str(&format!("    {}\n", snippet.template));
                    }

                    Some(output.trim().to_string())
                }
            } else {
                // Show specific snippet or expand it
                let trigger = args[0].trim_start_matches(':');
                if let Some(snippet) = manager.get(trigger) {
                    let desc = snippet
                        .description
                        .as_ref()
                        .map(|d| format!("\nDescription: {}", d))
                        .unwrap_or_default();

                    let placeholders = if snippet.placeholders.is_empty() {
                        String::new()
                    } else {
                        let ph: Vec<String> = snippet
                            .placeholders
                            .iter()
                            .map(|(k, v)| {
                                if v.is_empty() {
                                    format!("  ${{{}}}", k)
                                } else {
                                    format!("  ${{{}}} = \"{}\"", k, v)
                                }
                            })
                            .collect();
                        format!("\nPlaceholders:\n{}", ph.join("\n"))
                    };

                    Some(format!(
                        ":{}{}\nTemplate: {}{}",
                        snippet.trigger, desc, snippet.template, placeholders
                    ))
                } else {
                    Some(format!("Snippet '{}' not found", trigger))
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
    fn test_snippet_expansion() {
        let mut placeholders = HashMap::new();
        placeholders.insert("message".to_string(), "default".to_string());

        let snippet = Snippet {
            trigger: "test".to_string(),
            template: "git commit -m \"${message}\"".to_string(),
            description: None,
            category: None,
            placeholders,
        };

        // With default
        let expanded = snippet.expand(&HashMap::new());
        assert_eq!(expanded, "git commit -m \"default\"");

        // With custom value
        let mut values = HashMap::new();
        values.insert("message".to_string(), "custom".to_string());
        let expanded = snippet.expand(&values);
        assert_eq!(expanded, "git commit -m \"custom\"");
    }

    #[test]
    fn test_snippet_manager() {
        let mut manager = SnippetManager {
            snippets: HashMap::new(),
            config_path: PathBuf::from("/tmp/test-snippets.yaml"),
        };

        manager.add(Snippet {
            trigger: "test".to_string(),
            template: "echo test".to_string(),
            description: Some("Test snippet".to_string()),
            category: Some("test".to_string()),
            placeholders: HashMap::new(),
        });

        assert!(manager.get("test").is_some());
        assert_eq!(manager.get("test").unwrap().template, "echo test");
    }

    #[test]
    fn test_try_expand() {
        let mut manager = SnippetManager {
            snippets: HashMap::new(),
            config_path: PathBuf::from("/tmp/test-snippets.yaml"),
        };

        let mut placeholders = HashMap::new();
        placeholders.insert("name".to_string(), "world".to_string());

        manager.add(Snippet {
            trigger: "hello".to_string(),
            template: "echo Hello ${name}".to_string(),
            description: None,
            category: None,
            placeholders,
        });

        // Default expansion
        let result = manager.try_expand(":hello");
        assert_eq!(result, Some("echo Hello world".to_string()));

        // With custom value
        let result = manager.try_expand(":hello name=Claude");
        assert_eq!(result, Some("echo Hello Claude".to_string()));

        // Non-snippet input
        let result = manager.try_expand("hello");
        assert!(result.is_none());
    }
}
