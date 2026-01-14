use reedline::{Completer, Span, Suggestion};
use crate::command_def::CommandSpec;
use crate::definitions;
use std::collections::HashMap;

pub struct SmartCompleter {
    commands: HashMap<String, CommandSpec>,
}

impl SmartCompleter {
    pub fn new(loaded_commands: Vec<CommandSpec>) -> Self {
        let mut commands = HashMap::new();
        for cmd in loaded_commands {
            commands.insert(cmd.name.clone(), cmd);
        }
        
        // Add built-in defaults if not present or as extra
        // For now, assume everything comes from YAML or we can mix
        // Let's keep ls/cd manual if they are not in YAML yet, or move them to YAML.
        // For this step, I'll keep the hardcoded ones from `definitions::other_specs` 
        // IF they are not already loaded, or just add them.
        
        for cmd in definitions::other_specs() {
            if !commands.contains_key(&cmd.name) {
                commands.insert(cmd.name.clone(), cmd);
            }
        }

        Self { commands }
    }

    fn fuzzy_match(&self, input: &str, target: &str) -> bool {
        target.to_lowercase().starts_with(&input.to_lowercase())
    }
}

impl Completer for SmartCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let input = &line[0..pos];
        let parts: Vec<&str> = input.trim_start().split_whitespace().collect();
        
        if parts.is_empty() || (parts.len() == 1 && !line.ends_with(' ')) {
             let query = parts.first().unwrap_or(&"");
             return self.commands.values()
                .filter(|cmd| self.fuzzy_match(query, &cmd.name))
                .map(|cmd| Suggestion {
                    value: cmd.name.clone(),
                    description: Some(cmd.description.clone()),
                    extra: None,
                    span: Span { start: pos - query.len(), end: pos },
                    append_whitespace: true,
                    style: None,
                })
                .collect();
        }

        if let Some(cmd_name) = parts.first() {
            if let Some(cmd_spec) = self.commands.get(*cmd_name) {
                let last_part = parts.last().unwrap_or(&"");
                let is_new_arg = line.ends_with(' ');
                
                let query = if is_new_arg { "" } else { *last_part };
                let start_idx = if is_new_arg { pos } else { pos - query.len() };

                // Check for path completion trigger
                if cmd_spec.is_path_completion {
                     // logic below will handle it
                } else if query.starts_with('-') || is_new_arg {
                    let mut suggestions = Vec::new();
                    let is_short_chain = query.starts_with('-') && !query.starts_with("--");
                    
                    if is_short_chain {
                         if let Some(last_char) = query.chars().last() {
                             if let Some(_flag) = cmd_spec.flags.iter().find(|f| f.short == Some(last_char)) {
                                 // Option to handle specific logic if flag takes value
                             }
                         }
                    }

                    let used_chars: Vec<char> = if is_short_chain { query.chars().skip(1).collect() } else { vec![] };

                    let stop_flagging = if is_short_chain {
                        if let Some(last_char) = query.chars().last() {
                             cmd_spec.flags.iter().any(|f| f.short == Some(last_char) && f.takes_value)
                        } else { false }
                    } else { false };

                    if !stop_flagging {
                        for flag in &cmd_spec.flags {
                            let short = flag.short.map(|c| format!("-{}", c));
                            let long = flag.long.as_ref().map(|s| format!("--{}", s));
                            let match_short = short.as_ref().map(|s| s.starts_with(query)).unwrap_or(false);
                            let match_long = long.as_ref().map(|s| s.starts_with(query)).unwrap_or(false);

                            if match_short && short.is_some() {
                                 suggestions.push(Suggestion {
                                    value: short.clone().unwrap(),
                                    description: Some(flag.description.clone()),
                                    extra: None,
                                    span: Span { start: start_idx, end: pos },
                                    append_whitespace: true,
                                    style: None,
                                });
                            }
                            if match_long && long.is_some() {
                                 suggestions.push(Suggestion {
                                    value: long.clone().unwrap(),
                                    description: Some(flag.description.clone()),
                                    extra: None,
                                    span: Span { start: start_idx, end: pos },
                                    append_whitespace: true,
                                    style: None,
                                });
                            }

                            if is_short_chain {
                                 if let Some(c) = flag.short {
                                     if !used_chars.contains(&c) {
                                         suggestions.push(Suggestion {
                                            value: format!("{}{}", query, c),
                                            description: Some(format!("{} (+{})", flag.description, c)),
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
                    if !suggestions.is_empty() { return suggestions; }
                }

                if (parts.len() == 2 && !is_new_arg) || (parts.len() == 1 && is_new_arg) {
                     let sub_suggestions: Vec<Suggestion> = cmd_spec.subcommands.iter()
                        .filter(|sub| self.fuzzy_match(query, &sub.name))
                        .map(|sub| Suggestion {
                            value: sub.name.clone(),
                            description: Some(sub.description.clone()),
                            extra: None,
                            span: Span { start: start_idx, end: pos },
                            append_whitespace: true,
                            style: None,
                        })
                        .collect();
                     if !sub_suggestions.is_empty() { return sub_suggestions; }
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
