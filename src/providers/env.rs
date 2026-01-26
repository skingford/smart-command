//! Environment variable completion provider
//!
//! Provides completions for $VAR environment variables.

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
use std::env;
use std::time::Duration;

/// Provides environment variable completions
pub struct EnvVarProvider;

impl EnvVarProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_env_vars(&self) -> Vec<(String, String)> {
        env::vars().collect()
    }
}

impl Default for EnvVarProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for EnvVarProvider {
    fn name(&self) -> &str {
        "env_var"
    }

    fn matches(&self, _cmd: &str, _arg_position: usize, context: &ProviderContext) -> bool {
        // Match when typing $VAR or ${VAR
        context.partial_input.starts_with('$')
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let vars = self.get_env_vars();

        // Handle ${ prefix
        let (prefix, var_partial) = if partial.starts_with("${") {
            ("${", &partial[2..])
        } else if partial.starts_with('$') {
            ("$", &partial[1..])
        } else {
            return vec![];
        };

        let var_partial_upper = var_partial.to_uppercase();

        vars.into_iter()
            .filter(|(name, _)| name.starts_with(&var_partial_upper))
            .map(|(name, value)| {
                // Truncate long values for display
                let display_value = if value.len() > 50 {
                    format!("{}...", &value[..47])
                } else {
                    value
                };

                let completion = if prefix == "${" {
                    format!("${{{}}}", name)
                } else {
                    format!("${}", name)
                };

                // Common vars get higher scores
                let score = match name.as_str() {
                    "HOME" | "PATH" | "USER" | "PWD" | "SHELL" => 100,
                    s if s.starts_with("LANG") || s.starts_with("LC_") => 80,
                    _ => 50,
                };

                ProviderSuggestion::new(completion)
                    .with_description(display_value)
                    .with_category("env")
                    .with_score(score)
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        // Environment can change frequently
        Some(Duration::from_secs(1))
    }

    fn priority(&self) -> i32 {
        50 // Lower priority, only when $ is typed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_env_var_provider_matches() {
        let provider = EnvVarProvider::new();

        let ctx = ProviderContext::new(
            PathBuf::from("."),
            "echo",
            vec![],
            "$HO",
        );
        assert!(provider.matches("echo", 0, &ctx));

        let ctx2 = ProviderContext::new(
            PathBuf::from("."),
            "echo",
            vec![],
            "hello",
        );
        assert!(!provider.matches("echo", 0, &ctx2));
    }

    #[test]
    fn test_env_var_completion() {
        let provider = EnvVarProvider::new();
        let ctx = ProviderContext::new(
            PathBuf::from("."),
            "echo",
            vec![],
            "$HO",
        );

        let results = provider.complete("$HO", &ctx);
        // HOME should be in results on most systems
        assert!(results.iter().any(|s| s.value == "$HOME"));
    }
}
