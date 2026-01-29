//! Export variable completion provider
//!
//! Provides intelligent completions for export commands:
//! - JAVA_HOME, MAVEN_HOME, etc. with automatic path detection
//! - PATH variable with suggestions based on *_HOME variables
//! - Natural language variable name recognition

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Known software types and their typical installation paths
struct SoftwareInfo {
    /// Paths to search for installations
    search_paths: Vec<&'static str>,
    /// Pattern to match directories (e.g., "java", "jdk")
    dir_patterns: Vec<&'static str>,
    /// Subdirectory that should exist to confirm valid installation
    validation_subdir: &'static str,
}

/// Provides completions for export commands
pub struct ExportProvider {
    /// Known software configurations
    known_software: HashMap<&'static str, SoftwareInfo>,
}

impl ExportProvider {
    pub fn new() -> Self {
        let mut known_software = HashMap::new();

        // Java
        known_software.insert(
            "java",
            SoftwareInfo {
                search_paths: vec![
                    "/Library/Java/JavaVirtualMachines",
                    "/usr/lib/jvm",
                    "/opt/java",
                    "/opt/jdk",
                ],
                dir_patterns: vec!["jdk", "java", "openjdk", "graalvm", "zulu", "temurin"],
                validation_subdir: "bin/java",
            },
        );

        // Maven
        known_software.insert(
            "maven",
            SoftwareInfo {
                search_paths: vec!["/opt/maven", "/usr/local/opt/maven", "/opt/apache-maven"],
                dir_patterns: vec!["maven", "apache-maven"],
                validation_subdir: "bin/mvn",
            },
        );

        // Gradle
        known_software.insert(
            "gradle",
            SoftwareInfo {
                search_paths: vec!["/opt/gradle", "/usr/local/opt/gradle"],
                dir_patterns: vec!["gradle"],
                validation_subdir: "bin/gradle",
            },
        );

        // Node.js
        known_software.insert(
            "node",
            SoftwareInfo {
                search_paths: vec!["/usr/local", "/opt/node", "/opt/nodejs"],
                dir_patterns: vec!["node", "nodejs"],
                validation_subdir: "bin/node",
            },
        );

        // Go
        known_software.insert(
            "go",
            SoftwareInfo {
                search_paths: vec!["/usr/local/go", "/opt/go", "/usr/lib/go"],
                dir_patterns: vec!["go", "golang"],
                validation_subdir: "bin/go",
            },
        );

        // Rust
        known_software.insert(
            "rust",
            SoftwareInfo {
                search_paths: vec![],
                dir_patterns: vec!["rust", "rustup", "cargo"],
                validation_subdir: "bin/rustc",
            },
        );

        // Python
        known_software.insert(
            "python",
            SoftwareInfo {
                search_paths: vec![
                    "/usr/local/opt/python",
                    "/opt/python",
                    "/usr/local/Cellar/python",
                ],
                dir_patterns: vec!["python", "python3"],
                validation_subdir: "bin/python",
            },
        );

        // Hadoop
        known_software.insert(
            "hadoop",
            SoftwareInfo {
                search_paths: vec!["/opt/hadoop", "/usr/local/hadoop", "/usr/lib/hadoop"],
                dir_patterns: vec!["hadoop"],
                validation_subdir: "bin/hadoop",
            },
        );

        // Spark
        known_software.insert(
            "spark",
            SoftwareInfo {
                search_paths: vec!["/opt/spark", "/usr/local/spark"],
                dir_patterns: vec!["spark"],
                validation_subdir: "bin/spark-submit",
            },
        );

        // Scala
        known_software.insert(
            "scala",
            SoftwareInfo {
                search_paths: vec!["/opt/scala", "/usr/local/scala", "/usr/share/scala"],
                dir_patterns: vec!["scala"],
                validation_subdir: "bin/scala",
            },
        );

        // Android SDK
        known_software.insert(
            "android",
            SoftwareInfo {
                search_paths: vec![],
                dir_patterns: vec!["android", "Android"],
                validation_subdir: "platform-tools/adb",
            },
        );

        Self { known_software }
    }

    /// Recognize software type from variable name
    fn recognize_variable_type(&self, var_name: &str) -> Option<&'static str> {
        let name = var_name.to_uppercase();

