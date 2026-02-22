//! Process completion provider
//!
//! Provides completions for process names and PIDs for kill/pkill commands.

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;
use std::time::Duration;

/// Provides process name and PID completions
pub struct ProcessProvider;

impl ProcessProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_processes(&self) -> Vec<(String, String, String)> {
        // Returns (pid, name, user)
        #[cfg(target_os = "macos")]
        let output = Command::new("ps")
            .args(["-axo", "pid,comm,user"])
            .output()
            .ok();

        #[cfg(target_os = "linux")]
        let output = Command::new("ps")
            .args(["-eo", "pid,comm,user"])
            .output()
            .ok();

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let output: Option<std::process::Output> = None;

        output
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|output| {
                output
                    .lines()
                    .skip(1) // Skip header
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 {
                            Some((
                                parts[0].to_string(), // PID
                                parts[1].to_string(), // Command name
                                parts[2].to_string(), // User
                            ))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_process_names(&self) -> Vec<String> {
        let processes = self.get_processes();
        let mut names: Vec<String> = processes.into_iter().map(|(_, name, _)| name).collect();
        names.sort();
        names.dedup();
        names
    }
}

impl Default for ProcessProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for ProcessProvider {
    fn name(&self) -> &str {
        "process"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        // kill <pid>
        // kill -9 <pid>
        // pkill <name>
        // killall <name>

        if arg_position >= 1 {
            // kill command - complete PIDs or process names
            if cmd == "kill" {
                // After -9 or other signal flags
                let last_arg = context.args.last().map(|s| s.as_str()).unwrap_or("");
                if last_arg.starts_with('-') && !context.partial_input.is_empty() {
                    return true;
                }
                if !context.partial_input.starts_with('-') {
                    return true;
                }
            }

            // pkill/killall - complete process names
            if cmd == "pkill" || cmd == "killall" {
                return true;
            }
        }

        false
    }

    fn complete(&self, partial: &str, context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let cmd = &context.command;

        if cmd == "pkill" || cmd == "killall" {
            // Complete process names
            let names = self.get_process_names();
            let partial_lower = partial.to_lowercase();

            return names
                .into_iter()
                .filter(|name| name.to_lowercase().starts_with(&partial_lower))
                .map(|name| {
                    ProviderSuggestion::new(&name)
                        .with_description("process")
                        .with_category("process-name")
                        .with_score(100)
                })
                .collect();
        }

        // For kill, complete PIDs
        if cmd == "kill" {
            let processes = self.get_processes();

            // If partial looks like a number, suggest PIDs
            if partial.chars().all(|c| c.is_ascii_digit()) || partial.is_empty() {
                return processes
                    .into_iter()
                    .filter(|(pid, _, _)| pid.starts_with(partial))
                    .map(|(pid, name, user)| {
                        ProviderSuggestion::new(&pid)
                            .with_description(format!("{} ({})", name, user))
                            .with_category("pid")
                            .with_score(100)
                    })
                    .collect();
            }

            // Otherwise, suggest by process name and show PID
            let partial_lower = partial.to_lowercase();
            return processes
                .into_iter()
                .filter(|(_, name, _)| name.to_lowercase().starts_with(&partial_lower))
                .map(|(pid, name, user)| {
                    ProviderSuggestion::new(&pid)
                        .with_description(format!("{} ({})", name, user))
                        .with_category("pid")
                        .with_score(100)
                })
                .collect();
        }

        vec![]
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(2))
    }

    fn priority(&self) -> i32 {
        60
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_process_provider_matches() {
        let provider = ProcessProvider::new();

        let ctx = ProviderContext::new(PathBuf::from("."), "pkill", vec![], "");
        assert!(provider.matches("pkill", 1, &ctx));

        let ctx2 = ProviderContext::new(PathBuf::from("."), "kill", vec![], "123");
        assert!(provider.matches("kill", 1, &ctx2));
    }
}
