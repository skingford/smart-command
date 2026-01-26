//! SSH host completion provider
//!
//! Provides completions for SSH hosts from ~/.ssh/config and known_hosts.

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Provides SSH host completions from config files
pub struct SshHostProvider;

impl SshHostProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_ssh_config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".ssh").join("config"))
    }

    fn get_known_hosts_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".ssh").join("known_hosts"))
    }

    fn parse_ssh_config(&self) -> Vec<(String, Option<String>)> {
        // Returns (host_alias, hostname)
        let Some(config_path) = Self::get_ssh_config_path() else {
            return vec![];
        };

        let Ok(content) = fs::read_to_string(&config_path) else {
            return vec![];
        };

        let mut hosts = Vec::new();
        let mut current_host: Option<String> = None;
        let mut current_hostname: Option<String> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
            if parts.len() < 2 {
                continue;
            }

            let key = parts[0].to_lowercase();
            let value = parts[1].trim();

            match key.as_str() {
                "host" => {
                    // Save previous host
                    if let Some(host) = current_host.take() {
                        // Skip wildcards
                        if !host.contains('*') && !host.contains('?') {
                            hosts.push((host, current_hostname.take()));
                        }
                    }
                    current_host = Some(value.to_string());
                    current_hostname = None;
                }
                "hostname" => {
                    current_hostname = Some(value.to_string());
                }
                _ => {}
            }
        }

        // Don't forget the last host
        if let Some(host) = current_host {
            if !host.contains('*') && !host.contains('?') {
                hosts.push((host, current_hostname));
            }
        }

        hosts
    }

    fn parse_known_hosts(&self) -> Vec<String> {
        let Some(known_hosts_path) = Self::get_known_hosts_path() else {
            return vec![];
        };

        let Ok(content) = fs::read_to_string(&known_hosts_path) else {
            return vec![];
        };

        let mut hosts = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
                continue;
            }

            // Format: hostname[,hostname]... key-type key [comment]
            // or: |1|base64... (hashed hostnames, skip these)
            if line.starts_with('|') {
                continue;
            }

            if let Some(host_part) = line.split_whitespace().next() {
                // Handle multiple hosts separated by comma
                for host in host_part.split(',') {
                    // Remove port if present [host]:port
                    let host = host
                        .trim_start_matches('[')
                        .split(']')
                        .next()
                        .unwrap_or(host);

                    if !host.is_empty() && !host.starts_with('|') {
                        hosts.push(host.to_string());
                    }
                }
            }
        }

        hosts.sort();
        hosts.dedup();
        hosts
    }
}

impl Default for SshHostProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for SshHostProvider {
    fn name(&self) -> &str {
        "ssh_host"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if arg_position < 1 {
            return false;
        }

        // ssh <host>
        // scp user@<host>:path or <host>:path
        // rsync user@<host>:path or <host>:path
        // sftp <host>

        match cmd {
            "ssh" | "sftp" => {
                // First non-flag argument is the host
                let partial = &context.partial_input;
                !partial.starts_with('-')
            }
            "scp" | "rsync" => {
                // Could be completing host in user@host:path pattern
                let partial = &context.partial_input;
                // If it contains @, we're completing after the @
                partial.contains('@') || !partial.contains(':')
            }
            _ => false,
        }
    }

    fn complete(&self, partial: &str, context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let mut suggestions = Vec::new();

        // Handle user@host pattern for scp/rsync
        let (prefix, host_partial) = if partial.contains('@') {
            let parts: Vec<&str> = partial.splitn(2, '@').collect();
            (format!("{}@", parts[0]), *parts.get(1).unwrap_or(&""))
        } else {
            (String::new(), partial)
        };

        let host_partial_lower = host_partial.to_lowercase();

        // Add hosts from SSH config (higher priority)
        for (alias, hostname) in self.parse_ssh_config() {
            if alias.to_lowercase().starts_with(&host_partial_lower) {
                let desc = hostname
                    .map(|h| format!("-> {}", h))
                    .unwrap_or_else(|| "SSH config".to_string());

                let value = if context.command == "scp" || context.command == "rsync" {
                    format!("{}{}:", prefix, alias)
                } else {
                    format!("{}{}", prefix, alias)
                };

                suggestions.push(
                    ProviderSuggestion::new(value)
                        .with_description(desc)
                        .with_category("ssh-config")
                        .with_score(100),
                );
            }
        }

        // Add hosts from known_hosts (lower priority)
        for host in self.parse_known_hosts() {
            if host.to_lowercase().starts_with(&host_partial_lower) {
                let value = if context.command == "scp" || context.command == "rsync" {
                    format!("{}{}:", prefix, host)
                } else {
                    format!("{}{}", prefix, host)
                };

                // Skip if already added from config
                if !suggestions.iter().any(|s| s.value == value) {
                    suggestions.push(
                        ProviderSuggestion::new(value)
                            .with_description("known host")
                            .with_category("known-host")
                            .with_score(50),
                    );
                }
            }
        }

        suggestions
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(60)) // SSH config doesn't change often
    }

    fn priority(&self) -> i32 {
        70
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_host_provider_matches() {
        let provider = SshHostProvider::new();

        let ctx = ProviderContext::new(PathBuf::from("."), "ssh", vec![], "");
        assert!(provider.matches("ssh", 1, &ctx));

        let ctx2 = ProviderContext::new(PathBuf::from("."), "scp", vec![], "user@");
        assert!(provider.matches("scp", 1, &ctx2));
    }

    #[test]
    fn test_ssh_config_parsing() {
        let provider = SshHostProvider::new();
        // This will just check the method doesn't crash
        let _ = provider.parse_ssh_config();
        let _ = provider.parse_known_hosts();
    }
}
