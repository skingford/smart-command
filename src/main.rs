use reedline::{
    default_emacs_keybindings, ColumnarMenu, Emacs, FileBackedHistory, MenuBuilder, Prompt,
    PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus, Reedline, ReedlineEvent,
    ReedlineMenu, Signal,
};
use std::borrow::Cow;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

mod ai;
mod aliases;
mod argument;
mod bookmarks;
mod cli;
mod command_def;
mod completer;
mod config;
mod context;
mod definitions;
mod error;
mod highlighter;
mod hinter;
mod install;
mod loader;
mod output;
mod pipeline;
mod providers;
mod snippets;
mod timer;
mod ui;
mod validator;
mod watcher;
mod plugins;

use ai::{NaturalLanguageTemplates, TypoCorrector};
use aliases::AliasManager;
use bookmarks::BookmarkManager;
use cli::{Cli, Commands, ConfigAction};
use completer::SmartCompleter;
use config::AppConfig;
use highlighter::{SmartHighlighter, SyntaxTheme};
use hinter::SmartHinter;
use install::InstallOptions;
use nu_ansi_term::{Color, Style};
use output::Output;
use snippets::SnippetManager;
use timer::CommandTimer;
use validator::SmartValidator;
use plugins::PluginManager;

// Track previous directory for `cd -`
static OLDPWD: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Application state
#[allow(dead_code)]
struct AppState {
    config: AppConfig,
    danger_protection: bool,
}

/// Custom prompt showing current directory and git branch
struct SmartPrompt {
    config: AppConfig,
}

impl SmartPrompt {
    fn new(config: AppConfig) -> Self {
        Self { config }
    }

    fn get_cwd_display() -> String {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| {
                dirs::home_dir()
                    .and_then(|home| {
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
        let mut parts = Vec::new();

        if self.config.prompt.show_cwd {
            parts.push(Self::get_cwd_display());
        }

        if self.config.prompt.show_git_branch {
            if let Some(branch) = Self::get_git_branch() {
                parts.push(format!("({})", branch));
            }
        }

        Cow::Owned(output::Output::prompt(
            &parts.join(" "),
            Self::get_git_branch().as_deref(),
        ))
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Owned(format!("\n{} ", self.config.prompt.indicator))
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

fn main() -> anyhow::Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse_args();

    // Load configuration
    let mut config = AppConfig::load().unwrap_or_else(|e| {
        Output::warn(&format!("Failed to load config: {}, using defaults", e));
        AppConfig::default()
    });

    // CLI overrides
    if let Some(lang) = &cli.lang {
        config.lang = lang.clone();
    }
    if let Some(definitions_dir) = &cli.definitions {
        config.definitions_dir = Some(definitions_dir.clone());
    }
    if cli.no_danger_protection {
        config.danger_protection = false;
    }

    // Initialize tracing
    let log_level = if cli.verbose {
        "debug"
    } else {
        &config.log_level
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)),
        )
        .with_target(false)
        .init();

    info!("Starting Smart Command v{}", env!("CARGO_PKG_VERSION"));

    // Handle subcommands
    if let Some(subcommand) = cli.subcommand {
        return handle_subcommand(subcommand, &config);
    }

    // Handle single command execution
    if let Some(cmd) = cli.command {
        let definitions_dir = config
            .definitions_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("definitions"));
        let commands = loader::load_commands(&definitions_dir);
        let current_lang = Arc::new(RwLock::new(config.lang.clone()));
        let completer = SmartCompleter::new(commands, current_lang.clone());
        let command_names = completer.get_command_names();
        let typo_corrector = TypoCorrector::new(command_names);
        let state = AppState {
            config: config.clone(),
            danger_protection: config.danger_protection,
        };
        execute_command(&cmd, &current_lang, &state, &typo_corrector);
        return Ok(());
    }

    // Start REPL
    run_repl(config)
}

fn handle_subcommand(cmd: Commands, config: &AppConfig) -> anyhow::Result<()> {
    match cmd {
        Commands::Completions { shell } => {
            Cli::generate_completions(shell);
            eprintln!();
            cli::print_completion_instructions(shell);
        }
        Commands::Config { action } => match action {
            ConfigAction::Show => {
                Output::info("Current configuration:");
                println!("{:#?}", config);
            }
            ConfigAction::Generate => {
                println!("{}", config::generate_example_config());
            }
            ConfigAction::Path => {
                Output::info(&format!(
                    "Config file path: {}",
                    AppConfig::config_file_path().display()
                ));
            }
        },
        Commands::Search { query } => {
            let definitions_dir = config
                .definitions_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("definitions"));
            let commands = loader::load_commands(&definitions_dir);
            let current_lang = Arc::new(RwLock::new(config.lang.clone()));
            let completer = SmartCompleter::new(commands, current_lang);

            let results = completer.search(&query);
            if results.is_empty() {
                Output::warn(&format!("No results found for: {}", query));
            } else {
                Output::info(&format!("Search results for '{}':", query));
                for (i, (cmd, desc, match_type)) in results.iter().enumerate() {
                    Output::search_result(i + 1, cmd, match_type, desc);
                }
            }
        }
        Commands::List => {
            let definitions_dir = config
                .definitions_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("definitions"));
            let commands = loader::load_commands(&definitions_dir);
            let current_lang = Arc::new(RwLock::new(config.lang.clone()));
            let completer = SmartCompleter::new(commands, current_lang);

            Output::info("Available commands:");
            for name in completer.get_command_names() {
                println!("  {}", Output::command(&name));
            }
        }
        Commands::Install {
            bin_dir,
            definitions_dir,
            definitions_src,
            skip_bin,
            skip_definitions,
        } => {
            let opts = InstallOptions {
                bin_dir,
                definitions_dir,
                definitions_src,
                skip_bin,
                skip_definitions,
            };
            install::run_install(opts)?;
        }
    }
    Ok(())
}

