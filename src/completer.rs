use crate::command_def::CommandSpec;
use crate::context::tracker;
use crate::definitions;
use crate::providers::{self, ProviderContext, ProviderSuggestion};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use reedline::{Completer, Span, Suggestion};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub struct SmartCompleter {
    commands: HashMap<String, CommandSpec>,
    current_lang: Arc<RwLock<String>>,
}

impl Clone for SmartCompleter {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            current_lang: self.current_lang.clone(),
        }
    }
}

impl SmartCompleter {
    pub fn new(loaded_commands: Vec<CommandSpec>, current_lang: Arc<RwLock<String>>) -> Self {
        let mut commands = HashMap::new();
        for cmd in loaded_commands {
            commands.insert(cmd.name.clone(), cmd);
        }

        for cmd in definitions::other_specs() {
            if !commands.contains_key(&cmd.name) {
                commands.insert(cmd.name.clone(), cmd);
            }
        }

        Self {
            commands,
            current_lang,
        }
    }

    /// Get examples for a specific command path (e.g., "git", "git commit")
    pub fn get_examples(&self, command_path: &str, lang: &str) -> Vec<(String, String)> {
        let parts: Vec<&str> = command_path.split_whitespace().collect();
        if parts.is_empty() {
            return vec![];
        }

        // Find root command
        let root_name = parts[0];
        let root_spec = match self.commands.get(root_name) {
            Some(spec) => spec,
            None => return vec![],
        };

        // Descend along path
        let mut current = root_spec;
        for part in &parts[1..] {
            match current.subcommands.iter().find(|s| s.name == *part) {
                Some(sub) => current = sub,
                None => return vec![],
            }
        }

        // Return examples
        current
            .examples
            .iter()
            .map(|e| (e.cmd.clone(), e.scenario.get(lang).to_string()))
            .collect()
    }

    /// Get all commands that have examples
    pub fn get_commands_with_examples(&self) -> Vec<String> {
        let mut result = Vec::new();
        for (name, cmd) in &self.commands {
            Self::collect_commands_with_examples(cmd, name, &mut result);
        }
        result.sort();
        result
    }

    fn collect_commands_with_examples(cmd: &CommandSpec, path: &str, result: &mut Vec<String>) {
        if !cmd.examples.is_empty() {
            result.push(path.to_string());
        }
        for sub in &cmd.subcommands {
            let sub_path = format!("{} {}", path, sub.name);
            Self::collect_commands_with_examples(sub, &sub_path, result);
        }
    }

    /// Search examples by query (returns command path, example cmd, scenario)
    pub fn search_examples(&self, query: &str, lang: &str) -> Vec<(String, String, String)> {
        let mut results: Vec<(i64, String, String, String)> = Vec::new();

        for (name, cmd) in &self.commands {
            self.search_examples_recursive(cmd, query, lang, name, &mut results);
        }

        // Sort by score
        results.sort_by(|a, b| b.0.cmp(&a.0));

        results
            .into_iter()
            .map(|(_, path, cmd, scenario)| (path, cmd, scenario))
            .collect()
    }

    fn search_examples_recursive(
        &self,
        cmd: &CommandSpec,
        query: &str,
        lang: &str,
        path: &str,
        results: &mut Vec<(i64, String, String, String)>,
    ) {
        for example in &cmd.examples {
            let scenario = example.scenario.get(lang);
            // Match against scenario or command
            let score_scenario = self.fuzzy_match(query, scenario);
            let score_cmd = self.fuzzy_match(query, &example.cmd);

            if let Some(score) = score_scenario.or(score_cmd) {
                results.push((
                    score,
                    path.to_string(),
                    example.cmd.clone(),
                    scenario.to_string(),
                ));
            }
        }

        for sub in &cmd.subcommands {
            let sub_path = format!("{} {}", path, sub.name);
            self.search_examples_recursive(sub, query, lang, &sub_path, results);
        }
    }

