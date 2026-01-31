use reedline::{
    default_emacs_keybindings, ColumnarMenu, Emacs, FileBackedHistory, ListMenu, MenuBuilder,
    Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus, Reedline,
    ReedlineEvent, ReedlineMenu, Signal,
};
use std::borrow::Cow;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

mod active_ai;
mod ai;
mod ai_stream;
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
mod session;
mod snippets;
mod timer;
mod ui;
mod validator;
mod watcher;
mod plugins;
mod upgrade;

use active_ai::{ActiveAi, CommandResult, ErrorPatterns};
use ai::{NaturalLanguageTemplates, TypoCorrector};
use ai_stream::{AiModeCommand, AiSession, StreamingAiGenerator};
use aliases::AliasManager;
use session::{NextCommandPredictor, SessionContext};
use bookmarks::BookmarkManager;
use cli::{Cli, Commands, ConfigAction};
use completer::SmartCompleter;
use config::{AiConfig, AppConfig, ProviderType};
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

/// Maximum number of example search results to display
const MAX_EXAMPLE_RESULTS: usize = 20;

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

/// AI Mode prompt with visual indicator
struct AiPrompt {
    provider: String,
}

impl AiPrompt {
    fn new(provider: &str) -> Self {
        Self {
            provider: provider.to_string(),
        }
    }
}

impl Prompt for AiPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{}",
            nu_ansi_term::Color::Magenta.bold().paint(format!("AI [{}]", self.provider))
        ))
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Owned(format!(
            "\n{} ",
            nu_ansi_term::Color::Magenta.paint(">>")
        ))
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(".. ")
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
    /// AI conversation mode - all input goes to AI
    AiMode,
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
        Commands::Upgrade { check, force, yes, target_version } => {
            handle_upgrade(config, check, force, yes, target_version.as_deref())?;
        }
        Commands::Example { command, search } => {
            let definitions_dir = config
                .definitions_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("definitions"));
            let commands = loader::load_commands(&definitions_dir);
            let current_lang = Arc::new(RwLock::new(config.lang.clone()));
            let completer = SmartCompleter::new(commands, current_lang.clone());
            let lang = current_lang.read().unwrap().clone();

            // Handle search flag
            if let Some(query) = search {
                let results = completer.search_examples(&query, &lang);
                display_example_search_results(&results, &query);
                return Ok(());
            }

            // Handle command argument
            if command.is_empty() {
                display_commands_with_examples(&completer, true);
            } else {
                let command_path = command.join(" ");
                let examples = completer.get_examples(&command_path, &lang);
                display_command_examples(&examples, &command_path, true);
            }
        }
    }
    Ok(())
}

/// Handle upgrade command
fn handle_upgrade(
    config: &AppConfig,
    check_only: bool,
    force: bool,
    skip_confirm: bool,
    _target_version: Option<&str>,
) -> anyhow::Result<()> {
    use upgrade::Upgrader;

    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        let upgrader = Upgrader::new(config.upgrade.clone());
        let current = Upgrader::current_version();

        Output::info(&format!("当前版本: {}", current));
        Output::info("正在检查更新...");

        match upgrader.check_for_update().await {
            Ok(Some(info)) => {
                Output::success(&format!(
                    "发现新版本: {} -> {}",
                    current, info.version
                ));

                if let Some(notes) = &info.release_notes {
                    if !notes.is_empty() {
                        Output::dim("\n更新说明:");
                        // Show first few lines of release notes
                        for line in notes.lines().take(10) {
                            Output::dim(&format!("  {}", line));
                        }
                        if notes.lines().count() > 10 {
                            Output::dim("  ...");
                        }
                        println!();
                    }
                }

                if check_only {
                    Output::dim("使用 'sc upgrade' 来安装更新");
                    return Ok(());
                }

                // Confirm upgrade
                if !skip_confirm && !force {
                    print!("是否立即升级? [Y/n]: ");
                    io::stdout().flush().ok();

                    let mut input = String::new();
                    if io::stdin().read_line(&mut input).is_ok() {
                        let response = input.trim().to_lowercase();
                        if !response.is_empty() && response != "y" && response != "yes" {
                            Output::dim("升级已取消");
                            return Ok(());
                        }
                    }
                }

                // Perform upgrade
                match upgrader.upgrade(&info).await {
                    Ok(()) => {
                        Output::success(&format!("升级完成! 已更新到版本 {}", info.version));
                        Output::dim("请重启 shell 以使用新版本");
                    }
                    Err(e) => {
                        Output::error(&format!("升级失败: {}", e));
                        return Err(anyhow::anyhow!("Upgrade failed: {}", e));
                    }
                }
            }
            Ok(None) => {
                Output::success(&format!("已是最新版本: {}", current));
            }
            Err(e) => {
                Output::error(&format!("检查更新失败: {}", e));
                return Err(anyhow::anyhow!("Check failed: {}", e));
            }
        }

        Ok(())
    })
}