fn run_repl(config: AppConfig) -> anyhow::Result<()> {
    let definitions_dir = config
        .definitions_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("definitions"));
    let commands = loader::load_commands(&definitions_dir);
    let current_lang = Arc::new(RwLock::new(config.lang.clone()));
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

    keybindings.add_binding(
        reedline::KeyModifiers::ALT,
        reedline::KeyCode::Char('h'),
        ReedlineEvent::Menu("history_menu".to_string()),
    );

    let command_names: Vec<String> = completer.get_command_names();

    // Create AI features
    let typo_corrector = TypoCorrector::new(command_names.clone());
    let nl_templates = NaturalLanguageTemplates::new();

    // Create SmartHighlighter with theme based on config
    let theme = match config.theme.as_deref() {
        Some("nord") => SyntaxTheme::nord(),
        Some("dracula") => SyntaxTheme::dracula(),
        _ => SyntaxTheme::default(),
    };
    let highlighter = Box::new(SmartHighlighter::new(command_names).with_theme(theme));

    // Create SmartHinter for inline suggestions
    let hinter = Box::new(
        SmartHinter::new().with_style(Style::new().italic().fg(Color::DarkGray)),
    );

    // Create SmartValidator for syntax checking
    let validator = Box::new(SmartValidator::new());

    let edit_mode = Box::new(Emacs::new(keybindings));

    let history = FileBackedHistory::with_file(config.history_size, config.history_path.clone())
        .expect("Failed to create history file");

    let mut line_editor = Reedline::create()
        .with_completer(completer_for_editor)
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_highlighter(highlighter)
        .with_hinter(hinter)
        .with_validator(validator)
        .with_edit_mode(edit_mode)
        .with_history(Box::new(history));

    let prompt = SmartPrompt::new(config.clone());
    let mut shell_state = ShellState::Normal;

    let state = AppState {
        config: config.clone(),
        danger_protection: config.danger_protection,
    };

    // Initialize UX managers
    let mut alias_manager = AliasManager::new();
    let mut snippet_manager = SnippetManager::new();
    let mut bookmark_manager = BookmarkManager::new();
    let mut command_timer = CommandTimer::new();
    let mut plugin_manager = PluginManager::new();

    // Display startup banner
    output::Output::banner();

    // Welcome message
    Output::dim("  Tab         - completion menu    /<keyword>  - search commands");
    Output::dim("  ?<query>    - natural language   :<snippet>  - expand snippet");
    Output::dim("  @<bookmark> - jump to bookmark   alias/bm    - manage aliases/bookmarks");
    Output::dim("  time        - command statistics Ctrl-D/exit - quit");
    println!();

    debug!("REPL started with config: {:?}", config);

    loop {
        let sig = line_editor.read_line(&prompt)?;
        match sig {
            Signal::Success(buffer) => {
                let trimmed = buffer.trim();

                match &shell_state {
                    ShellState::Normal => {
                        // Handle `/` prefix for command search
                        if trimmed.starts_with('/') && trimmed.len() > 1 {
                            let query = &trimmed[1..];
                            let results = completer.search(query);

                            if results.is_empty() {
                                Output::warn(&format!("No results found for: {}", query));
                            } else {
                                println!();
                                Output::info(&format!("Search results for '{}':", query));
                                for (i, (cmd, desc, match_type)) in results.iter().enumerate() {
                                    Output::search_result(i + 1, cmd, match_type, desc);
                                }
                                Output::dim("\nType a number to execute, 'e<num>' to edit, or Enter to cancel:");
                                shell_state = ShellState::SelectingSearchResult(results);
                            }
                            continue;
                        }

                        // Handle `?` prefix for natural language queries
                        if trimmed.starts_with('?') && trimmed.len() > 1 {
                            let query = &trimmed[1..];
                            let matches = nl_templates.find(query);

                            if matches.is_empty() {
                                Output::warn(&format!("No matching commands for: {}", query));
                                Output::dim("Try keywords like: large files, disk space, git history, etc.");
                            } else {
                                println!();
                                Output::info(&format!("Commands for '{}':", query));
                                for (i, (cmd, desc)) in matches.iter().enumerate() {
                                    println!("  {}. {} - {}", i + 1, Output::command(cmd), desc);
                                }
                                print!("\nExecute command? [1-{}/n]: ", matches.len());
                                io::stdout().flush().ok();

                                let mut input = String::new();
                                if io::stdin().read_line(&mut input).is_ok() {
                                    let input = input.trim();
                                    if let Ok(num) = input.parse::<usize>() {
                                        if num > 0 && num <= matches.len() {
                                            let cmd = matches[num - 1].0;
                                            Output::info(&format!("Executing: {}", cmd));
                                            execute_command(cmd, &current_lang, &state, &typo_corrector);
                                        }
                                    }
                                }
                            }
                            continue;
                        }

                        if trimmed == "exit" {
                            break;
                        }
                        if trimmed.is_empty() {
                            continue;
                        }

                        // Handle snippet expansion (`:snippet` prefix)
                        if trimmed.starts_with(':') {
                            if let Some(expanded) = snippet_manager.try_expand(trimmed) {
                                Output::info(&format!("Expanded: {}", expanded));
                                print!("Execute? [Y/n]: ");
                                io::stdout().flush().ok();
                                let mut input = String::new();
                                if io::stdin().read_line(&mut input).is_ok() {
                                    let response = input.trim().to_lowercase();
                                    if response.is_empty() || response == "y" || response == "yes" {
                                        command_timer.start(&expanded);
                                        execute_command(&expanded, &current_lang, &state, &typo_corrector);
                                        if let Some(dur) = command_timer.stop(None) {
                                            if let Some(formatted) = command_timer.format_duration(dur) {
                                                Output::dim(&format!("⏱  {}", formatted));
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Try as snippet command
                                let parts: Vec<&str> = trimmed[1..].split_whitespace().collect();
                                if let Some(output) = snippets::handle_snippet_command(&mut snippet_manager, "snippet", &parts) {
                                    println!("{}", output);
                                } else {
                                    Output::warn(&format!("Unknown snippet: {}", trimmed));
                                }
                            }
                            continue;
                        }

                        // Handle bookmark jump (`@bookmark` syntax)
                        if trimmed.starts_with('@') && !trimmed.contains(' ') {
                            let name = &trimmed[1..];
                            if let Some(path) = bookmark_manager.try_resolve(trimmed) {
                                let path_str = path.display().to_string();
                                Output::dim(&path_str);
                                if let Err(e) = std::env::set_current_dir(path) {
                                    Output::error(&format!("cd: {}: {}", path.display(), e));
                                } else {
                                    bookmark_manager.record_visit(name);
                                }
                            } else {
                                Output::error(&format!("Bookmark @{} not found", name));
                            }
                            continue;
                        }

                        // Handle built-in UX commands
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        let cmd = parts.first().map(|s| *s).unwrap_or("");

                        // Alias command
                        if cmd == "alias" || cmd == "unalias" {
                            if let Some(output) = aliases::handle_alias_command(&mut alias_manager, cmd, &parts[1..]) {
                                println!("{}", output);
                            }
                            continue;
                        }

                        // Bookmark command
                        if cmd == "bookmark" || cmd == "bm" || cmd == "unbookmark" || cmd == "unbm" {
                            let cwd = std::env::current_dir().unwrap_or_default();
                            if let Some(output) = bookmarks::handle_bookmark_command(&mut bookmark_manager, cmd, &parts[1..], &cwd) {
                                println!("{}", output);
                            }
                            continue;
                        }

                        // Snippet command
                        if cmd == "snippet" || cmd == "snip" {
                            if let Some(output) = snippets::handle_snippet_command(&mut snippet_manager, cmd, &parts[1..]) {
                                println!("{}", output);
                            }
                            continue;
                        }

                        // Timer command
                        if cmd == "time" || cmd == "timer" {
                            if let Some(output) = timer::handle_timer_command(&command_timer, cmd, &parts[1..]) {
                                println!("{}", output);
                            }
                            continue;
                        }

                        // Plugin command
                        if cmd == "plugin" || cmd == "plugins" {
                            if let Some(output) = plugins::handle_plugin_command(&mut plugin_manager, cmd, &parts[1..]) {
                                println!("{}", output);
                            }
                            continue;
                        }

                        // Expand aliases before execution
                        let expanded = alias_manager.expand(trimmed);
                        let final_cmd = if expanded != trimmed {
                            Output::dim(&format!("→ {}", expanded));
                            expanded
                        } else {
                            trimmed.to_string()
                        };

                        // Time the command execution
                        command_timer.start(&final_cmd);
                        execute_command(&final_cmd, &current_lang, &state, &typo_corrector);
                        if let Some(dur) = command_timer.stop(None) {
                            if let Some(formatted) = command_timer.format_duration(dur) {
                                Output::dim(&format!("⏱  {}", formatted));
                            }
                        }
                    }

                    ShellState::SelectingSearchResult(results) => {
                        if trimmed.starts_with('e') || trimmed.starts_with('E') {
                            if let Ok(num) = trimmed[1..].trim().parse::<usize>() {
                                if num > 0 && num <= results.len() {
                                    let selected = &results[num - 1].0;
                                    Output::info(&format!("Command to edit: {}", selected));
                                    Output::dim("   (Type the command with your modifications)");
                                    shell_state = ShellState::Normal;
                                    continue;
                                }
                            }
                            Output::error("Invalid edit selection. Try 'e1', 'e2', etc.");
                            continue;
                        }

                        if let Ok(num) = trimmed.parse::<usize>() {
                            if num > 0 && num <= results.len() {
                                let selected = &results[num - 1].0;
                                Output::info(&format!("Executing: {}", selected));
                                execute_command(selected, &current_lang, &state, &typo_corrector);
                                shell_state = ShellState::Normal;
                            } else {
                                Output::error(&format!(
                                    "Invalid selection. Try again (1-{}):",
                                    results.len()
                                ));
                            }
                        } else if trimmed.is_empty() {
                            Output::dim("Search cancelled.");
                            shell_state = ShellState::Normal;
                        } else {
                            Output::error(&format!(
                                "Please enter a number (1-{}), 'e<num>' to edit, or Enter to cancel:",
                                results.len()
                            ));
                        }
                    }
                }
            }
            Signal::CtrlC => {
                println!("^C");
                shell_state = ShellState::Normal;
            }
            Signal::CtrlD => {
                Output::success("Goodbye!");
                break;
            }
        }
    }

    Ok(())
}

/// Execute a command and handle special built-in commands
fn execute_command(
    command: &str,
    current_lang: &Arc<RwLock<String>>,
    state: &AppState,
    typo_corrector: &TypoCorrector,
) {
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

        // Check for dangerous commands
        if state.danger_protection {
            if let Some(warning) = output::get_danger_warning(command) {
                warn!("Dangerous command detected: {}", command);
                Output::warn(&warning);
                print!("Are you sure you want to execute this command? [y/N] ");
                io::stdout().flush().ok();

                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_ok() {
                    let response = input.trim().to_lowercase();
                    if response != "y" && response != "yes" {
                        Output::dim("Command cancelled.");
                        return;
                    }
                } else {
                    Output::dim("Command cancelled.");
                    return;
                }
            }
        }

        debug!("Executing command: {}", command);

        // Execute external command
        let status = Command::new("sh").arg("-c").arg(command).status();

        match status {
            Ok(exit_status) => {
                match exit_status.code() {
                    Some(0) => {} // Success, silent
                    Some(127) => {
                        // Command not found - suggest typo corrections
                        Output::exit_code(127);
                        if let Some(message) = typo_corrector.did_you_mean(cmd) {
                            Output::info(&message);
                        }
                    }
                    Some(code) => Output::exit_code(code),
                    None => Output::error("Process terminated by signal"),
                }
            }
            Err(e) => Output::error(&format!("Error executing command: {}", e)),
        }
    }
}

/// Handle 'cd' command with OLDPWD support
fn handle_cd(parts: &[&str]) {
    let current_dir = std::env::current_dir().ok();

    let target_path: Option<PathBuf> = if let Some(path) = parts.get(1) {
        if *path == "-" {
            let old = OLDPWD.lock().unwrap().clone();
            if let Some(ref old_path) = old {
                Output::dim(&old_path.display().to_string());
                Some(old_path.clone())
            } else {
                Output::error("cd: OLDPWD not set");
                None
            }
        } else if path.starts_with('~') {
            dirs::home_dir().map(|home| {
                if *path == "~" {
                    home
                } else {
                    home.join(&path[2..])
                }
            })
        } else {
            Some(PathBuf::from(path))
        }
    } else {
        dirs::home_dir()
    };

    if let Some(target) = target_path {
        if let Err(e) = std::env::set_current_dir(&target) {
            Output::error(&format!("cd: {}: {}", target.display(), e));
        } else if let Some(old) = current_dir {
            *OLDPWD.lock().unwrap() = Some(old);
        }
    }
}

/// Handle 'config' command
fn handle_config(parts: &[&str], current_lang: &Arc<RwLock<String>>) {
    if let Some(sub) = parts.get(1) {
        if *sub == "set-lang" {
            if let Some(lang) = parts.get(2) {
                *current_lang.write().unwrap() = lang.to_string();
                Output::success(&format!("Language switched to: {}", lang));
            } else {
                Output::info("Usage: config set-lang <lang>");
                Output::dim("Available languages: en, zh");
            }
        } else {
            Output::error(&format!("Unknown config subcommand: {}", sub));
            Output::dim("Available: set-lang");
        }
    } else {
        Output::info("Usage: config set-lang <lang>");
    }
}
