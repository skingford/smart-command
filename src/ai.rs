//! AI-Assisted Suggestions
//!
//! Provides local intelligence for typo correction, command prediction,
//! and optional LLM integration for natural language commands.

#![allow(dead_code)]

use strsim::levenshtein;
use std::collections::HashMap;

/// Typo correction using edit distance
pub struct TypoCorrector {
    /// Known command names
    commands: Vec<String>,
    /// Maximum edit distance to consider
    max_distance: usize,
}

impl TypoCorrector {
    pub fn new(commands: Vec<String>) -> Self {
        Self {
            commands,
            max_distance: 2,
        }
    }

    /// Find corrections for a potentially misspelled command
    pub fn suggest(&self, input: &str) -> Vec<(String, usize)> {
        let mut suggestions = Vec::new();

        for cmd in &self.commands {
            let distance = levenshtein(input, cmd);
            if distance <= self.max_distance && distance > 0 {
                suggestions.push((cmd.clone(), distance));
            }
        }

        // Sort by distance (closest first)
        suggestions.sort_by_key(|(_, d)| *d);

        // Return top 3 suggestions
        suggestions.truncate(3);
        suggestions
    }

    /// Get the best correction if confidence is high enough
    pub fn best_correction(&self, input: &str) -> Option<String> {
        let suggestions = self.suggest(input);

        // Only return if there's exactly one suggestion with distance 1
        if suggestions.len() == 1 && suggestions[0].1 == 1 {
            return Some(suggestions[0].0.clone());
        }

        None
    }

    /// Format a "did you mean" message
    pub fn did_you_mean(&self, input: &str) -> Option<String> {
        let suggestions = self.suggest(input);

        if suggestions.is_empty() {
            return None;
        }

        if suggestions.len() == 1 {
            Some(format!("Did you mean '{}'?", suggestions[0].0))
        } else {
            let names: Vec<_> = suggestions.iter().map(|(s, _)| s.as_str()).collect();
            Some(format!("Did you mean one of: {}?", names.join(", ")))
        }
    }
}

/// Command prediction based on n-gram patterns
pub struct CommandPredictor {
    /// Bigram frequencies: (prev_cmd -> next_cmd -> count)
    bigrams: HashMap<String, HashMap<String, u32>>,
    /// Most frequent commands overall
    frequent_commands: HashMap<String, u32>,
}

impl CommandPredictor {
    pub fn new() -> Self {
        Self {
            bigrams: HashMap::new(),
            frequent_commands: HashMap::new(),
        }
    }

    /// Record a command execution
    pub fn record(&mut self, command: &str, previous_command: Option<&str>) {
        // Update frequency
        *self.frequent_commands.entry(command.to_string()).or_insert(0) += 1;

        // Update bigram
        if let Some(prev) = previous_command {
            let next_counts = self.bigrams.entry(prev.to_string()).or_default();
            *next_counts.entry(command.to_string()).or_insert(0) += 1;
        }
    }

    /// Predict likely next command based on previous command
    pub fn predict(&self, previous_command: Option<&str>) -> Vec<(String, f32)> {
        let mut predictions = Vec::new();

        // If we have a previous command, use bigrams
        if let Some(prev) = previous_command {
            if let Some(next_counts) = self.bigrams.get(prev) {
                let total: u32 = next_counts.values().sum();
                for (cmd, count) in next_counts {
                    let probability = *count as f32 / total as f32;
                    if probability >= 0.1 {
                        // At least 10% probability
                        predictions.push((cmd.clone(), probability));
                    }
                }
            }
        }

        // If no bigram predictions, fall back to frequent commands
        if predictions.is_empty() {
            let total: u32 = self.frequent_commands.values().sum();
            if total > 0 {
                for (cmd, count) in &self.frequent_commands {
                    let probability = *count as f32 / total as f32;
                    if probability >= 0.05 {
                        // At least 5% probability
                        predictions.push((cmd.clone(), probability));
                    }
                }
            }
        }

        // Sort by probability
        predictions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        predictions.truncate(5);
        predictions
    }

