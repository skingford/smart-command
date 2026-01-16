use reedline::{
    default_emacs_keybindings, ColumnarMenu, Reedline, ReedlineEvent, ReedlineMenu, Signal, ExampleHighlighter, Emacs, MenuBuilder,
    FileBackedHistory, Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus,
};
use std::borrow::Cow;
use std::process::Command;
use std::path::PathBuf;
use std::sync::Mutex;

mod command_def;
mod definitions;
mod completer;
mod loader;

use completer::SmartCompleter;

use std::sync::{Arc, RwLock};

// Track previous directory for `cd -`
static OLDPWD: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Custom prompt showing current directory and git branch
struct SmartPrompt;

impl SmartPrompt {
    fn get_cwd_display() -> String {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| {
                // Try to shorten path by replacing home with ~
                dirs::home_dir().and_then(|home| {
                    cwd.strip_prefix(&home)
                        .ok()
                        .map(|rel| format!("~/{}", rel.display()))
                })
                .or_else(|| Some(cwd.display().to_string()))
            })
            .unwrap_or_else(|| "?".to_string())
    }

    fn get_git_branch() -> Option<String> {
        Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                } else {
                    None
                }
            })
    }
}

impl Prompt for SmartPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        let cwd = Self::get_cwd_display();
        let branch_info = Self::get_git_branch()
            .map(|b| format!(" ({})", b))
            .unwrap_or_default();

        Cow::Owned(format!("{}{}", cwd, branch_info))
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("\n❯ ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("... ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!(
            "({}reverse-search: {}) ",
            prefix, history_search.term
        ))
    }
}

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

    // Dynamic highlighter based on loaded commands
    let command_names: Vec<String> = completer.get_command_names();
    let highlighter = Box::new(ExampleHighlighter::new(command_names));

    let edit_mode = Box::new(Emacs::new(keybindings));

    // Setup history persistence
    let history_path = dirs::home_dir()
        .map(|h| h.join(".smart_command_history"))
        .unwrap_or_else(|| PathBuf::from(".smart_command_history"));

    let history = FileBackedHistory::with_file(1000, history_path.clone())
        .expect("Failed to create history file");

    let mut line_editor = Reedline::create()
        .with_completer(completer_for_editor)
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_highlighter(highlighter)
        .with_edit_mode(edit_mode)
        .with_history(Box::new(history));

    let prompt = SmartPrompt;
    let mut shell_state = ShellState::Normal;

    println!("Welcome to Smart Command!");
    println!("  • Type 'git <tab>' to see completions");
    println!("  • Type '/<keyword>' to search commands (e.g. '/commit')");
    println!("  • Type 'config set-lang <lang>' to change language (zh/en)");
    println!("  • Press Ctrl-D or type 'exit' to quit");
    println!("  • History saved to: {}", history_path.display());

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
                                println!("\nType a number to execute, 'e<num>' to edit (e.g. 'e1'), or Enter to cancel:");
                                shell_state = ShellState::SelectingSearchResult(results);
                            }
                            continue;
                        }

                        // Normal command handling
                        if trimmed == "exit" { break; }
                        if trimmed.is_empty() { continue; }

                        execute_command(trimmed, &current_lang);
                    }

                    ShellState::SelectingSearchResult(results) => {
                        // Handle 'e<num>' for edit mode hint
                        if trimmed.starts_with('e') || trimmed.starts_with('E') {
                            if let Ok(num) = trimmed[1..].trim().parse::<usize>() {
                                if num > 0 && num <= results.len() {
                                    let selected = &results[num - 1].0;
                                    println!("\n💡 Command to edit: {}", selected);
                                    println!("   (Type the command with your modifications)");
                                    shell_state = ShellState::Normal;
                                    continue;
                                }
                            }
                            println!("Invalid edit selection. Try 'e1', 'e2', etc.");
                            continue;
                        }

                        // Handle number selection - execute directly
                        if let Ok(num) = trimmed.parse::<usize>() {
                            if num > 0 && num <= results.len() {
                                let selected = &results[num - 1].0;
                                println!("\n→ Executing: {}", selected);
                                execute_command(selected, &current_lang);
                                shell_state = ShellState::Normal;
                            } else {
                                println!("Invalid selection. Try again (1-{}):", results.len());
                            }
                        } else if trimmed.is_empty() {
                            println!("Search cancelled.");
                            shell_state = ShellState::Normal;
                        } else {
                            println!("Please enter a number (1-{}), 'e<num>' to edit, or Enter to cancel:", results.len());
                        }
                    }
                }
            }
            Signal::CtrlC => {
                // Ctrl+C: Clear current input, continue REPL
                println!("^C");
                shell_state = ShellState::Normal;
                // Continue the loop, don't exit
            }
            Signal::CtrlD => {
                // Ctrl+D: Exit shell
                println!("\nGoodbye!");
                break;
            }
        }
    }

    Ok(())
}

/// Execute a command and handle special built-in commands
fn execute_command(command: &str, current_lang: &Arc<RwLock<String>>) {
    let parts: Vec<&str> = command.split_whitespace().collect();

    if let Some(cmd) = parts.first() {
        // Handle 'cd' command
        if *cmd == "cd" {
            handle_cd(&parts);
            return;
        }

        // Handle 'config' command
        if *cmd == "config" {
            handle_config(&parts, current_lang);
            return;
        }

        // Execute external command
        let status = Command::new("sh")
            .arg("-c")
            .arg(command)
            .status();

        match status {
            Ok(exit_status) => {
                match exit_status.code() {
                    Some(0) => {}, // Success, silent
                    Some(code) => eprintln!("Exit: {}", code),
                    None => eprintln!("Process terminated by signal"),
                }
            },
            Err(e) => eprintln!("Error executing command: {}", e),
        }
    }
}

/// Handle 'cd' command with OLDPWD support
fn handle_cd(parts: &[&str]) {
    let current_dir = std::env::current_dir().ok();

    let target_path: Option<PathBuf> = if let Some(path) = parts.get(1) {
        if *path == "-" {
            // cd - : go to previous directory
            let old = OLDPWD.lock().unwrap().clone();
            if let Some(ref old_path) = old {
                println!("{}", old_path.display());
                Some(old_path.clone())
            } else {
                eprintln!("cd: OLDPWD not set");
                None
            }
        } else if path.starts_with('~') {
            // Expand ~ to home directory
            dirs::home_dir().map(|home| {
                if *path == "~" {
                    home
                } else {
                    home.join(&path[2..]) // Skip "~/"
                }
            })
        } else {
            Some(PathBuf::from(path))
        }
    } else {
        // cd with no args: go to home
        dirs::home_dir()
    };

    if let Some(target) = target_path {
        if let Err(e) = std::env::set_current_dir(&target) {
            eprintln!("cd: {}: {}", target.display(), e);
        } else {
            // Update OLDPWD on successful cd
            if let Some(old) = current_dir {
                *OLDPWD.lock().unwrap() = Some(old);
            }
        }
    }
}

/// Handle 'config' command
fn handle_config(parts: &[&str], current_lang: &Arc<RwLock<String>>) {
    if let Some(sub) = parts.get(1) {
        if *sub == "set-lang" {
            if let Some(lang) = parts.get(2) {
                *current_lang.write().unwrap() = lang.to_string();
                println!("Language switched to: {}", lang);
            } else {
                println!("Usage: config set-lang <lang>");
                println!("Available languages: en, zh");
            }
        } else {
            println!("Unknown config subcommand: {}", sub);
            println!("Available: set-lang");
        }
    } else {
        println!("Usage: config set-lang <lang>");
    }
}