fn run_repl(mut config: AppConfig) -> anyhow::Result<()> {
    let definitions_dir = config
        .definitions_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("definitions"));
    let commands = loader::load_commands(&definitions_dir);
    let current_lang = Arc::new(RwLock::new(config.lang.clone()));
    let completer = SmartCompleter::new(commands, current_lang.clone());
    let completer_for_editor = Box::new(completer.clone());
    let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));

    // Create history menu for browsing command history
    let history_menu = Box::new(
        ListMenu::default()
            .with_name("history_menu")
            .with_page_size(15),
    );

    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        reedline::KeyModifiers::NONE,
        reedline::KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );

    // Alt+H - Open history menu
    keybindings.add_binding(
        reedline::KeyModifiers::ALT,
        reedline::KeyCode::Char('h'),
        ReedlineEvent::Menu("history_menu".to_string()),
    );

    // Ctrl+R - History search (prefix filtering)
    keybindings.add_binding(
        reedline::KeyModifiers::CONTROL,
        reedline::KeyCode::Char('r'),
        ReedlineEvent::SearchHistory,
    );

    // Alt+L - AI command generation (requires configuration)
    keybindings.add_binding(
        reedline::KeyModifiers::ALT,
        reedline::KeyCode::Char('l'),
        ReedlineEvent::Edit(vec![reedline::EditCommand::InsertString("?ai ".to_string())]),
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
        .with_menu(ReedlineMenu::HistoryMenu(history_menu))
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
    let mut ai_session = AiSession::new();

    // Initialize Active AI and Session Context
    let mut session_context = SessionContext::new();
    let active_ai = ActiveAi::new(config.ai.active_ai.clone());
    let next_cmd_predictor = NextCommandPredictor::new();

    // Display startup banner
    output::Output::banner();

    // Welcome message
    Output::dim("  Tab         - completion menu    /<keyword>  - search commands");
    Output::dim("  Ctrl+R      - history search     Alt+H       - history menu");
    Output::dim("  ?<query>    - natural language   :<snippet>  - expand snippet");
    Output::dim("  @<bookmark> - jump to bookmark   example     - show command examples");
    Output::dim("  alias/bm    - manage shortcuts   Ctrl-D/exit - quit");
    if config.ai.enabled {
        Output::dim("  ai on       - enter AI mode      ?ai <query> - generate command");
        Output::dim("  explain/??  - explain last error context    - session context");
    }
    println!();

    // Start background version check if enabled
    let version_check_rx = if config.upgrade.auto_check {
        Some(start_background_version_check(config.upgrade.clone()))
    } else {
        None
    };

    debug!("REPL started with config: {:?}", config);

    // Check for version update result (non-blocking)
    if let Some(rx) = version_check_rx {
        // Give the check a moment to complete
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(Some(new_version)) = rx.try_recv() {
            Output::upgrade_available(upgrade::Upgrader::current_version(), &new_version);
        }
    }

    loop {
        // Use different prompt based on shell state
        let sig = match shell_state {
            ShellState::AiMode => {
                let effective = config.ai.get_effective_settings();
                let ai_prompt = AiPrompt::new(&format!("{}", effective.provider_type));
                line_editor.read_line(&ai_prompt)?
            }
            _ => line_editor.read_line(&prompt)?,
        };
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

                        // Handle `command ?` suffix for help mode (categorized options)
                        if trimmed.ends_with(" ?") || trimmed == "?" {
                            let command_path = if trimmed == "?" {
                                String::new()
                            } else {
                                trimmed[..trimmed.len() - 2].trim().to_string()
                            };

                            if command_path.is_empty() {
                                // Show general help
                                let lang = current_lang.read().unwrap().clone();
                                let help_msg = if lang == "zh" {
                                    "用法: <命令> ? 查看该命令的所有选项"
                                } else {
                                    "Usage: <command> ? to see all options for that command"
                                };
                                Output::info(help_msg);
                                Output::dim("  git ?        - Show git options");
                                Output::dim("  git commit ? - Show git commit options");
                                Output::dim("  docker run ? - Show docker run options");
                            } else {
                                // Find the command and show help
                                let parts: Vec<&str> = command_path.split_whitespace().collect();
                                if let Some(root_name) = parts.first() {
                                    if let Some(spec) = completer.get_command_spec(root_name) {
                                        let lang = current_lang.read().unwrap().clone();
                                        // Navigate to subcommand if specified
                                        let mut current_spec = spec;
                                        let mut current_path = root_name.to_string();
                                        for part in parts.iter().skip(1) {
                                            if let Some(sub) = current_spec.subcommands.iter().find(|s| &s.name == part) {
                                                current_spec = sub;
                                                current_path = format!("{} {}", current_path, part);
                                            } else {
                                                break;
                                            }
                                        }
                                        output::display_categorized_help(
                                            &current_path,
                                            &current_spec.subcommands,
                                            &current_spec.flags,
                                            &lang,
                                        );
                                    } else {
                                        Output::warn(&format!("Unknown command: {}", root_name));
                                    }
                                }
                            }
                            continue;
                        }

                        // Handle `?ai` prefix for AI-powered command generation (with streaming)
                        if trimmed.starts_with("?ai ") && trimmed.len() > 4 {
                            let query = &trimmed[4..];

                            if !config.ai.enabled {
                                Output::warn("AI completion is not enabled.");
                                Output::dim("To enable, set ai.enabled = true in config");
                                Output::dim("and configure your API key");
                                continue;
                            }

                            Output::info(&format!("Query: {}", query));
                            let effective = config.ai.get_effective_settings();
                            Output::dim(&format!("  {} streaming...", effective.provider_type));

                            let generator = StreamingAiGenerator::new(&config.ai);
                            let context = ai::llm::AiContext::default();

                            match generator.generate_streaming(query, &context, None) {
                                Ok(raw_response) => {
                                    let response = ai::llm::AiResponse::parse(&raw_response);

                                    if response.commands.is_empty() {
                                        // AI returned prose/explanation, not a command
                                        // The response was already streamed to terminal
                                        println!();
                                    } else if response.is_multi() {
                                        // Multi-command response with descriptions
                                        println!();
                                        Output::success("Generated commands:");
                                        println!();
                                        for (i, entry) in response.commands.iter().enumerate() {
                                            let danger = output::get_danger_warning(&entry.command);
                                            let cmd_display = Output::command(&entry.command);

                                            if let Some(desc) = &entry.description {
                                                println!("  {}. {} - {}", i + 1, cmd_display, desc);
                                            } else {
                                                println!("  {}. {}", i + 1, cmd_display);
                                            }

                                            if let Some(warning) = danger {
                                                Output::warn(&format!("       {}", warning));
                                            }
                                        }

                                        if response.commands.len() > 1 {
                                            print!("\nExecute command [1-{}/n/a(ll)]: ", response.commands.len());
                                        } else {
                                            print!("\nExecute? [Y/n]: ");
                                        }
                                        io::stdout().flush().ok();

                                        let mut input = String::new();
                                        if io::stdin().read_line(&mut input).is_ok() {
                                            let input = input.trim().to_lowercase();
                                            if input == "a" || input == "all" {
                                                let total = response.commands.len();
                                                for (i, entry) in response.commands.iter().enumerate() {
                                                    let desc = entry.description.as_deref().unwrap_or("执行命令");
                                                    Output::step(i + 1, total, desc);
                                                    Output::executing(&entry.command);
                                                    let result = execute_command_with_result(&entry.command, &current_lang, &state, &typo_corrector);
                                                    Output::exec_result(result.0, result.1);
                                                    if !result.0 {
                                                        print!("Continue? [Y/n]: ");
                                                        io::stdout().flush().ok();
                                                        let mut cont = String::new();
                                                        if io::stdin().read_line(&mut cont).is_ok() {
                                                            let cont = cont.trim().to_lowercase();
                                                            if cont == "n" || cont == "no" {
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                            } else if let Ok(num) = input.parse::<usize>() {
                                                if num > 0 && num <= response.commands.len() {
                                                    let entry = &response.commands[num - 1];
                                                    let desc = entry.description.as_deref().unwrap_or("执行命令");
                                                    Output::step(1, 1, desc);
                                                    Output::executing(&entry.command);
                                                    let result = execute_command_with_result(&entry.command, &current_lang, &state, &typo_corrector);
                                                    Output::exec_result(result.0, result.1);
                                                }
                                            } else if input.is_empty() || input == "y" || input == "yes" {
                                                if !response.commands.is_empty() {
                                                    let entry = &response.commands[0];
                                                    let desc = entry.description.as_deref().unwrap_or("执行命令");
                                                    Output::step(1, 1, desc);
                                                    Output::executing(&entry.command);
                                                    let result = execute_command_with_result(&entry.command, &current_lang, &state, &typo_corrector);
                                                    Output::exec_result(result.0, result.1);
                                                }
                                            }
                                        }
                                    } else {
                                        // Single command response
                                        if !response.commands.is_empty() {
                                            let entry = &response.commands[0];
                                            let cmd = &entry.command;
                                            println!();
                                            if let Some(warning) = output::get_danger_warning(cmd) {
                                                Output::warn(&format!(" {}", warning));
                                            }

                                            Output::success(&format!("Generated: {}", Output::command(cmd)));
                                            print!("\nExecute? [Y/n/e(dit)]: ");
                                            io::stdout().flush().ok();

                                            let mut input = String::new();
                                            if io::stdin().read_line(&mut input).is_ok() {
                                                let input = input.trim().to_lowercase();
                                                if input.is_empty() || input == "y" || input == "yes" {
                                                    let desc = entry.description.as_deref().unwrap_or("执行命令");
                                                    Output::step(1, 1, desc);
                                                    Output::executing(cmd);
                                                    let result = execute_command_with_result(cmd, &current_lang, &state, &typo_corrector);
                                                    Output::exec_result(result.0, result.1);
                                                } else if input == "e" || input == "edit" {
                                                    Output::info("Command to edit (copy and modify):");
                                                    println!("  {}", Output::command(cmd));
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    Output::error(&format!("AI error: {}", e));
                                }
                            }
                            continue;
                        }

                        // Handle `?` prefix for natural language queries (local templates)
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

                        // Example command
                        if cmd == "example" || cmd == "examples" || cmd == "ex" {
                            let lang = current_lang.read().unwrap().clone();
                            handle_example_command(&completer, &parts[1..], &lang);
                            continue;
                        }

                        // AI management commands
                        if cmd == "ai" {
                            let subcommand = parts.get(1).map(|s| *s).unwrap_or("status");
                            let args = if parts.len() > 2 { &parts[2..] } else { &[] };

                            // Handle "ai on" to enter AI mode
                            if subcommand == "on" || subcommand == "start" || subcommand == "enter" || subcommand == "mode" {
                                if !config.ai.enabled {
                                    Output::warn("AI is not enabled. Set ai.enabled = true in config.");
                                    Output::dim("Run: config edit");
                                    continue;
                                }
                                let effective = config.ai.get_effective_settings();
                                ai_session.enter();
                                shell_state = ShellState::AiMode;
                                ai_stream::show_ai_mode_welcome(
                                    &format!("{}", effective.provider_type),
                                    effective.model.as_deref(),
                                );
                                continue;
                            }

                            handle_ai_command(&mut config.ai, subcommand, args);
                            continue;
                        }

                        // Explain command - explain last error
                        if cmd == "explain" || cmd == "??" {
                            if !config.ai.enabled {
                                Output::warn("AI is not enabled. Set ai.enabled = true in config.");
                                continue;
                            }

                            if let Some(last_err) = session_context.last_error() {
                                Output::dim(&format!("  Analyzing: {}", last_err.command));
                                let cmd_result = CommandResult::new(
                                    &last_err.command,
                                    last_err.exit_code,
                                    last_err.stdout.clone(),
                                    last_err.stderr.clone(),
                                );
                                match active_ai.explain_error(&cmd_result, &config.ai) {
                                    Ok(explanation) => {
                                        Output::active_ai_explain(&explanation);
                                    }
                                    Err(e) => {
                                        Output::error(&e);
                                    }
                                }
                            } else {
                                Output::dim("No recent error to explain.");
                            }
                            continue;
                        }

                        // Context command - show session context
                        if cmd == "context" {
                            let subcommand = parts.get(1).map(|s| *s).unwrap_or("show");
                            match subcommand {
                                "show" | "status" => {
                                    let stats = session_context.stats();
                                    Output::session_summary(
                                        stats.total_commands,
                                        stats.failed_commands,
                                        &stats.format_duration(),
                                    );
                                    println!();
                                    Output::info("Recent commands:");
                                    for entry in session_context.recent(5).iter().rev() {
                                        let status = if entry.is_success() { "✓" } else { "✗" };
                                        Output::dim(&format!("  [{}] {}", status, entry.command));
                                    }
                                }
                                "clear" => {
                                    session_context.clear();
                                    Output::success("Session context cleared.");
                                }
                                "errors" => {
                                    let errors = session_context.recent_errors(5);
                                    if errors.is_empty() {
                                        Output::dim("No recent errors.");
                                    } else {
                                        Output::info("Recent errors:");
                                        for entry in errors.iter().rev() {
                                            Output::dim(&format!("  [exit {}] {}",
                                                entry.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "?".to_string()),
                                                entry.command
                                            ));
                                            if let Some(ref stderr) = entry.stderr {
                                                if !stderr.is_empty() && stderr.len() < 100 {
                                                    Output::dim(&format!("    → {}", stderr.trim()));
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    Output::dim("Usage: context [show|clear|errors]");
                                }
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
                        let cmd_result = execute_command_for_active_ai(&final_cmd, &current_lang, &state, &typo_corrector);
                        let duration = command_timer.stop(None);

                        if let Some(dur) = duration {
                            if let Some(formatted) = command_timer.format_duration(dur) {
                                Output::dim(&format!("⏱  {}", formatted));
                            }
                        }

                        // Track in session context
                        session_context.record(&cmd_result, duration);

                        // Active AI: Show proactive suggestions on error
                        if active_ai.should_handle(&cmd_result) && config.ai.enabled {
                            // First, try quick hints without AI
                            if let Some(error_type) = ErrorPatterns::detect_error_type(&cmd_result) {
                                if let Some(hint) = ErrorPatterns::get_quick_hint(&error_type, &cmd_result) {
                                    Output::quick_error_hint(&hint);
                                }
                            }

                            // Show Active AI prompt
                            Output::active_ai_hint();

                            // Read user input for Active AI action
                            let mut ai_input = String::new();
                            if io::stdin().read_line(&mut ai_input).is_ok() {
                                let ai_trimmed = ai_input.trim().to_lowercase();
                                match ai_trimmed.as_str() {
                                    "e" | "explain" => {
                                        Output::dim("  Analyzing error...");
                                        match active_ai.explain_error(&cmd_result, &config.ai) {
                                            Ok(explanation) => {
                                                Output::active_ai_explain(&explanation);
                                            }
                                            Err(e) => {
                                                Output::error(&e);
                                            }
                                        }
                                    }
                                    "f" | "fix" => {
                                        Output::dim("  Generating fix...");
                                        match active_ai.suggest_fix(&cmd_result, &config.ai) {
                                            Ok(fix) => {
                                                let fix_cmd = fix.trim();
                                                Output::active_ai_fix(fix_cmd);
                                                print!("Execute fix? [Y/n]: ");
                                                io::stdout().flush().ok();

                                                let mut fix_input = String::new();
                                                if io::stdin().read_line(&mut fix_input).is_ok() {
                                                    let response = fix_input.trim().to_lowercase();
                                                    if response.is_empty() || response == "y" || response == "yes" {
                                                        let fix_result = execute_command_for_active_ai(fix_cmd, &current_lang, &state, &typo_corrector);
                                                        session_context.record(&fix_result, None);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                Output::error(&e);
                                            }
                                        }
                                    }
                                    "r" | "retry" => {
                                        Output::info("Retrying command...");
                                        let retry_result = execute_command_for_active_ai(&final_cmd, &current_lang, &state, &typo_corrector);
                                        session_context.record(&retry_result, None);
                                    }
                                    _ => {
                                        // User pressed Enter or something else, skip
                                    }
                                }
                            }
                        }

                        // Next command prediction (show hint for next likely command)
                        if config.ai.next_command.enabled && cmd_result.success {
                            if let Some((predicted, confidence)) = next_cmd_predictor.predict(&final_cmd, &session_context) {
                                if confidence >= config.ai.next_command.min_confidence {
                                    Output::next_command_hint(&predicted, confidence);
                                }
                            }
                        } else if !cmd_result.success {
                            // After error, suggest recovery command
                            if let Some((fix_cmd, confidence)) = next_cmd_predictor.predict_after_error(&cmd_result) {
                                if confidence >= 0.5 {
                                    Output::next_command_hint(&fix_cmd, confidence);
                                }
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

                    ShellState::AiMode => {
                        // Handle AI mode commands
                        if let Some(cmd) = ai_stream::parse_ai_mode_command(trimmed) {
                            match cmd {
                                AiModeCommand::Exit => {
                                    ai_session.exit();
                                    shell_state = ShellState::Normal;
                                    ai_stream::show_ai_mode_exit();
                                    continue;
                                }
                                AiModeCommand::Clear => {
                                    ai_session.clear();
                                    Output::success("Conversation history cleared.");
                                    continue;
                                }
                                AiModeCommand::Help => {
                                    ai_stream::show_ai_mode_help();
                                    continue;
                                }
                                AiModeCommand::Enter => {
                                    Output::dim("Already in AI mode.");
                                    continue;
                                }
                            }
                        }

                        // Empty input - just continue
                        if trimmed.is_empty() {
                            continue;
                        }

                        // Handle "exit" in AI mode
                        if trimmed == "exit" {
                            ai_session.exit();
                            shell_state = ShellState::Normal;
                            ai_stream::show_ai_mode_exit();
                            continue;
                        }

                        // Process AI query with streaming
                        let effective = config.ai.get_effective_settings();
                        Output::dim(&format!("  {} thinking...", effective.provider_type));

                        let generator = StreamingAiGenerator::new(&config.ai);
                        let context = ai::llm::AiContext::default();

                        // Add user message to session
                        ai_session.add_user_message(trimmed);

                        match generator.generate_streaming(trimmed, &context, Some(&ai_session)) {
                            Ok(response) => {
                                // Add assistant response to session
                                ai_session.add_assistant_message(&response);

                                // Parse response for commands
                                let parsed = ai::llm::AiResponse::parse(&response);

                                if parsed.commands.is_empty() {
                                    // AI returned prose/explanation, not a command
                                    // The response was already streamed to terminal, just add newline
                                    println!();
                                } else if parsed.commands.len() == 1 {
                                    let entry = &parsed.commands[0];
                                    let cmd = &entry.command;
                                    println!();
                                    if let Some(warning) = output::get_danger_warning(cmd) {
                                        Output::warn(&format!("  {}", warning));
                                    }
                                    print!("Execute? [Y/n/e(dit)]: ");
                                    io::stdout().flush().ok();

                                    let mut input = String::new();
                                    if io::stdin().read_line(&mut input).is_ok() {
                                        let input = input.trim().to_lowercase();
                                        if input.is_empty() || input == "y" || input == "yes" {
                                            // Show step info
                                            let desc = entry.description.as_deref().unwrap_or("执行命令");
                                            Output::step(1, 1, desc);
                                            Output::executing(cmd);
                                            let result = execute_command_with_result(cmd, &current_lang, &state, &typo_corrector);
                                            Output::exec_result(result.0, result.1);
                                        } else if input == "e" || input == "edit" {
                                            Output::info("Command to edit (copy and modify):");
                                            println!("  {}", Output::command(cmd));
                                        }
                                    }
                                } else {
                                    // Multiple commands
                                    println!();
                                    Output::dim("Generated commands:");
                                    for (i, entry) in parsed.commands.iter().enumerate() {
                                        if let Some(desc) = &entry.description {
                                            println!("  {}. {} - {}", i + 1, Output::command(&entry.command), desc);
                                        } else {
                                            println!("  {}. {}", i + 1, Output::command(&entry.command));
                                        }
                                    }
                                    print!("\nExecute [1-{}/n/a(ll)]: ", parsed.commands.len());
                                    io::stdout().flush().ok();

                                    let mut input = String::new();
                                    if io::stdin().read_line(&mut input).is_ok() {
                                        let input = input.trim().to_lowercase();
                                        if input == "a" || input == "all" {
                                            let total = parsed.commands.len();
                                            for (i, entry) in parsed.commands.iter().enumerate() {
                                                let desc = entry.description.as_deref().unwrap_or("执行命令");
                                                Output::step(i + 1, total, desc);
                                                Output::executing(&entry.command);
                                                let result = execute_command_with_result(&entry.command, &current_lang, &state, &typo_corrector);
                                                Output::exec_result(result.0, result.1);
                                                // If command failed, ask if continue
                                                if !result.0 {
                                                    print!("Continue? [Y/n]: ");
                                                    io::stdout().flush().ok();
                                                    let mut cont = String::new();
                                                    if io::stdin().read_line(&mut cont).is_ok() {
                                                        let cont = cont.trim().to_lowercase();
                                                        if cont == "n" || cont == "no" {
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        } else if let Ok(num) = input.parse::<usize>() {
                                            if num > 0 && num <= parsed.commands.len() {
                                                let entry = &parsed.commands[num - 1];
                                                let desc = entry.description.as_deref().unwrap_or("执行命令");
                                                Output::step(1, 1, desc);
                                                Output::executing(&entry.command);
                                                let result = execute_command_with_result(&entry.command, &current_lang, &state, &typo_corrector);
                                                Output::exec_result(result.0, result.1);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                Output::error(&format!("AI error: {}", e));
                            }
                        }
                    }
                }
            }
            Signal::CtrlC => {
                println!("^C");
                if ai_session.is_active() {
                    ai_session.exit();
                    ai_stream::show_ai_mode_exit();
                }
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

/// Execute a command and return the result (success, exit_code)
/// This is used by AI commands to show execution results
fn execute_command_with_result(
    command: &str,
    current_lang: &Arc<RwLock<String>>,
    state: &AppState,
    typo_corrector: &TypoCorrector,
) -> (bool, Option<i32>) {
    let parts: Vec<&str> = command.split_whitespace().collect();

    if let Some(cmd) = parts.first() {
        // Handle 'cd' command
        if *cmd == "cd" {
            handle_cd(&parts);
            return (true, Some(0));
        }

        // Handle 'config' command
        if *cmd == "config" {
            handle_config(&parts, current_lang);
            return (true, Some(0));
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
                        return (false, None);
                    }
                } else {
                    Output::dim("Command cancelled.");
                    return (false, None);
                }
            }
        }

        debug!("Executing command: {}", command);

        // Execute external command
        let status = Command::new("sh").arg("-c").arg(command).status();

        match status {
            Ok(exit_status) => {
                let code = exit_status.code();
                match code {
                    Some(0) => (true, Some(0)),
                    Some(127) => {
                        // Command not found - suggest typo corrections
                        if let Some(message) = typo_corrector.did_you_mean(cmd) {
                            Output::info(&message);
                        }
                        (false, Some(127))
                    }
                    Some(c) => (false, Some(c)),
                    None => (false, None),
                }
            }
            Err(e) => {
                Output::error(&format!("Error executing command: {}", e));
                (false, None)
            }
        }
    } else {
        (false, None)
    }
}

/// Execute a command and return CommandResult for Active AI integration
/// This captures stderr for error analysis while still showing output in real-time
fn execute_command_for_active_ai(
    command: &str,
    current_lang: &Arc<RwLock<String>>,
    state: &AppState,
    typo_corrector: &TypoCorrector,
) -> CommandResult {
    let parts: Vec<&str> = command.split_whitespace().collect();

    if let Some(cmd) = parts.first() {
        // Handle 'cd' command
        if *cmd == "cd" {
            handle_cd(&parts);
            return CommandResult::new(command, Some(0), None, None);
        }

        // Handle 'config' command
        if *cmd == "config" {
            handle_config(&parts, current_lang);
            return CommandResult::new(command, Some(0), None, None);
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
                        return CommandResult::new(command, None, None, Some("Cancelled by user".to_string()));
                    }
                } else {
                    Output::dim("Command cancelled.");
                    return CommandResult::new(command, None, None, Some("Cancelled by user".to_string()));
                }
            }
        }

        debug!("Executing command: {}", command);

        // Execute external command with output capture
        // We use output() instead of status() to capture stderr for error analysis
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output();

        match output {
            Ok(output) => {
                let code = output.status.code();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                // Print stdout if not empty
                if !stdout.is_empty() {
                    print!("{}", stdout);
                }

                // Print stderr if not empty (to stderr)
                if !stderr.is_empty() {
                    eprint!("{}", stderr);
                }

                match code {
                    Some(0) => {} // Success, silent
                    Some(127) => {
                        // Command not found - suggest typo corrections
                        Output::exit_code(127);
                        if let Some(message) = typo_corrector.did_you_mean(cmd) {
                            Output::info(&message);
                        }
                    }
                    Some(c) => Output::exit_code(c),
                    None => Output::error("Process terminated by signal"),
                }

                CommandResult::new(
                    command,
                    code,
                    if stdout.is_empty() { None } else { Some(stdout) },
                    if stderr.is_empty() { None } else { Some(stderr) },
                )
            }
            Err(e) => {
                let error_msg = format!("Error executing command: {}", e);
                Output::error(&error_msg);
                CommandResult::new(command, None, None, Some(error_msg))
            }
        }
    } else {
        CommandResult::new(command, None, None, None)
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

/// Start background version check
fn start_background_version_check(
    config: config::UpgradeConfig,
) -> std::sync::mpsc::Receiver<Option<String>> {
    use std::sync::mpsc;
    use upgrade::Upgrader;

    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();

        if let Ok(rt) = rt {
            let result = rt.block_on(async {
                let upgrader = Upgrader::new(config);
                upgrader.check_for_update().await.ok().flatten()
            });

            let _ = tx.send(result.map(|info| info.version));
        }
    });

    rx
}

/// Handle 'config' command
fn handle_config(parts: &[&str], current_lang: &Arc<RwLock<String>>) {
    let sub = parts.get(1).map(|s| *s).unwrap_or("help");

    match sub {
        "set-lang" => {
            if let Some(lang) = parts.get(2) {
                *current_lang.write().unwrap() = lang.to_string();
                Output::success(&format!("Language switched to: {}", lang));
            } else {
                Output::info("Usage: config set-lang <lang>");
                Output::dim("Available languages: en, zh");
            }
        }

        "check" | "validate" => {
            // Validate config file
            let config_path = AppConfig::config_file_path();
            println!();
            Output::info(&format!("Validating config: {}", config_path.display()));
            println!();

            if !config_path.exists() {
                Output::warn("Config file not found.");
                Output::dim(&format!("Create one with: config example > {}", config_path.display()));
                return;
            }

            // Try to load and parse the config
            match AppConfig::load() {
                Ok(config) => {
                    Output::success("✓ Config file is valid!");
                    println!();

                    // Show summary
                    Output::dim("Summary:");
                    println!("  Language:          {}", config.lang);
                    println!("  History size:      {}", config.history_size);
                    println!("  Danger protection: {}", config.danger_protection);
                    println!("  AI enabled:        {}", config.ai.enabled);
                    if config.ai.enabled {
                        println!("  AI provider:       {}", config.ai.active);
                    }

                    // Validate AI config
                    if config.ai.enabled {
                        println!();
                        Output::dim("AI Configuration:");
                        if let Some(provider) = config.ai.get_active_provider() {
                            let key_status = match &provider.api_key {
                                Some(key) if key.starts_with('$') => {
                                    let var_name = &key[1..];
                                    if std::env::var(var_name).is_ok() {
                                        format!("✓ {} (set)", key)
                                    } else {
                                        format!("✗ {} (not set)", key)
                                    }
                                }
                                Some(_) => "⚠ plain text (not recommended)".to_string(),
                                None => {
                                    if provider.provider_type == ProviderType::Ollama {
                                        "✓ not required".to_string()
                                    } else {
                                        "✗ not configured".to_string()
                                    }
                                }
                            };
                            println!("  API Key: {}", key_status);
                            if let Some(ref model) = provider.model {
                                println!("  Model:   {}", model);
                            }
                            if let Some(ref endpoint) = provider.endpoint {
                                println!("  Endpoint: {}", endpoint);
                            }
                        }
                    }
                }
                Err(e) => {
                    Output::error(&format!("✗ Config file has errors: {}", e));
                    println!();
                    Output::dim("Common issues:");
                    Output::dim("  - Missing quotes around string values");
                    Output::dim("  - Invalid TOML syntax");
                    Output::dim("  - Unknown field names");
                    Output::dim("  - Wrong value types (e.g., string instead of number)");
                }
            }
        }

        "show" => {
            // Show current config
            let config_path = AppConfig::config_file_path();
            if config_path.exists() {
                match std::fs::read_to_string(&config_path) {
                    Ok(content) => {
                        println!();
                        Output::info(&format!("Config file: {}", config_path.display()));
                        println!();
                        println!("{}", content);
                    }
                    Err(e) => Output::error(&format!("Failed to read config: {}", e)),
                }
            } else {
                Output::warn("Config file not found.");
                Output::dim(&format!("Expected at: {}", config_path.display()));
            }
        }

        "path" => {
            let config_path = AppConfig::config_file_path();
            println!("{}", config_path.display());
        }

        "edit" => {
            let config_path = AppConfig::config_file_path();

            // Create config directory and file if not exists
            if !config_path.exists() {
                if let Some(parent) = config_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let example = config::generate_example_config();
                if std::fs::write(&config_path, &example).is_err() {
                    Output::error("Failed to create config file");
                    return;
                }
                Output::success(&format!("Created config file: {}", config_path.display()));
            }

            // Try to open in editor
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
            Output::info(&format!("Opening {} with {}...", config_path.display(), editor));

            let status = std::process::Command::new(&editor)
                .arg(&config_path)
                .status();

            match status {
                Ok(s) if s.success() => {
                    Output::success("Config file saved. Restart sc to apply changes.");
                }
                Ok(_) => Output::warn("Editor exited with non-zero status"),
                Err(e) => {
                    Output::error(&format!("Failed to open editor: {}", e));
                    Output::dim(&format!("Set EDITOR env var or edit manually: {}", config_path.display()));
                }
            }
        }

        "example" => {
            // Generate example config
            let example = config::generate_example_config();
            println!("{}", example);
        }

        "init" => {
            // Initialize config file with example
            let config_path = AppConfig::config_file_path();

            if config_path.exists() {
                Output::warn(&format!("Config file already exists: {}", config_path.display()));
                Output::dim("Use 'config edit' to modify or 'config show' to view");
                return;
            }

            // Create directory
            if let Some(parent) = config_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    Output::error(&format!("Failed to create directory: {}", e));
                    return;
                }
            }

            // Write example config
            let example = config::generate_example_config();
            match std::fs::write(&config_path, &example) {
                Ok(_) => {
                    Output::success(&format!("Created config file: {}", config_path.display()));
                    Output::dim("Edit with: config edit");
                }
                Err(e) => Output::error(&format!("Failed to write config: {}", e)),
            }
        }

        "help" | _ => {
            println!();
            Output::info("Config Commands");
            println!();
            println!("  {}       - Validate config file", Color::Cyan.paint("config check"));
            println!("  {}        - Show current config", Color::Cyan.paint("config show"));
            println!("  {}        - Show config file path", Color::Cyan.paint("config path"));
            println!("  {}        - Edit config file in $EDITOR", Color::Cyan.paint("config edit"));
            println!("  {}        - Initialize config file", Color::Cyan.paint("config init"));
            println!("  {}     - Print example config", Color::Cyan.paint("config example"));
            println!("  {} - Set display language", Color::Cyan.paint("config set-lang <en|zh>"));
            println!();
            Output::dim(&format!("Config file: {}", AppConfig::config_file_path().display()));
            println!();
        }
    }
}

/// Display a list of commands that have examples
fn display_commands_with_examples(completer: &SmartCompleter, is_cli: bool) {
    let commands = completer.get_commands_with_examples();
    Output::info("Commands with examples:");
    println!();

    // Group by root command for better display
    let mut current_root = String::new();
    for cmd in &commands {
        let root = cmd.split_whitespace().next().unwrap_or(cmd);
        if root != current_root {
            if !current_root.is_empty() {
                println!();
            }
            current_root = root.to_string();
        }
        println!("  {}", Output::command(cmd));
    }

    println!();
    Output::dim("Usage:");
    if is_cli {
        Output::dim("  sc example <command>         Show examples for a command");
        Output::dim("  sc example <cmd> <subcmd>    Show examples for a subcommand");
        Output::dim("  sc example -s <query>        Search all examples");
    } else {
        Output::dim("  example <command>           Show examples for a command");
        Output::dim("  example <cmd> <subcmd>      Show examples for a subcommand");
        Output::dim("  example search <query>      Search all examples");
    }
}

/// Display search results for examples
fn display_example_search_results(results: &[(String, String, String)], query: &str) {
    if results.is_empty() {
        Output::warn(&format!("No examples found for: {}", query));
        return;
    }

    Output::info(&format!("Examples matching '{}':", query));
    println!();

    let num_style = nu_ansi_term::Style::new().fg(nu_ansi_term::Color::DarkGray);
    let path_style = nu_ansi_term::Style::new().fg(nu_ansi_term::Color::Cyan);

    for (i, (path, cmd, scenario)) in results.iter().take(MAX_EXAMPLE_RESULTS).enumerate() {
        println!(
            "  {}. {} {}",
            num_style.paint(format!("{:>2}", i + 1)),
            path_style.paint(format!("[{}]", path)),
            Output::command(cmd)
        );
        Output::dim(&format!("      → {}", scenario));
    }

    if results.len() > MAX_EXAMPLE_RESULTS {
        println!();
        Output::dim(&format!(
            "  ... and {} more results",
            results.len() - MAX_EXAMPLE_RESULTS
        ));
    }
}

/// Display examples for a specific command
fn display_command_examples(examples: &[(String, String)], command_path: &str, is_cli: bool) {
    if examples.is_empty() {
        Output::warn(&format!("No examples found for: {}", command_path));
        if is_cli {
            Output::dim(
                "Try 'sc example' to see commands with examples, or 'sc example -s <query>' to search.",
            );
        } else {
            Output::dim(
                "Try 'example' to see commands with examples, or 'example search <query>' to search.",
            );
        }
        return;
    }

    Output::info(&format!("Examples for '{}':", command_path));
    println!();

    let num_style = nu_ansi_term::Style::new().fg(nu_ansi_term::Color::DarkGray);

    for (i, (cmd, scenario)) in examples.iter().enumerate() {
        println!(
            "  {}. {}",
            num_style.paint(format!("{:>2}", i + 1)),
            Output::command(cmd)
        );
        Output::dim(&format!("      → {}", scenario));
    }
    println!();
}

/// Handle 'example' command - display examples for commands (REPL version)
fn handle_example_command(completer: &SmartCompleter, args: &[&str], lang: &str) {
    if args.is_empty() {
        display_commands_with_examples(completer, false);
        return;
    }

    // Handle search subcommand
    if args[0] == "search" || args[0] == "s" {
        if args.len() < 2 {
            Output::warn("Usage: example search <query>");
            return;
        }

        let query = args[1..].join(" ");
        let results = completer.search_examples(&query, lang);
        display_example_search_results(&results, &query);
        return;
    }

    // Show examples for a specific command
    let command_path = args.join(" ");
    let examples = completer.get_examples(&command_path, lang);
    display_command_examples(&examples, &command_path, false);
}

/// Handle 'ai' command - manage AI providers
fn handle_ai_command(ai_config: &mut AiConfig, subcommand: &str, args: &[&str]) {
    match subcommand {
        "status" | "info" => {
            // Show current AI configuration status
            println!();
            Output::info("AI Configuration Status");
            println!();

            let enabled_str = if ai_config.enabled {
                format!("{}", Color::Green.paint("enabled"))
            } else {
                format!("{}", Color::Red.paint("disabled"))
            };
            println!("  Status:  {}", enabled_str);
            println!("  Active:  {}", Color::Cyan.paint(&ai_config.active));

            if let Some(provider) = ai_config.get_active_provider() {
                println!("  Type:    {}", provider.provider_type);
                if let Some(ref model) = provider.model {
                    println!("  Model:   {}", model);
                }
                if let Some(ref endpoint) = provider.endpoint {
                    println!("  Endpoint: {}", endpoint);
                }

                // Check if API key is configured
                let key_status = match &provider.api_key {
                    Some(key) if key.starts_with('$') => {
                        let var_name = &key[1..];
                        if std::env::var(var_name).is_ok() {
                            format!("{} (via {})", Color::Green.paint("configured"), key)
                        } else {
                            format!("{} ({} not set)", Color::Yellow.paint("missing"), key)
                        }
                    }
                    Some(_) => format!("{}", Color::Yellow.paint("configured (plain text - not recommended)")),
                    None => {
                        if provider.provider_type == ProviderType::Ollama {
                            format!("{}", Color::Green.paint("not required (local)"))
                        } else {
                            format!("{}", Color::Red.paint("not configured"))
                        }
                    }
                };
                println!("  API Key: {}", key_status);
            }

            println!();
            Output::dim("Commands:");
            Output::dim("  ai list              - List all configured providers");
            Output::dim("  ai use <provider>    - Switch to a different provider");
            Output::dim("  ai test              - Test the current provider connection");
            Output::dim("  ai providers         - Show available provider types");
            Output::dim("  config edit          - Edit config file");
            println!();
        }

        "list" | "ls" => {
            // List all configured providers
            println!();
            Output::info("Configured AI Providers");
            println!();

            let providers = ai_config.list_providers();
            if providers.is_empty() {
                Output::warn("No providers configured");
                return;
            }

            for (name, config) in providers {
                let active_marker = if name == &ai_config.active {
                    format!("{}", Color::Green.paint(" ✓ (active)"))
                } else {
                    String::new()
                };

                let model = config.model.as_deref().unwrap_or("default");
                println!(
                    "  {} [{}/{}]{}",
                    Color::Cyan.paint(name),
                    config.provider_type,
                    model,
                    active_marker
                );
            }
            println!();
        }

        "use" | "switch" => {
            // Switch to a different provider
            if args.is_empty() {
                Output::warn("Usage: ai use <provider>");
                Output::dim("  Available providers: ai list");
                return;
            }

            let provider_name = args[0];
            match ai_config.switch_provider(provider_name) {
                Ok(()) => {
                    Output::success(&format!("Switched to provider: {}", provider_name));
                    if let Some(provider) = ai_config.get_active_provider() {
                        if let Some(ref model) = provider.model {
                            Output::dim(&format!("  Model: {}", model));
                        }
                    }
                }
                Err(e) => {
                    Output::error(&e);
                }
            }
        }

        "test" => {
            // Test the current provider connection
            if !ai_config.enabled {
                Output::warn("AI is not enabled. Set ai.enabled = true in config.");
                return;
            }

            Output::info(&format!("Testing connection to {}...", ai_config.active));

            let generator = ai::llm::AiCommandGenerator::new(ai_config);
            match generator.test_connection() {
                Ok(msg) => Output::success(&msg),
                Err(e) => Output::error(&format!("Connection failed: {}", e)),
            }
        }

        "providers" | "types" => {
            // Show available provider types
            println!();
            Output::info("Available AI Provider Types");
            println!();

            let providers = [
                ("claude", "Anthropic Claude", "claude-sonnet-4, claude-opus-4"),
                ("openai", "OpenAI GPT", "gpt-4o, gpt-4o-mini, o1"),
                ("gemini", "Google Gemini", "gemini-2.0-flash, gemini-1.5-pro"),
                ("deepseek", "DeepSeek", "deepseek-chat, deepseek-reasoner"),
                ("glm", "智谱AI GLM", "glm-4-plus, glm-4-flash"),
                ("qwen", "阿里通义千问", "qwen-max, qwen-plus"),
                ("ollama", "Ollama (Local)", "qwen2.5, llama3.2, deepseek-r1"),
                ("openrouter", "OpenRouter", "Access multiple providers"),
                ("custom", "Custom", "Any OpenAI-compatible API"),
            ];

            for (name, display, desc) in providers {
                println!(
                    "  {:12} {:20} {}",
                    Color::Cyan.paint(name),
                    display,
                    Color::DarkGray.paint(desc)
                );
            }

            println!();
            Output::dim("Configure in ~/.config/smart-command/config.toml:");
            Output::dim("  [ai.providers.my_provider]");
            Output::dim("  provider_type = \"openai\"");
            Output::dim("  api_key = \"$OPENAI_API_KEY\"");
            Output::dim("  endpoint = \"https://your-proxy.com/v1/chat/completions\"");
            Output::dim("  model = \"gpt-4o-mini\"");
            println!();
        }

        "enable" => {
            ai_config.enabled = true;
            Output::success("AI completion enabled");
        }

        "disable" => {
            ai_config.enabled = false;
            Output::success("AI completion disabled");
        }

        "help" | "?" => {
            println!();
            Output::info("AI Command Help");
            println!();
            println!("  {}      - Show current AI configuration", Color::Cyan.paint("ai status"));
            println!("  {}        - List all configured providers", Color::Cyan.paint("ai list"));
            println!("  {} - Switch to a different provider", Color::Cyan.paint("ai use <name>"));
            println!("  {}        - Test the current provider connection", Color::Cyan.paint("ai test"));
            println!("  {}   - Show available provider types", Color::Cyan.paint("ai providers"));
            println!("  {}      - Enable AI completion", Color::Cyan.paint("ai enable"));
            println!("  {}     - Disable AI completion", Color::Cyan.paint("ai disable"));
            println!();
            Output::dim("Generate commands with AI:");
            Output::dim("  ?ai <query>   - Generate a command from natural language");
            Output::dim("  Alt+L         - Quick AI input");
            println!();
        }

        _ => {
            Output::error(&format!("Unknown ai subcommand: {}", subcommand));
            Output::dim("Try: ai help");
        }
    }
}
