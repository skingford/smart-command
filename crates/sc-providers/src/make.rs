//! Make target completion provider
//!
//! Parses Makefile to provide completion for make targets.

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Provider for make targets from Makefile
pub struct MakeTargetProvider;

impl MakeTargetProvider {
    pub fn new() -> Self {
        Self
    }

    /// Parse Makefile and extract targets
    fn get_targets(&self, cwd: &Path) -> Vec<(String, Option<String>)> {
        let makefile_paths = ["Makefile", "makefile", "GNUmakefile"];

        for filename in &makefile_paths {
            let path = cwd.join(filename);
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    return self.parse_makefile(&content);
                }
            }
        }

        Vec::new()
    }

    /// Parse Makefile content to extract targets
    fn parse_makefile(&self, content: &str) -> Vec<(String, Option<String>)> {
        let mut targets = Vec::new();
        let mut current_comment: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            // Capture comments that might describe the next target
            if trimmed.starts_with('#') {
                let comment = trimmed.trim_start_matches('#').trim();
                if !comment.is_empty() {
                    current_comment = Some(comment.to_string());
                }
                continue;
            }

            // Skip empty lines but preserve comment for next target
            if trimmed.is_empty() {
                continue;
            }

            // Match target definitions (target: dependencies)
            if let Some(colon_pos) = trimmed.find(':') {
                // Skip assignments (:=) and double colons (::)
                if colon_pos > 0 {
                    let after_colon = trimmed.get(colon_pos + 1..);
                    if after_colon.map(|s| s.starts_with('=')).unwrap_or(false) {
                        current_comment = None;
                        continue;
                    }
                }

                let target_part = &trimmed[..colon_pos];

                // Skip pattern rules (%)
                if target_part.contains('%') {
                    current_comment = None;
                    continue;
                }

                // Skip special targets
                if target_part.starts_with('.') {
                    current_comment = None;
                    continue;
                }

                // Handle multiple targets on same line
                for target in target_part.split_whitespace() {
                    let target = target.trim();
                    if !target.is_empty() && !target.contains('$') {
                        targets.push((target.to_string(), current_comment.clone()));
                    }
                }

                current_comment = None;
            } else {
                // Non-target line, clear comment
                current_comment = None;
            }
        }

        // Deduplicate
        targets.sort_by(|a, b| a.0.cmp(&b.0));
        targets.dedup_by(|a, b| a.0 == b.0);

        targets
    }
}

impl Default for MakeTargetProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for MakeTargetProvider {
    fn name(&self) -> &str {
        "make_target"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        // Match for 'make' command
        if cmd != "make" && cmd != "gmake" {
            return false;
        }

        // Don't complete if we're providing a flag value
        if let Some(prev) = context.args.last() {
            if prev == "-f" || prev == "--file" || prev == "-C" || prev == "--directory" {
                return false;
            }
        }

        // Complete targets for any argument position
        let _ = arg_position; // Used for potential future filtering
        true
    }

    fn complete(&self, partial: &str, context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let targets = self.get_targets(&context.cwd);

        targets
            .into_iter()
            .filter(|(name, _)| {
                partial.is_empty() || name.to_lowercase().starts_with(&partial.to_lowercase())
            })
            .map(|(name, description)| {
                let mut suggestion = ProviderSuggestion::new(&name)
                    .with_category("target")
                    .with_score(50);

                if let Some(desc) = description {
                    suggestion = suggestion.with_description(desc);
                }

                suggestion
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        // Cache for 10 seconds (Makefile doesn't change often)
        Some(Duration::from_secs(10))
    }

    fn priority(&self) -> i32 {
        10 // Higher priority for make targets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_makefile() {
        let provider = MakeTargetProvider::new();

        let content = r#"
# Build the project
build:
	cargo build

# Run tests
test: build
	cargo test

# Clean build artifacts
clean:
	rm -rf target

.PHONY: build test clean
"#;

        let targets = provider.parse_makefile(content);
        let names: Vec<_> = targets.iter().map(|(n, _)| n.as_str()).collect();

        assert!(names.contains(&"build"));
        assert!(names.contains(&"test"));
        assert!(names.contains(&"clean"));
        assert!(!names.contains(&".PHONY")); // Special targets excluded
    }

    #[test]
    fn test_parse_with_comments() {
        let provider = MakeTargetProvider::new();

        let content = r#"
# This builds everything
all: build test

build:
	echo "building"
"#;

        let targets = provider.parse_makefile(content);

        let all_target = targets.iter().find(|(n, _)| n == "all");
        assert!(all_target.is_some());
        assert!(all_target.unwrap().1.is_some());
    }

    #[test]
    fn test_skip_variables() {
        let provider = MakeTargetProvider::new();

        let content = r#"
CC := gcc
CFLAGS := -Wall

build:
	$(CC) $(CFLAGS) main.c
"#;

        let targets = provider.parse_makefile(content);
        let names: Vec<_> = targets.iter().map(|(n, _)| n.as_str()).collect();

        assert!(names.contains(&"build"));
        assert!(!names.contains(&"CC"));
        assert!(!names.contains(&"CFLAGS"));
    }
}
