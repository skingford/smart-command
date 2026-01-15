use reedline::{Completer, Span, Suggestion};
use crate::command_def::CommandSpec;
use crate::definitions;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct SmartCompleter {
    commands: HashMap<String, CommandSpec>,
    current_lang: Arc<RwLock<String>>,
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

        Self { commands, current_lang }
    }

    fn fuzzy_match(&self, input: &str, target: &str) -> bool {
        target.to_lowercase().starts_with(&input.to_lowercase())
    }

    fn get_lang(&self) -> String {
        self.current_lang.read().unwrap().clone()
    }
}

impl Completer for SmartCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let lang = self.get_lang();
        let input = &line[0..pos];
        let parts: Vec<&str> = input.trim_start().split_whitespace().collect();
        
        if line.starts_with('/') {
            let query = &line[1..pos]; // skip the slash
            let span_start = pos - query.len() - 1; // encompass the slash
            
            return self.commands.values()
                .filter(|cmd| self.fuzzy_match(query, &cmd.name))
                .map(|cmd| Suggestion {
                    value: cmd.name.clone(),
                    description: Some(cmd.description.get(&lang).to_string()),
                    extra: None,
                    span: Span { start: span_start, end: pos },
                    append_whitespace: true,
                    style: None,
                })
                .collect();
        }

        if parts.is_empty() || (parts.len() == 1 && !line.ends_with(' ')) {
             let query = parts.first().unwrap_or(&"");
             return self.commands.values()
                .filter(|cmd| self.fuzzy_match(query, &cmd.name))
                .map(|cmd| Suggestion {
                    value: cmd.name.clone(),
                    description: Some(cmd.description.get(&lang).to_string()),
                    extra: None,
                    span: Span { start: pos - query.len(), end: pos },
                    append_whitespace: true,
                    style: None,
                })
                .collect();
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
                // Suggest subcommands of the CURRENT spec
                let sub_suggestions: Vec<Suggestion> = current_spec.subcommands.iter()
                    .filter(|sub| self.fuzzy_match(query, &sub.name))
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

                // Combine suggestions
                let mut all_suggestions = sub_suggestions;
                all_suggestions.extend(flag_suggestions);

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