    /// Get the most likely next command with confidence
    pub fn most_likely(&self, previous_command: Option<&str>) -> Option<(String, f32)> {
        let predictions = self.predict(previous_command);

        // Only return if probability is high enough
        predictions
            .into_iter()
            .find(|(_, prob)| *prob >= 0.3)
    }
}

/// Smart defaults based on common flag combinations
pub struct SmartDefaults {
    /// Common flag combinations per command
    common_flags: HashMap<String, Vec<String>>,
}

impl SmartDefaults {
    pub fn new() -> Self {
        let mut common_flags = HashMap::new();

        // Pre-populate with common flag combinations
        common_flags.insert(
            "git commit".to_string(),
            vec!["-m".to_string()],
        );
        common_flags.insert(
            "git push".to_string(),
            vec!["-u".to_string(), "origin".to_string()],
        );
        common_flags.insert(
            "git log".to_string(),
            vec!["--oneline".to_string(), "-10".to_string()],
        );
        common_flags.insert(
            "ls".to_string(),
            vec!["-la".to_string()],
        );
        common_flags.insert(
            "grep".to_string(),
            vec!["-r".to_string(), "-n".to_string()],
        );
        common_flags.insert(
            "rm".to_string(),
            vec!["-rf".to_string()],
        );
        common_flags.insert(
            "cp".to_string(),
            vec!["-r".to_string()],
        );
        common_flags.insert(
            "docker run".to_string(),
            vec!["--rm".to_string(), "-it".to_string()],
        );
        common_flags.insert(
            "docker build".to_string(),
            vec!["-t".to_string()],
        );
        common_flags.insert(
            "cargo build".to_string(),
            vec!["--release".to_string()],
        );
        common_flags.insert(
            "npm install".to_string(),
            vec!["--save-dev".to_string()],
        );

        Self { common_flags }
    }

    /// Record flag usage
    pub fn record(&mut self, command: &str, flags: &[String]) {
        if flags.is_empty() {
            return;
        }

        let entry = self.common_flags.entry(command.to_string()).or_default();
        for flag in flags {
            if !entry.contains(flag) {
                entry.push(flag.clone());
            }
        }

        // Keep only top 5 flags
        if entry.len() > 5 {
            entry.truncate(5);
        }
    }

    /// Get common flags for a command
    pub fn get_flags(&self, command: &str) -> Option<&[String]> {
        self.common_flags.get(command).map(|v| v.as_slice())
    }

    /// Suggest flags for a command
    pub fn suggest_flags(&self, command: &str) -> String {
        if let Some(flags) = self.get_flags(command) {
            flags.join(" ")
        } else {
            String::new()
        }
    }
}

/// Natural language command templates
pub struct NaturalLanguageTemplates {
    templates: Vec<(Vec<&'static str>, &'static str, &'static str)>,
}

impl NaturalLanguageTemplates {
    pub fn new() -> Self {
        Self {
            templates: vec![
                // (trigger words, command, description)
                (vec!["large", "files", "big"], "find . -size +100M -type f", "Find files larger than 100MB"),
                (vec!["compress", "folder", "zip"], "tar -czvf archive.tar.gz ./", "Compress folder to tar.gz"),
                (vec!["decompress", "extract", "unzip"], "tar -xzvf", "Extract tar.gz archive"),
                (vec!["disk", "space", "usage"], "du -sh *", "Show disk usage of files"),
                (vec!["free", "space", "available"], "df -h", "Show available disk space"),
                (vec!["running", "processes"], "ps aux", "Show running processes"),
                (vec!["kill", "process"], "pkill", "Kill process by name"),
                (vec!["memory", "usage"], "free -h", "Show memory usage"),
                (vec!["network", "connections"], "netstat -tuln", "Show network connections"),
                (vec!["open", "ports"], "lsof -i -P -n | grep LISTEN", "Show open ports"),
                (vec!["git", "history", "log"], "git log --oneline -20", "Show recent git commits"),
                (vec!["git", "changes", "modified"], "git status", "Show git status"),
                (vec!["undo", "last", "commit"], "git reset --soft HEAD~1", "Undo last commit"),
                (vec!["docker", "running", "containers"], "docker ps", "Show running containers"),
                (vec!["docker", "all", "images"], "docker images", "Show all docker images"),
                (vec!["empty", "file", "truncate"], "truncate -s 0", "Empty/truncate a file"),
                (vec!["count", "lines", "file"], "wc -l", "Count lines in file"),
                (vec!["search", "text", "grep"], "grep -rn", "Search for text recursively"),
                (vec!["replace", "text", "sed"], "sed -i 's/old/new/g'", "Replace text in file"),
                (vec!["permission", "executable"], "chmod +x", "Make file executable"),
                (vec!["ownership", "chown"], "chown -R $USER:$USER", "Change ownership"),
                (vec!["download", "file", "url"], "curl -O", "Download file from URL"),
                (vec!["http", "server", "python"], "python -m http.server 8000", "Start HTTP server"),
                (vec!["json", "format", "pretty"], "jq '.'", "Pretty print JSON"),
                (vec!["base64", "encode"], "base64", "Encode to base64"),
                (vec!["base64", "decode"], "base64 -d", "Decode from base64"),
            ],
        }
    }

