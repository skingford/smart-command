use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultPrompt, Reedline, ReedlineEvent, ReedlineMenu, Signal, ExampleHighlighter, Emacs, MenuBuilder
};
use std::process::Command;

mod command_def;
mod definitions;
mod completer;
mod loader;

use completer::SmartCompleter;

use std::sync::{Arc, RwLock};

enum ShellState {
    Normal,
    SelectingSearchResult(Vec<(String, String, String)>),
}

fn main() -> reedline::Result<()> {
    let commands = loader::load_commands("definitions");
    let current_lang = Arc::new(RwLock::new("en".to_string()));
    let completer = SmartCompleter::new(commands, current_lang.clone());
    let completer_for_editor = Box::new(completer.clone());
    let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));

    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        reedline::KeyModifiers::NONE,
        reedline::KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );

    // Add Alt+H for history (placeholder as user requested)
    keybindings.add_binding(
        reedline::KeyModifiers::ALT,
        reedline::KeyCode::Char('h'),
        ReedlineEvent::Menu("history_menu".to_string()),
    );

    let highlighter = Box::new(ExampleHighlighter::new(vec![
        "git".to_string(), "tar".to_string(), "ls".to_string(), "cd".to_string(), "cargo".to_string()
    ]));

    let edit_mode = Box::new(Emacs::new(keybindings));

    let mut line_editor = Reedline::create()
        .with_completer(completer_for_editor)
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_highlighter(highlighter)
        .with_edit_mode(edit_mode);

    let prompt = DefaultPrompt::default();
    let mut shell_state = ShellState::Normal;

    println!("Welcome to Smart Command! \nType 'git <tab>' to see magic. Press Ctrl-C or type 'exit' to quit.\nType 'config set-lang <lang>' to change language (e.g. 'zh').");

    loop {
        let sig = line_editor.read_line(&prompt)?;
        match sig {
            Signal::Success(buffer) => {
                let trimmed = buffer.trim();

                match &shell_state {
                    ShellState::Normal => {
                        // Check if this is a search command
                        if trimmed.starts_with('/') && trimmed.len() > 1 {
                            let query = &trimmed[1..]; // Remove '/' prefix
                            let results = completer.search(query);

                            if results.is_empty() {
                                println!("No results found for: {}", query);
                            } else {
                                println!("\nSearch results for '{}':", query);
                                for (i, (cmd, desc, match_type)) in results.iter().enumerate() {
                                    println!("{}. [{}] {}", i + 1, match_type, cmd);
                                    println!("   {}", desc);
                                }
                                println!("\nType a number to select, or press Enter to cancel:");
                                shell_state = ShellState::SelectingSearchResult(results);
                            }
                            continue;
                        }

                        // Normal command handling
                        if trimmed == "exit" { break; }
                        if trimmed.is_empty() { continue; }

                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if let Some(cmd) = parts.first() {
                            if *cmd == "cd" {
                                if let Some(path) = parts.get(1) {
                                    if let Err(e) = std::env::set_current_dir(path) {
                                        eprintln!("cd: {}", e);
                                    }
                                } else if let Some(home) = dirs::home_dir() {
                                    let _ = std::env::set_current_dir(home);
                                }
                                continue;
                            } else if *cmd == "config" {
                                if let Some(sub) = parts.get(1) {
                                    if *sub == "set-lang" {
                                        if let Some(lang) = parts.get(2) {
                                            *current_lang.write().unwrap() = lang.to_string();
                                            println!("Language switching to: {}", lang);
                                        } else {
                                            println!("Usage: config set-lang <lang>");
                                        }
                                    } else {
                                        println!("Unknown config subcommand: {}", sub);
                                    }
                                } else {
                                    println!("Usage: config set-lang <lang>");
                                }
                                continue;
                            }

                            let status = Command::new("sh")
                                .arg("-c")
                                .arg(&buffer)
                                .status();

                            match status {
                                Ok(_) => { println!(""); },
                                Err(e) => eprintln!("Error executing command: {}", e),
                            }
                        }
                    }

                    ShellState::SelectingSearchResult(results) => {
                        // Handle number selection
                        if let Ok(num) = trimmed.parse::<usize>() {
                            if num > 0 && num <= results.len() {
                                let selected = &results[num - 1].0;
                                println!("Selected: {}", selected);
                                println!("(Command copied - you can now type it or modify it)");
                                shell_state = ShellState::Normal;
                            } else {
                                println!("Invalid selection. Try again (1-{}):", results.len());
                            }
                        } else if trimmed.is_empty() {
                            println!("Search cancelled.");
                            shell_state = ShellState::Normal;
                        } else {
                            println!("Please enter a number (1-{}):", results.len());
                        }
                    }
                }
            }
            Signal::CtrlD | Signal::CtrlC => {
                println!("\nAborted!");
                break;
            }
        }
    }

    Ok(())
}