        if name.contains("JAVA") || name == "JDK_HOME" || name == "JRE_HOME" {
            return Some("java");
        }
        if name.contains("MAVEN") || name == "M2_HOME" || name == "MVN_HOME" {
            return Some("maven");
        }
        if name.contains("GRADLE") {
            return Some("gradle");
        }
        if name.contains("NODE") || name.contains("NVM") || name == "NPM_HOME" {
            return Some("node");
        }
        if name.contains("GO") && (name.contains("ROOT") || name.contains("HOME")) {
            return Some("go");
        }
        if name.contains("RUST") || name.contains("CARGO") {
            return Some("rust");
        }
        if name.contains("PYTHON") || name.contains("PYENV") || name == "VIRTUAL_ENV" {
            return Some("python");
        }
        if name.contains("HADOOP") {
            return Some("hadoop");
        }
        if name.contains("SPARK") {
            return Some("spark");
        }
        if name.contains("SCALA") {
            return Some("scala");
        }
        if name.contains("ANDROID") {
            return Some("android");
        }

        None
    }

    /// Find installations for a software type
    fn find_installations(&self, software_type: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();

        let info = match self.known_software.get(software_type) {
            Some(info) => info,
            None => return results,
        };

        // Check SDKMAN candidates
        if let Some(home) = dirs::home_dir() {
            let sdkman_path = home.join(".sdkman/candidates").join(software_type);
            if sdkman_path.exists() {
                if let Ok(entries) = std::fs::read_dir(&sdkman_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.join(info.validation_subdir).exists() {
                            let version = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if version != "current" {
                                results.push((
                                    path.to_string_lossy().to_string(),
                                    format!("SDKMAN {}", version),
                                ));
                            }
                        }
                    }
                }
            }

            // Check asdf
            let asdf_path = home.join(".asdf/installs").join(software_type);
            if asdf_path.exists() {
                if let Ok(entries) = std::fs::read_dir(&asdf_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let version = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            results
                                .push((path.to_string_lossy().to_string(), format!("asdf {}", version)));
                        }
                    }
                }
            }
        }

        // Search standard paths
        for search_path in &info.search_paths {
            let path = Path::new(search_path);

            // Direct path check
            if path.exists() && path.join(info.validation_subdir).exists() {
                results.push((
                    search_path.to_string(),
                    "System installation".to_string(),
                ));
            }

            // Check subdirectories
            if path.exists() && path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        if entry_path.is_dir() {
                            let name = entry_path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_lowercase())
                                .unwrap_or_default();

                            // Check if directory matches pattern
                            let matches_pattern = info.dir_patterns.iter().any(|p| name.contains(p));
                            if matches_pattern {
                                // For Java, check Contents/Home on macOS
                                let java_home = entry_path.join("Contents/Home");
                                let actual_path = if java_home.exists() {
                                    java_home
                                } else {
                                    entry_path.clone()
                                };

                                if actual_path.join(info.validation_subdir).exists() {
                                    let version = entry_path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default();
                                    results.push((
                                        actual_path.to_string_lossy().to_string(),
                                        version,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        results
    }

    /// Get PATH suggestions based on defined *_HOME variables
    fn get_path_suggestions(&self) -> Vec<(String, String)> {
        let mut suggestions = Vec::new();
        let mut home_vars = Vec::new();

        // Collect all *_HOME variables
        for (key, value) in env::vars() {
            if (key.ends_with("_HOME") || key == "GOROOT" || key == "CARGO_HOME")
                && !value.is_empty()
            {
                let bin_path = PathBuf::from(&value).join("bin");
                if bin_path.exists() {
                    home_vars.push((key.clone(), value));
                }
            }
        }

        // Generate suggestions
        if !home_vars.is_empty() {
            // Suggest adding all bin directories
            let mut all_bins: Vec<String> = home_vars
                .iter()
                .map(|(k, _)| format!("${}/bin", k))
                .collect();
            all_bins.insert(0, "$PATH".to_string());

            suggestions.push((
                all_bins.join(":"),
                format!("Add all {} tool bins", home_vars.len()),
            ));

            // Individual suggestions
            for (key, _value) in &home_vars {
                suggestions.push((
                    format!("$PATH:${}/bin", key),
                    format!("Add {} to PATH", key),
                ));
            }
        }

        // Common PATH additions
        let common_paths = [
            ("$HOME/.local/bin", "User local binaries"),
            ("$HOME/bin", "User binaries"),
            ("/usr/local/bin", "Local system binaries"),
            ("$HOME/.cargo/bin", "Rust/Cargo binaries"),
            ("$HOME/go/bin", "Go binaries"),
            ("$HOME/.npm-global/bin", "NPM global binaries"),
        ];

        for (path, desc) in common_paths {
            // Expand $HOME and check if path exists
            let expanded = if path.starts_with("$HOME") {
                dirs::home_dir()
                    .map(|h| path.replace("$HOME", &h.to_string_lossy()))
                    .unwrap_or_else(|| path.to_string())
            } else {
                path.to_string()
            };

            if Path::new(&expanded).exists() {
                suggestions.push((format!("$PATH:{}", path), desc.to_string()));
            }
        }

        suggestions
    }

    /// Parse export command to extract variable name and partial value
    fn parse_export_command<'a>(&self, line: &'a str) -> Option<(&'a str, &'a str)> {
        let line = line.trim();

        // Handle: export VAR=value
        if line.starts_with("export ") {
            let rest = &line[7..].trim_start();
            if let Some(eq_pos) = rest.find('=') {
                let var_name = &rest[..eq_pos];
                let partial_value = &rest[eq_pos + 1..];
                return Some((var_name, partial_value));
            }
        }

        None
    }
}

impl Default for ExportProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for ExportProvider {
    fn name(&self) -> &str {
        "export"
    }

    fn matches(&self, cmd: &str, _arg_position: usize, context: &ProviderContext) -> bool {
        // Match export command with = sign
        cmd == "export" && context.partial_input.contains('=')
    }

    fn complete(&self, _partial: &str, context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let mut suggestions = Vec::new();

        // Reconstruct the full command line
        let full_line = format!("export {}", context.args.join(" "));

        if let Some((var_name, partial_value)) = self.parse_export_command(&full_line) {
            // Special handling for PATH
            if var_name.to_uppercase() == "PATH" {
                for (value, desc) in self.get_path_suggestions() {
                    if partial_value.is_empty() || value.starts_with(partial_value) {
                        suggestions.push(
                            ProviderSuggestion::new(value)
                                .with_description(desc)
                                .with_category("path")
                                .with_score(100),
                        );
                    }
                }
                return suggestions;
            }

            // Try to recognize variable type
            if let Some(software_type) = self.recognize_variable_type(var_name) {
                let installations = self.find_installations(software_type);

                for (path, desc) in installations {
                    if partial_value.is_empty() || path.starts_with(partial_value) {
                        suggestions.push(
                            ProviderSuggestion::new(path)
                                .with_description(desc)
                                .with_category(software_type)
                                .with_score(90),
                        );
                    }
                }
            }

            // If no specific suggestions, offer current environment value
            if suggestions.is_empty() {
                if let Ok(current) = env::var(var_name) {
                    if partial_value.is_empty() || current.starts_with(partial_value) {
                        suggestions.push(
                            ProviderSuggestion::new(current)
                                .with_description("Current value")
                                .with_category("env")
                                .with_score(50),
                        );
                    }
                }
            }
        }

        suggestions
    }

    fn cache_ttl(&self) -> Option<Duration> {
        // Filesystem scanning can be slow, cache for longer
        Some(Duration::from_secs(30))
    }

    fn priority(&self) -> i32 {
        60 // Higher priority for export command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_provider_creation() {
        let provider = ExportProvider::new();
        assert!(!provider.known_software.is_empty());
    }

    #[test]
    fn test_recognize_java() {
        let provider = ExportProvider::new();
        assert_eq!(provider.recognize_variable_type("JAVA_HOME"), Some("java"));
        assert_eq!(provider.recognize_variable_type("JDK_HOME"), Some("java"));
    }

    #[test]
    fn test_recognize_maven() {
        let provider = ExportProvider::new();
        assert_eq!(provider.recognize_variable_type("MAVEN_HOME"), Some("maven"));
        assert_eq!(provider.recognize_variable_type("M2_HOME"), Some("maven"));
    }

    #[test]
    fn test_recognize_go() {
        let provider = ExportProvider::new();
        assert_eq!(provider.recognize_variable_type("GOROOT"), Some("go"));
        assert_eq!(provider.recognize_variable_type("GO_HOME"), Some("go"));
    }

    #[test]
    fn test_parse_export_command() {
        let provider = ExportProvider::new();

        let result = provider.parse_export_command("export JAVA_HOME=/opt/java");
        assert_eq!(result, Some(("JAVA_HOME", "/opt/java")));

        let result = provider.parse_export_command("export PATH=");
        assert_eq!(result, Some(("PATH", "")));
    }

    #[test]
    fn test_export_provider_matches() {
        let provider = ExportProvider::new();

        let ctx = ProviderContext::new(
            PathBuf::from("."),
            "export",
            vec!["JAVA_HOME=".to_string()],
            "JAVA_HOME=",
        );
        assert!(provider.matches("export", 1, &ctx));

        let ctx2 = ProviderContext::new(
            PathBuf::from("."),
            "export",
            vec!["JAVA_HOME".to_string()],
            "JAVA_HOME",
        );
        assert!(!provider.matches("export", 1, &ctx2));
    }
}