    /// Find matching templates for a natural language query
    pub fn find(&self, query: &str) -> Vec<(&'static str, &'static str)> {
        let query_lower = query.to_lowercase();
        let words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut matches = Vec::new();

        for (triggers, command, description) in &self.templates {
            let match_count = triggers.iter().filter(|t| words.contains(t)).count();
            if match_count >= 2 || (triggers.len() == 2 && match_count >= 1) {
                matches.push((*command, *description));
            }
        }

        matches
    }

    /// Try to translate a natural language query to a command
    pub fn translate(&self, query: &str) -> Option<(&'static str, &'static str)> {
        let matches = self.find(query);
        matches.into_iter().next()
    }
}

impl Default for NaturalLanguageTemplates {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SmartDefaults {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for CommandPredictor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typo_correction() {
        let commands = vec![
            "git".to_string(),
            "grep".to_string(),
            "curl".to_string(),
            "cargo".to_string(),
        ];
        let corrector = TypoCorrector::new(commands);

        // Test simple typo
        let suggestions = corrector.suggest("gti");
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].0, "git");

        // Test another typo
        let suggestions = corrector.suggest("greo");
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].0, "grep");
    }

    #[test]
    fn test_did_you_mean() {
        let commands = vec!["commit".to_string(), "push".to_string()];
        let corrector = TypoCorrector::new(commands);

        let message = corrector.did_you_mean("comit");
        assert!(message.is_some());
        assert!(message.unwrap().contains("commit"));
    }

    #[test]
    fn test_command_prediction() {
        let mut predictor = CommandPredictor::new();

        // Record some commands
        predictor.record("git add", None);
        predictor.record("git commit", Some("git add"));
        predictor.record("git commit", Some("git add"));
        predictor.record("git push", Some("git commit"));

        // Predict after "git add"
        let predictions = predictor.predict(Some("git add"));
        assert!(!predictions.is_empty());
        assert_eq!(predictions[0].0, "git commit");
    }

    #[test]
    fn test_smart_defaults() {
        let defaults = SmartDefaults::new();

        let flags = defaults.get_flags("git commit");
        assert!(flags.is_some());
        assert!(flags.unwrap().contains(&"-m".to_string()));
    }

    #[test]
    fn test_natural_language() {
        let templates = NaturalLanguageTemplates::new();

        // Test finding large files
        let matches = templates.find("show me large files");
        assert!(!matches.is_empty());
        assert!(matches[0].0.contains("find"));

        // Test disk space
        let matches = templates.find("check disk space usage");
        assert!(!matches.is_empty());
        assert!(matches[0].0.contains("du"));
    }

    #[test]
    fn test_natural_language_translate() {
        let templates = NaturalLanguageTemplates::new();

        let result = templates.translate("compress this folder");
        assert!(result.is_some());
        assert!(result.unwrap().0.contains("tar"));
    }
}