    /// True fuzzy match - allows non-contiguous character matching
    /// e.g., "cm" matches "commit", "gco" matches "git checkout"
    fn fuzzy_match(&self, input: &str, target: &str) -> Option<i64> {
        let matcher = SkimMatcherV2::default();
        matcher.fuzzy_match(target, input)
    }

    /// Prefix match for backward compatibility (faster for Tab completion)
    fn prefix_match(&self, input: &str, target: &str) -> bool {
        target.to_lowercase().starts_with(&input.to_lowercase())
    }

    /// Get combined flag suggestions based on common combos and available flags
    fn get_combo_suggestions(
        &self,
        spec: &CommandSpec,
        current_chain: &str,
        start_idx: usize,
        pos: usize,
        lang: &str,
    ) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        // Extract already used characters (skip leading '-')
        let used_chars: Vec<char> = current_chain.chars().skip(1).collect();
        let chain_prefix = &current_chain[1..]; // without leading '-'

        // 1. First, check for matching common flag combos
        for combo in &spec.common_flag_combos {
            // Skip if combo doesn't start with current prefix
            if !combo.combo.starts_with(chain_prefix) {
                continue;
            }
            // Skip if combo is same as current (already complete)
            if combo.combo == chain_prefix {
                continue;
            }

            // Check if all characters in combo are valid flags
            let all_valid = combo.combo.chars().all(|c| {
                spec.flags.iter().any(|f| f.short == Some(c))
            });

            if all_valid {
                suggestions.push(Suggestion {
                    value: format!("-{}", combo.combo),
                    description: Some(format!(
                        "[combo] {}",
                        combo.description.get(lang)
                    )),
                    extra: None,
                    span: Span {
                        start: start_idx,
                        end: pos,
                    },
                    append_whitespace: true,
                    style: None,
                });
            }
        }

        // 2. Then, suggest individual flags that can be added
        for flag in &spec.flags {
            if let Some(c) = flag.short {
                // Skip if already used
                if used_chars.contains(&c) {
                    continue;
                }

                // Skip if last char in chain takes value (can't chain further)
                if let Some(last_char) = used_chars.last() {
                    let last_takes_value = spec.flags.iter().any(|f| {
                        f.short == Some(*last_char) && f.takes_value
                    });
                    if last_takes_value {
                        continue;
                    }
                }

                suggestions.push(Suggestion {
                    value: format!("{}{}", current_chain, c),
                    description: Some(format!(
                        "(+{}) {}",
                        c,
                        flag.description.get(lang)
                    )),
                    extra: None,
                    span: Span {
                        start: start_idx,
                        end: pos,
                    },
                    append_whitespace: !flag.takes_value,
                    style: None,
                });
            }
        }

