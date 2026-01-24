//! CLI argument parsing with clap
//!
//! Supports:
//! - Subcommands for utility operations
//! - Shell completion generation
//! - Environment variable overrides

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::io;
use std::path::PathBuf;

/// Smart Command - An intelligent shell with context-aware completion
#[derive(Parser, Debug)]
#[command(name = "smart-command")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Set the display language
    #[arg(short, long, env = "SMART_CMD_LANG")]
    pub lang: Option<String>,

    /// Custom definitions directory
    #[arg(short, long, env = "SMART_CMD_DEFINITIONS_DIR")]
    pub definitions: Option<PathBuf>,

    /// Enable verbose/debug output
    #[arg(short, long, env = "SMART_CMD_VERBOSE")]
    pub verbose: bool,

    /// Disable dangerous command protection
    #[arg(long)]
    pub no_danger_protection: bool,

    /// Execute a single command and exit
    #[arg(short = 'c', long)]
    pub command: Option<String>,

    #[command(subcommand)]
    pub subcommand: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate shell completion scripts
    Completions {
        /// Shell type to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Show or generate configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Search commands by keyword
    Search {
        /// Search query
        query: String,
    },

    /// List all available commands
    List,

    /// Install binary and definitions to standard locations
    Install {
        /// Destination directory for binary (default: platform user bin)
        #[arg(long)]
        bin_dir: Option<PathBuf>,

        /// Destination directory for definitions (default: platform config dir)
        #[arg(long)]
        definitions_dir: Option<PathBuf>,

        /// Source directory for definitions (default: auto-discover)
        #[arg(long)]
        definitions_src: Option<PathBuf>,

        /// Skip copying the binary
        #[arg(long)]
        skip_bin: bool,

        /// Skip copying definitions
        #[arg(long)]
        skip_definitions: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Generate example configuration file
    Generate,
    /// Show configuration file path
    Path,
}

impl Cli {
    /// Parse CLI arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Generate shell completions and write to stdout
    pub fn generate_completions(shell: Shell) {
        let mut cmd = Self::command();
        let name = cmd.get_name().to_string();
        generate(shell, &mut cmd, name, &mut io::stdout());
    }

    /// Get completions as string for specific shell
    #[allow(dead_code)]
    pub fn get_completions_string(shell: Shell) -> String {
        let mut cmd = Self::command();
        let name = cmd.get_name().to_string();
        let mut buf = Vec::new();
        generate(shell, &mut cmd, name, &mut buf);
        String::from_utf8(buf).unwrap_or_default()
    }
}

/// Print installation instructions for shell completions
pub fn print_completion_instructions(shell: Shell) {
    match shell {
        Shell::Bash => {
            println!("# Add to ~/.bashrc:");
            println!("eval \"$(smart-command completions bash)\"");
            println!();
            println!("# Or save to file:");
            println!("smart-command completions bash > ~/.local/share/bash-completion/completions/smart-command");
        }
        Shell::Zsh => {
            println!("# Add to ~/.zshrc:");
            println!("eval \"$(smart-command completions zsh)\"");
            println!();
            println!("# Or save to fpath directory:");
            println!("smart-command completions zsh > ~/.zsh/completions/_smart-command");
        }
        Shell::Fish => {
            println!("# Save to fish completions directory:");
            println!(
                "smart-command completions fish > ~/.config/fish/completions/smart-command.fish"
            );
        }
        Shell::PowerShell => {
            println!("# Add to PowerShell profile:");
            println!("Invoke-Expression (smart-command completions powershell | Out-String)");
        }
        _ => {
            println!("# Pipe completions output to appropriate location for your shell");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test default parsing
        let cli = Cli::parse_from(["smart-command"]);
        assert!(cli.lang.is_none());
        assert!(!cli.verbose);

        // Test with flags
        let cli = Cli::parse_from(["smart-command", "-l", "zh", "-v"]);
        assert_eq!(cli.lang, Some("zh".to_string()));
        assert!(cli.verbose);
    }

    #[test]
    fn test_completions_generation() {
        // Just verify it doesn't panic
        let completions = Cli::get_completions_string(Shell::Bash);
        assert!(!completions.is_empty());
    }
}
