use reedline::{Completer, Span, Suggestion};
use crate::command_def::CommandSpec;
use crate::definitions;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

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

    fn get_lang(&self) -> String {
        self.current_lang.read().unwrap().clone()
    }

    fn search_commands(&self, query: &str, lang: &str) -> Vec<(String, String, String)> {
        let mut results: Vec<(i64, String, String, String)> = Vec::new(); // (score, cmd, desc, match_type)

        for cmd in self.commands.values() {
            self.search_command_recursive(cmd, query, lang, &cmd.name, &mut results);
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
}

impl Completer for SmartCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let lang = self.get_lang();
        let input = &line[0..pos];
        let parts: Vec<&str> = input.trim_start().split_whitespace().collect();
        
        if line.starts_with('/') {
            let query = &line[1..pos]; // skip the slash

            // If query is empty, show a hint
            if query.is_empty() {
                return vec![Suggestion {
                    value: "/".to_string(),
                    description: Some("Type to search commands (e.g., /压缩 for compression)".to_string()),
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
             let mut suggestions: Vec<(i64, Suggestion)> = self.commands.values()
                .filter_map(|cmd| {
                    self.fuzzy_match(query, &cmd.name).map(|score| (score, cmd))
                })
                .map(|(score, cmd)| (score, Suggestion {
                    value: cmd.name.clone(),
                    description: Some(cmd.description.get(&lang).to_string()),
                    extra: None,
                    span: Span { start: pos - query.len(), end: pos },
                    append_whitespace: true,
                    style: None,
                }))
                .collect();

             // Sort by fuzzy score (higher is better)
             suggestions.sort_by(|a, b| b.0.cmp(&a.0));
             return suggestions.into_iter().map(|(_, s)| s).collect();
        }

        if let Some(cmd_name) = parts.first() {
            if let Some(root_spec) = self.commands.get(*cmd_name) {
                // Determine which tokens are "completed" and can be used for descent
                let is_new_arg = line.ends_with(' ');
                let num_parts_to_descend = if is_new_arg { parts.len() } else { parts.len().saturating_sub(1) };
                
                // Descend the tree
                let mut current_spec = root_spec;
                for i in 1..num_parts_to_descend {
                    let sub_name = parts[i];
                     if let Some(sub) = current_spec.subcommands.iter().find(|s| s.name == sub_name) {
                         current_spec = sub;
                     } else {
                         // User typed something that isn't a known subcommand. 
                         break;
                     }
                }

                // Setup Query
                let last_part = parts.last().unwrap_or(&"");
                let query = if is_new_arg { "" } else { *last_part };
                let start_idx = if is_new_arg { pos } else { pos - query.len() };
                
                // 1. Subcommand completion
                // Suggest subcommands of the CURRENT spec (use prefix match for speed in tab completion)
                let sub_suggestions: Vec<Suggestion> = current_spec.subcommands.iter()
                    .filter(|sub| self.prefix_match(query, &sub.name))
                    .map(|sub| Suggestion {
                        value: sub.name.clone(),
                        description: Some(sub.description.get(&lang).to_string()),
                        extra: None,
                        span: Span { start: start_idx, end: pos },
                        append_whitespace: true,
                        style: None,
                    })
                    .collect();
                
                // 2. Flag completion
                // Suggest flags of the CURRENT spec
                let mut flag_suggestions = Vec::new();
                if query.starts_with('-') || is_new_arg {
                    // Copied/Refined flag logic
                    let is_short_chain = query.starts_with('-') && !query.starts_with("--");
                    let used_chars: Vec<char> = if is_short_chain { query.chars().skip(1).collect() } else { vec![] };
                    
                    // Simple check: if we are in short chain and last char takes value, don't suggest more flags?
                     let stop_flagging = if is_short_chain {
                        if let Some(last_char) = query.chars().last() {
                             current_spec.flags.iter().any(|f| f.short == Some(last_char) && f.takes_value)
                        } else { false }
                    } else { false };

                    if !stop_flagging {
                         for flag in &current_spec.flags {
                            let short = flag.short.map(|c| format!("-{}", c));
                            let long = flag.long.as_ref().map(|s| format!("--{}", s));
                            let match_short = short.as_ref().map(|s| s.starts_with(query)).unwrap_or(false);
                            let match_long = long.as_ref().map(|s| s.starts_with(query)).unwrap_or(false);

                            if match_short && short.is_some() {
                                 flag_suggestions.push(Suggestion {
                                    value: short.clone().unwrap(),
                                    description: Some(flag.description.get(&lang).to_string()),
                                    extra: None,
                                    span: Span { start: start_idx, end: pos },
                                    append_whitespace: true,
                                    style: None,
                                });
                            }
                            if match_long && long.is_some() {
                                 flag_suggestions.push(Suggestion {
                                    value: long.clone().unwrap(),
                                    description: Some(flag.description.get(&lang).to_string()),
                                    extra: None,
                                    span: Span { start: start_idx, end: pos },
                                    append_whitespace: true,
                                    style: None,
                                });
                            }
                            
                            // Combined short flags
                            if is_short_chain {
                                 if let Some(c) = flag.short {
                                     if !used_chars.contains(&c) {
                                         flag_suggestions.push(Suggestion {
                                            value: format!("{}{}", query, c),
                                            description: Some(format!("{} (+{})", flag.description.get(&lang), c)),
                                            extra: None,
                                            span: Span { start: start_idx, end: pos },
                                            append_whitespace: true,
                                            style: None, 
                                         });
                                     }
                                 }
                            }
                         }
                    }
                }

                // 3. Example completion
                let mut example_suggestions = Vec::new();
                let full_input = &line[0..pos];
                for example in &current_spec.examples {
                    if example.cmd.starts_with(full_input) {
                        example_suggestions.push(Suggestion {
                            value: example.cmd.clone(),
                            description: Some(format!("[Ex] {}", example.scenario.get(&lang))),
                            extra: None,
                            span: Span { start: 0, end: pos },
                            append_whitespace: false, // Examples are complete commands usually
                            style: None,
                        });
                    }
                }

                // Combine suggestions
                let mut all_suggestions = sub_suggestions;
                all_suggestions.extend(flag_suggestions);
                all_suggestions.extend(example_suggestions);

                if !all_suggestions.is_empty() { return all_suggestions; }

                // Check for path completion trigger
                if current_spec.is_path_completion {
                   // Fall through to path completion below
                } else {
                   return vec![];
                }
            }
        }

        // Fallback: Path completion
        let last_part = parts.last().unwrap_or(&"");
        let is_new_arg = line.ends_with(' ');
        let query = if is_new_arg { "" } else { *last_part };
        let start_idx = if is_new_arg { pos } else { pos - query.len() };
        
        // Simple directory reader
        if let Ok(paths) = std::fs::read_dir(".") {
             return paths.filter_map(|p| p.ok())
                .map(|p| {
                   let name = p.file_name().to_string_lossy().to_string();
                   let is_dir = p.file_type().map(|t| t.is_dir()).unwrap_or(false);
                   (name, is_dir)
                })
                .filter(|(name, _)| name.starts_with(query))
                .map(|(name, is_dir)| Suggestion {
                    value: if is_dir { format!("{}/", name) } else { name },
                    description: Some(if is_dir { "Dir".to_string() } else { "File".to_string() }),
                    extra: None,
                    span: Span { start: start_idx, end: pos },
                    append_whitespace: true,
                    style: None,
                })
                .collect();
        }
        
        vec![]
    }
}