        suggestions
    }

    fn get_lang(&self) -> String {
        self.current_lang.read().unwrap().clone()
    }

    fn search_commands(&self, query: &str, lang: &str) -> Vec<(String, String, String)> {
        let mut results: Vec<(i64, String, String, String)> = Vec::new(); // (score, cmd, desc, match_type)
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        for cmd in self.commands.values() {
            self.search_command_recursive(cmd, query, lang, &cmd.name, &mut results);
        }

        // Apply context-aware boosting to scores
        for result in &mut results {
            let cmd_name = result.1.split_whitespace().next().unwrap_or(&result.1);
            let context_boost = tracker().score_boost(cmd_name, &cwd);
            result.0 += context_boost;
        }

        // Sort by score (higher is better)
        results.sort_by(|a, b| b.0.cmp(&a.0));

        // Remove duplicates and return without scores
        let mut seen = std::collections::HashSet::new();
        results
            .into_iter()
            .filter(|(_, cmd, _, _)| seen.insert(cmd.clone()))
            .map(|(_, cmd, desc, match_type)| (cmd, desc, match_type))
            .collect()
    }

    fn search_command_recursive(
        &self,
        cmd: &CommandSpec,
        query: &str,
        lang: &str,
        full_cmd: &str,
        results: &mut Vec<(i64, String, String, String)>,
    ) {
        // Search in command name using fuzzy matching
        if let Some(score) = self.fuzzy_match(query, &cmd.name) {
            results.push((
                score + 100, // Boost command name matches
                full_cmd.to_string(),
                cmd.description.get(lang).to_string(),
                format!("Command: {}", cmd.name),
            ));
        }

        // Search in description using fuzzy matching
        let desc = cmd.description.get(lang);
        if let Some(score) = self.fuzzy_match(query, desc) {
            results.push((
                score,
                full_cmd.to_string(),
                desc.to_string(),
                "Description".to_string(),
            ));
        }

        // Search in examples
        for example in &cmd.examples {
            let scenario = example.scenario.get(lang);
            if let Some(score) = self.fuzzy_match(query, scenario) {
                results.push((
                    score - 10, // Slightly lower priority for examples
                    example.cmd.clone(),
                    format!("{} - {}", scenario, example.cmd),
                    "Example".to_string(),
                ));
            }
        }

        // Search in subcommands
        for sub in &cmd.subcommands {
            let sub_full_cmd = format!("{} {}", full_cmd, sub.name);
            self.search_command_recursive(sub, query, lang, &sub_full_cmd, results);
        }
    }

    pub fn search(&self, query: &str) -> Vec<(String, String, String)> {
        let lang = self.get_lang();
        self.search_commands(query, &lang)
    }

    /// Get all command names for syntax highlighting
    pub fn get_command_names(&self) -> Vec<String> {
        self.commands.keys().cloned().collect()
    }

    /// Get command spec by name (for help mode)
    pub fn get_command_spec(&self, name: &str) -> Option<&CommandSpec> {
        self.commands.get(name)
    }

    /// Get completions from dynamic providers
    fn get_provider_completions(
        &self,
        cmd: &str,
        args: &[&str],
        partial: &str,
    ) -> Vec<Suggestion> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let context = ProviderContext::new(
            cwd,
            cmd,
            args.iter().map(|s| s.to_string()).collect(),
            partial,
        );

        let registry = providers::registry();
        let suggestions = registry.complete(&context);

        suggestions
            .into_iter()
            .map(|s| self.provider_suggestion_to_reedline(s, partial.len()))
            .collect()
    }

    /// Convert provider suggestion to reedline suggestion
    fn provider_suggestion_to_reedline(
        &self,
        suggestion: ProviderSuggestion,
        partial_len: usize,
    ) -> Suggestion {
        let description = match (suggestion.description, suggestion.category) {
            (Some(desc), Some(cat)) => Some(format!("[{}] {}", cat, desc)),
            (Some(desc), None) => Some(desc),
            (None, Some(cat)) => Some(format!("[{}]", cat)),
            (None, None) => None,
        };

        Suggestion {
            value: suggestion.value,
            description,
            extra: None,
            span: Span {
                start: 0, // Will be adjusted by caller
                end: partial_len,
            },
            append_whitespace: suggestion.append_whitespace,
            style: None,
        }
    }
}

impl Completer for SmartCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let lang = self.get_lang();
        let input = &line[0..pos];
        let parts: Vec<&str> = input.split_whitespace().collect();

        if line.starts_with('/') {
            let query = &line[1..pos]; // skip the slash

            // If query is empty, show a hint
            if query.is_empty() {
                return vec![Suggestion {
                    value: "/".to_string(),
                    description: Some(
                        "Type to search commands (e.g., /commit or /压缩)".to_string(),
                    ),
                    extra: None,
                    span: Span { start: 0, end: pos },
                    append_whitespace: false,
                    style: None,
                }];
            }

            // Search across all commands, descriptions, and examples
            let search_results = self.search_commands(query, &lang);

            let mut suggestions: Vec<Suggestion> = search_results
                .into_iter()
                .map(|(cmd, desc, match_type)| Suggestion {
                    value: cmd.clone(),
                    description: Some(format!("[{}] {}", match_type, desc)),
                    extra: None,
                    span: Span { start: 0, end: pos },
                    append_whitespace: false,
                    style: None,
                })
                .collect();

            // Add system commands if they match
            if self.fuzzy_match(query, "config").is_some() {
                suggestions.push(Suggestion {
                    value: "config".to_string(),
                    description: Some("[System] Configure shell settings".to_string()),
                    extra: None,
                    span: Span { start: 0, end: pos },
                    append_whitespace: false,
                    style: None,
                });
            }

            if self.fuzzy_match(query, "exit").is_some() {
                suggestions.push(Suggestion {
                    value: "exit".to_string(),
                    description: Some("[System] Exit the shell".to_string()),
                    extra: None,
                    span: Span { start: 0, end: pos },
                    append_whitespace: false,
                    style: None,
                });
            }

            return suggestions;
        }

        if parts.is_empty() || (parts.len() == 1 && !line.ends_with(' ')) {
            let query = parts.first().unwrap_or(&"");
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

            let mut suggestions: Vec<(i64, Suggestion)> = self
                .commands
                .values()
                .filter_map(|cmd| self.fuzzy_match(query, &cmd.name).map(|score| (score, cmd)))
                .map(|(score, cmd)| {
                    // Apply context-aware boosting
                    let context_boost = tracker().score_boost(&cmd.name, &cwd);
                    (
                        score + context_boost,
                        Suggestion {
                            value: cmd.name.clone(),
                            description: Some(cmd.description.get(&lang).to_string()),
                            extra: None,
                            span: Span {
                                start: pos - query.len(),
                                end: pos,
                            },
                            append_whitespace: true,
                            style: None,
                        },
                    )
                })
                .collect();

            // Sort by fuzzy score + context boost (higher is better)
            suggestions.sort_by(|a, b| b.0.cmp(&a.0));
            return suggestions.into_iter().map(|(_, s)| s).collect();
        }

        if let Some(cmd_name) = parts.first() {
            // Special handling for 'example' command - complete with command names that have examples
            if *cmd_name == "example" || *cmd_name == "examples" || *cmd_name == "ex" {
                let is_new_arg = line.ends_with(' ');
                let query: String = if is_new_arg {
                    String::new()
                } else if parts.len() > 1 {
                    parts[1..].join(" ")
                } else {
                    String::new()
                };
                let query_ref = query.as_str();
                let start_idx = if is_new_arg {
                    pos
                } else {
                    // Find start of the argument portion after 'example '
                    let prefix_len = cmd_name.len() + 1; // "example "
                    if input.len() > prefix_len {
                        prefix_len
                    } else {
                        pos
                    }
                };

                // Get commands with examples and filter by query
                let commands = self.get_commands_with_examples();
                let mut suggestions: Vec<Suggestion> = commands
                    .iter()
                    .filter(|c| {
                        if query_ref.is_empty() {
                            true
                        } else {
                            c.to_lowercase().contains(&query_ref.to_lowercase())
                                || self.fuzzy_match(query_ref, c).is_some()
                        }
                    })
                    .map(|c| Suggestion {
                        value: c.clone(),
                        description: Some(format!("[{}]", c.split_whitespace().next().unwrap_or(c))),
                        extra: None,
                        span: Span {
                            start: start_idx,
                            end: pos,
                        },
                        append_whitespace: true,
                        style: None,
                    })
                    .collect();

                // Add 'search' subcommand if matching
                if "search".starts_with(query_ref) || query_ref.is_empty() {
                    suggestions.insert(
                        0,
                        Suggestion {
                            value: "search".to_string(),
                            description: Some("Search all examples".to_string()),
                            extra: None,
                            span: Span {
                                start: start_idx,
                                end: pos,
                            },
                            append_whitespace: true,
                            style: None,
                        },
                    );
                }

                return suggestions;
            }

            if let Some(root_spec) = self.commands.get(*cmd_name) {
                // Determine which tokens are "completed" and can be used for descent
                let is_new_arg = line.ends_with(' ');
                let num_parts_to_descend = if is_new_arg {
                    parts.len()
                } else {
                    parts.len().saturating_sub(1)
                };

                // Descend the tree
                let mut current_spec = root_spec;
                let mut subcommand_depth = 0;
                for sub_name in parts.iter().take(num_parts_to_descend).skip(1) {
                    if let Some(sub) = current_spec.subcommands.iter().find(|s| &s.name == sub_name)
                    {
                        current_spec = sub;
                        subcommand_depth += 1;
                    } else {
                        // User typed something that isn't a known subcommand.
                        break;
                    }
                }

                // Setup Query
                let last_part = parts.last().unwrap_or(&"");
                let query = if is_new_arg { "" } else { *last_part };
                let start_idx = if is_new_arg { pos } else { pos - query.len() };

                // Calculate current argument position (after subcommands)
                // This will be used in future phases for argument type validation
                let _arg_position = if is_new_arg {
                    parts.len() - 1 - subcommand_depth
                } else {
                    parts.len() - 2 - subcommand_depth
                };

                // 1. Try dynamic provider completions first
                let provider_suggestions = self.get_provider_completions(
                    cmd_name,
                    &parts[1..],
                    query,
                );

                if !provider_suggestions.is_empty() {
                    return provider_suggestions
                        .into_iter()
                        .map(|mut s| {
                            s.span = Span {
                                start: start_idx,
                                end: pos,
                            };
                            s
                        })
                        .collect();
                }

                // 2. Subcommand completion
                // Suggest subcommands of the CURRENT spec (use prefix match for speed in tab completion)
                let sub_suggestions: Vec<Suggestion> = current_spec
                    .subcommands
                    .iter()
                    .filter(|sub| self.prefix_match(query, &sub.name))
                    .map(|sub| Suggestion {
                        value: sub.name.clone(),
                        description: Some(sub.description.get(&lang).to_string()),
                        extra: None,
                        span: Span {
                            start: start_idx,
                            end: pos,
                        },
                        append_whitespace: true,
                        style: None,
                    })
                    .collect();

                // 3. Flag completion
                // Suggest flags of the CURRENT spec
                let mut flag_suggestions = Vec::new();
                if query.starts_with('-') || is_new_arg {
                    let is_short_chain = query.starts_with('-') && !query.starts_with("--");
                    let is_long_flag = query.starts_with("--");

                    // For short flag chains (e.g., "-zx"), use combo suggestions
                    if is_short_chain && query.len() > 1 {
                        // Check if last char takes value - if so, stop suggesting more flags
                        let last_char = query.chars().last().unwrap();
                        let last_takes_value = current_spec
                            .flags
                            .iter()
                            .any(|f| f.short == Some(last_char) && f.takes_value);

                        if !last_takes_value {
                            flag_suggestions = self.get_combo_suggestions(
                                current_spec,
                                query,
                                start_idx,
                                pos,
                                &lang,
                            );
                        }
                    } else {
                        // Standard flag suggestions (single dash or long flags)
                        for flag in &current_spec.flags {
                            let short = flag.short.map(|c| format!("-{}", c));
                            let long = flag.long.as_ref().map(|s| format!("--{}", s));

                            // Match short flags
                            if !is_long_flag {
                                if let Some(ref s) = short {
                                    if s.starts_with(query) || (is_new_arg && query.is_empty()) {
                                        flag_suggestions.push(Suggestion {
                                            value: s.clone(),
                                            description: Some(
                                                flag.description.get(&lang).to_string(),
                                            ),
                                            extra: None,
                                            span: Span {
                                                start: start_idx,
                                                end: pos,
                                            },
                                            append_whitespace: !flag.takes_value,
                                            style: None,
                                        });
                                    }
                                }
                            }

                            // Match long flags
                            if let Some(ref l) = long {
                                if l.starts_with(query) || (is_new_arg && query.is_empty()) {
                                    flag_suggestions.push(Suggestion {
                                        value: l.clone(),
                                        description: Some(
                                            flag.description.get(&lang).to_string(),
                                        ),
                                        extra: None,
                                        span: Span {
                                            start: start_idx,
                                            end: pos,
                                        },
                                        append_whitespace: !flag.takes_value,
                                        style: None,
                                    });
                                }
                            }
                        }
                    }
                }

                // 4. Example completion - DISABLED in Tab completion
                // Examples are now accessible via the 'example' command
                // This makes Tab completion cleaner and faster
                // let mut example_suggestions = Vec::new();
                // let full_input = &line[0..pos];
                // for example in &current_spec.examples {
                //     if example.cmd.starts_with(full_input) {
                //         example_suggestions.push(Suggestion {
                //             value: example.cmd.clone(),
                //             description: Some(format!("[Ex] {}", example.scenario.get(&lang))),
                //             extra: None,
                //             span: Span { start: 0, end: pos },
                //             append_whitespace: false,
                //             style: None,
                //         });
                //     }
                // }

                // Combine suggestions (subcommands + flags only, no examples)
                let mut all_suggestions = sub_suggestions;
                all_suggestions.extend(flag_suggestions);

                if !all_suggestions.is_empty() {
                    return all_suggestions;
                }

                // Check for path completion trigger
                if current_spec.is_path_completion {
                    // Fall through to path completion below
                } else {
                    return vec![];
                }
            }

            // Try provider completions for unknown commands too
            let is_new_arg = line.ends_with(' ');
            let query = if is_new_arg {
                ""
            } else {
                parts.last().unwrap_or(&"")
            };

            let provider_suggestions = self.get_provider_completions(
                cmd_name,
                &parts[1..],
                query,
            );

            if !provider_suggestions.is_empty() {
                let start_idx = if is_new_arg { pos } else { pos - query.len() };
                return provider_suggestions
                    .into_iter()
                    .map(|mut s| {
                        s.span = Span {
                            start: start_idx,
                            end: pos,
                        };
                        s
                    })
                    .collect();
            }
        }

        // Fallback: Path completion
        let last_part = parts.last().unwrap_or(&"");
        let is_new_arg = line.ends_with(' ');
        let query = if is_new_arg { "" } else { *last_part };
        let start_idx = if is_new_arg { pos } else { pos - query.len() };

        // Use enhanced path provider
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let cmd = parts.first().unwrap_or(&"");
        let context = ProviderContext::new(
            cwd.clone(),
            cmd,
            parts.iter().skip(1).map(|s| s.to_string()).collect(),
            query,
        );

        let registry = providers::registry();
        let path_suggestions = registry.complete(&context);

        if !path_suggestions.is_empty() {
            return path_suggestions
                .into_iter()
                .map(|s| {
                    let mut suggestion = self.provider_suggestion_to_reedline(s, query.len());
                    suggestion.span = Span {
                        start: start_idx,
                        end: pos,
                    };
                    suggestion
                })
                .collect();
        }

        // Fallback to simple directory listing if providers didn't return anything
        if let Ok(paths) = std::fs::read_dir(".") {
            return paths
                .filter_map(|p| p.ok())
                .map(|p| {
                    let name = p.file_name().to_string_lossy().to_string();
                    let is_dir = p.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    (name, is_dir)
                })
                .filter(|(name, _)| name.starts_with(query))
                .map(|(name, is_dir)| Suggestion {
                    value: if is_dir { format!("{}/", name) } else { name },
                    description: Some(if is_dir {
                        "Dir".to_string()
                    } else {
                        "File".to_string()
                    }),
                    extra: None,
                    span: Span {
                        start: start_idx,
                        end: pos,
                    },
                    append_whitespace: true,
                    style: None,
                })
                .collect();
        }

        vec![]
    }
}
