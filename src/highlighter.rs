//! Real-Time Syntax Highlighting
//!
//! Provides syntax highlighting for command input as the user types.

#![allow(dead_code)]

use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};
use std::collections::HashSet;
use std::path::Path;

/// Syntax theme for highlighting
#[derive(Debug, Clone)]
pub struct SyntaxTheme {
    /// Valid command names
    pub command: Style,
    /// Unknown command names
    pub unknown_command: Style,
    /// Valid subcommands
    pub subcommand: Style,
    /// Short flags (-f)
    pub flag_short: Style,
    /// Long flags (--flag)
    pub flag_long: Style,
    /// Invalid flags
    pub invalid_flag: Style,
    /// Single-quoted strings
    pub string_single: Style,
    /// Double-quoted strings
    pub string_double: Style,
    /// Numbers
    pub number: Style,
    /// Existing paths
    pub path_exists: Style,
    /// Non-existing paths
    pub path_missing: Style,
    /// Operators (| && > etc)
    pub operator: Style,
    /// Variables ($VAR)
    pub variable: Style,
    /// Comments (# ...)
    pub comment: Style,
    /// Default text
    pub default: Style,
}

impl Default for SyntaxTheme {
    fn default() -> Self {
        Self {
            command: Style::new().bold().fg(Color::Yellow),
            unknown_command: Style::new().fg(Color::Red).underline(),
            subcommand: Style::new().fg(Color::Green),
            flag_short: Style::new().fg(Color::Cyan),
            flag_long: Style::new().fg(Color::Cyan),
            invalid_flag: Style::new().fg(Color::Red).underline(),
            string_single: Style::new().fg(Color::Yellow),
            string_double: Style::new().fg(Color::Yellow),
            number: Style::new().fg(Color::Magenta),
            path_exists: Style::new().fg(Color::White),
            path_missing: Style::new().fg(Color::Red).dimmed(),
            operator: Style::new().bold().fg(Color::White),
            variable: Style::new().fg(Color::Green),
            comment: Style::new().fg(Color::DarkGray).dimmed(),
            default: Style::new(),
        }
    }
}

impl SyntaxTheme {
    /// Nord color scheme
    pub fn nord() -> Self {
        Self {
            command: Style::new().bold().fg(Color::Rgb(136, 192, 208)), // frost
            unknown_command: Style::new().fg(Color::Rgb(191, 97, 106)).underline(), // red
            subcommand: Style::new().fg(Color::Rgb(163, 190, 140)), // green
            flag_short: Style::new().fg(Color::Rgb(129, 161, 193)), // frost lighter
            flag_long: Style::new().fg(Color::Rgb(129, 161, 193)),
            invalid_flag: Style::new().fg(Color::Rgb(191, 97, 106)).underline(),
            string_single: Style::new().fg(Color::Rgb(163, 190, 140)), // green
            string_double: Style::new().fg(Color::Rgb(163, 190, 140)),
            number: Style::new().fg(Color::Rgb(180, 142, 173)), // purple
            path_exists: Style::new().fg(Color::Rgb(236, 239, 244)), // snow storm
            path_missing: Style::new().fg(Color::Rgb(191, 97, 106)).dimmed(),
            operator: Style::new().bold().fg(Color::Rgb(216, 222, 233)),
            variable: Style::new().fg(Color::Rgb(235, 203, 139)), // yellow
            comment: Style::new().fg(Color::Rgb(76, 86, 106)).dimmed(),
            default: Style::new().fg(Color::Rgb(216, 222, 233)),
        }
    }

    /// Dracula color scheme
    pub fn dracula() -> Self {
        Self {
            command: Style::new().bold().fg(Color::Rgb(189, 147, 249)), // purple
            unknown_command: Style::new().fg(Color::Rgb(255, 85, 85)).underline(), // red
            subcommand: Style::new().fg(Color::Rgb(80, 250, 123)), // green
            flag_short: Style::new().fg(Color::Rgb(139, 233, 253)), // cyan
            flag_long: Style::new().fg(Color::Rgb(139, 233, 253)),
            invalid_flag: Style::new().fg(Color::Rgb(255, 85, 85)).underline(),
            string_single: Style::new().fg(Color::Rgb(241, 250, 140)), // yellow
            string_double: Style::new().fg(Color::Rgb(241, 250, 140)),
            number: Style::new().fg(Color::Rgb(255, 184, 108)), // orange
            path_exists: Style::new().fg(Color::Rgb(248, 248, 242)), // foreground
            path_missing: Style::new().fg(Color::Rgb(255, 85, 85)).dimmed(),
            operator: Style::new().bold().fg(Color::Rgb(255, 121, 198)), // pink
            variable: Style::new().fg(Color::Rgb(80, 250, 123)), // green
            comment: Style::new().fg(Color::Rgb(98, 114, 164)).dimmed(), // comment
            default: Style::new().fg(Color::Rgb(248, 248, 242)),
        }
    }
}

/// Token types for syntax highlighting
#[derive(Debug, Clone, PartialEq)]
enum TokenType {
    Command,
    Subcommand,
    Argument,
    ShortFlag,
    LongFlag,
    SingleQuotedString,
    DoubleQuotedString,
    Number,
    Path,
    Operator,
    Variable,
    Comment,
    Whitespace,
    Unknown,
}

/// A token in the command line
#[derive(Debug, Clone)]
struct Token {
    text: String,
    token_type: TokenType,
    start: usize,
    end: usize,
}

/// Smart syntax highlighter
pub struct SmartHighlighter {
    /// Known command names
    commands: HashSet<String>,
    /// Syntax theme
    theme: SyntaxTheme,
}

impl SmartHighlighter {
    pub fn new(commands: Vec<String>) -> Self {
        Self {
            commands: commands.into_iter().collect(),
            theme: SyntaxTheme::default(),
        }
    }

    pub fn with_theme(mut self, theme: SyntaxTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Tokenize a command line
    fn tokenize(&self, line: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut chars = line.char_indices().peekable();
        let mut is_first_token = true;
        let mut after_command = false;

        while let Some((start, c)) = chars.next() {
            // Handle comments
            if c == '#' {
                let text: String = std::iter::once(c)
                    .chain(chars.by_ref().map(|(_, c)| c))
                    .collect();
                tokens.push(Token {
                    text: text.clone(),
                    token_type: TokenType::Comment,
                    start,
                    end: start + text.len(),
                });
                continue;
            }

            // Handle whitespace
            if c.is_whitespace() {
                let mut text = String::from(c);
                while let Some(&(_, next_c)) = chars.peek() {
                    if next_c.is_whitespace() {
                        text.push(chars.next().unwrap().1);
                    } else {
                        break;
                    }
                }
                tokens.push(Token {
                    text: text.clone(),
                    token_type: TokenType::Whitespace,
                    start,
                    end: start + text.len(),
                });
                continue;
            }

            // Handle operators
            if let Some((text, token_type)) = self.try_parse_operator(c, &mut chars) {
                tokens.push(Token {
                    text: text.clone(),
                    token_type,
                    start,
                    end: start + text.len(),
                });
                is_first_token = true; // Reset after operator
                after_command = false;
                continue;
            }

            // Handle single-quoted strings
            if c == '\'' {
                let mut text = String::from(c);
                let mut closed = false;
                while let Some((_, next_c)) = chars.next() {
                    text.push(next_c);
                    if next_c == '\'' {
                        closed = true;
                        break;
                    }
                }
                tokens.push(Token {
                    text: text.clone(),
                    token_type: if closed {
                        TokenType::SingleQuotedString
                    } else {
                        TokenType::Unknown
                    },
                    start,
                    end: start + text.len(),
                });
                continue;
            }

            // Handle double-quoted strings
            if c == '"' {
                let mut text = String::from(c);
                let mut closed = false;
                while let Some((_, next_c)) = chars.next() {
                    text.push(next_c);
                    if next_c == '"' {
                        closed = true;
                        break;
                    }
                }
                tokens.push(Token {
                    text: text.clone(),
                    token_type: if closed {
                        TokenType::DoubleQuotedString
                    } else {
                        TokenType::Unknown
                    },
                    start,
                    end: start + text.len(),
                });
                continue;
            }

            // Handle variables
            if c == '$' {
                let mut text = String::from(c);
                // Check for ${...} syntax
                if chars.peek().map(|(_, c)| *c) == Some('{') {
                    text.push(chars.next().unwrap().1);
                    while let Some((_, next_c)) = chars.next() {
                        text.push(next_c);
                        if next_c == '}' {
                            break;
                        }
                    }
                } else {
                    // Regular $VAR
                    while let Some(&(_, next_c)) = chars.peek() {
                        if next_c.is_alphanumeric() || next_c == '_' {
                            text.push(chars.next().unwrap().1);
                        } else {
                            break;
                        }
                    }
                }
                tokens.push(Token {
                    text: text.clone(),
                    token_type: TokenType::Variable,
                    start,
                    end: start + text.len(),
                });
                continue;
            }

            // Handle regular tokens (commands, arguments, flags)
            let mut text = String::from(c);
            while let Some(&(_, next_c)) = chars.peek() {
                if next_c.is_whitespace()
                    || next_c == '|'
                    || next_c == '&'
                    || next_c == ';'
                    || next_c == '>'
                    || next_c == '<'
                    || next_c == '\''
                    || next_c == '"'
                    || next_c == '$'
                {
                    break;
                }
                text.push(chars.next().unwrap().1);
            }

            // Determine token type
            let token_type = if is_first_token {
                is_first_token = false;
                after_command = true;
                TokenType::Command
            } else if text.starts_with("--") {
                TokenType::LongFlag
            } else if text.starts_with('-') && text.len() > 1 {
                TokenType::ShortFlag
            } else if text.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
                TokenType::Number
            } else if text.contains('/') || text.starts_with('.') || text.starts_with('~') {
                TokenType::Path
            } else if after_command && !is_first_token {
                // Could be a subcommand or argument
                TokenType::Argument
            } else {
                TokenType::Unknown
            };

            tokens.push(Token {
                text: text.clone(),
                token_type,
                start,
                end: start + text.len(),
            });
        }

        tokens
    }

    /// Try to parse an operator
    fn try_parse_operator(
        &self,
        c: char,
        chars: &mut std::iter::Peekable<std::str::CharIndices>,
    ) -> Option<(String, TokenType)> {
        match c {
            '|' => {
                if chars.peek().map(|(_, c)| *c) == Some('|') {
                    chars.next();
                    Some(("||".to_string(), TokenType::Operator))
                } else {
                    Some(("|".to_string(), TokenType::Operator))
                }
            }
            '&' => {
                if chars.peek().map(|(_, c)| *c) == Some('&') {
                    chars.next();
                    Some(("&&".to_string(), TokenType::Operator))
                } else {
                    Some(("&".to_string(), TokenType::Operator))
                }
            }
            ';' => Some((";".to_string(), TokenType::Operator)),
            '>' => {
                if chars.peek().map(|(_, c)| *c) == Some('>') {
                    chars.next();
                    Some((">>".to_string(), TokenType::Operator))
                } else {
                    Some((">".to_string(), TokenType::Operator))
                }
            }
            '<' => Some(("<".to_string(), TokenType::Operator)),
            _ => None,
        }
    }

    /// Get style for a token
    fn style_for_token(&self, token: &Token) -> Style {
        match &token.token_type {
            TokenType::Command => {
                if self.commands.contains(&token.text) || self.is_system_command(&token.text) {
                    self.theme.command
                } else {
                    self.theme.unknown_command
                }
            }
            TokenType::Subcommand => self.theme.subcommand,
            TokenType::ShortFlag => self.theme.flag_short,
            TokenType::LongFlag => self.theme.flag_long,
            TokenType::SingleQuotedString => self.theme.string_single,
            TokenType::DoubleQuotedString => self.theme.string_double,
            TokenType::Number => self.theme.number,
            TokenType::Path => {
                if Path::new(&token.text).exists() {
                    self.theme.path_exists
                } else {
                    self.theme.path_missing
                }
            }
            TokenType::Operator => self.theme.operator,
            TokenType::Variable => self.theme.variable,
            TokenType::Comment => self.theme.comment,
            TokenType::Whitespace => self.theme.default,
            TokenType::Argument | TokenType::Unknown => self.theme.default,
        }
    }

    /// Check if a command is a system command
    fn is_system_command(&self, cmd: &str) -> bool {
        // Common system commands that might not be in our definitions
        let system_commands = [
            "ls", "cd", "pwd", "echo", "cat", "grep", "find", "mkdir", "rm", "cp", "mv", "touch",
            "chmod", "chown", "sudo", "apt", "yum", "brew", "which", "whereis", "man", "less",
            "more", "head", "tail", "wc", "sort", "uniq", "cut", "awk", "sed", "tr", "xargs",
            "curl", "wget", "ssh", "scp", "rsync", "tar", "zip", "unzip", "gzip", "gunzip",
            "python", "python3", "node", "npm", "npx", "yarn", "pnpm", "cargo", "rustc", "go",
            "java", "javac", "ruby", "perl", "php", "make", "cmake", "gcc", "g++", "clang",
            "git", "docker", "kubectl", "terraform", "ansible", "vim", "nvim", "nano", "code",
            "exit", "config", "clear", "history", "alias", "export", "source", "env", "set",
        ];

        system_commands.contains(&cmd)
    }
}

impl Highlighter for SmartHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let tokens = self.tokenize(line);
        let mut styled = StyledText::new();

        for token in tokens {
            let style = self.style_for_token(&token);
            styled.push((style, token.text));
        }

        styled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple() {
        let highlighter = SmartHighlighter::new(vec!["git".to_string()]);
        let tokens = highlighter.tokenize("git commit -m 'message'");

        assert!(!tokens.is_empty());
        assert_eq!(tokens[0].token_type, TokenType::Command);
        assert_eq!(tokens[0].text, "git");
    }

    #[test]
    fn test_tokenize_pipeline() {
        let highlighter = SmartHighlighter::new(vec!["ls".to_string(), "grep".to_string()]);
        let tokens = highlighter.tokenize("ls -la | grep foo");

        let operators: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type == TokenType::Operator)
            .collect();
        assert_eq!(operators.len(), 1);
        assert_eq!(operators[0].text, "|");
    }

    #[test]
    fn test_tokenize_variables() {
        let highlighter = SmartHighlighter::new(vec![]);
        let tokens = highlighter.tokenize("echo $HOME ${PATH}");

        let vars: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type == TokenType::Variable)
            .collect();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].text, "$HOME");
        assert_eq!(vars[1].text, "${PATH}");
    }

    #[test]
    fn test_tokenize_strings() {
        let highlighter = SmartHighlighter::new(vec![]);
        let tokens = highlighter.tokenize("echo 'hello' \"world\"");

        let strings: Vec<_> = tokens
            .iter()
            .filter(|t| {
                t.token_type == TokenType::SingleQuotedString
                    || t.token_type == TokenType::DoubleQuotedString
            })
            .collect();
        assert_eq!(strings.len(), 2);
    }

    #[test]
    fn test_tokenize_flags() {
        let highlighter = SmartHighlighter::new(vec!["git".to_string()]);
        let tokens = highlighter.tokenize("git commit -m --amend");

        let short_flags: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type == TokenType::ShortFlag)
            .collect();
        let long_flags: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type == TokenType::LongFlag)
            .collect();

        assert_eq!(short_flags.len(), 1);
        assert_eq!(long_flags.len(), 1);
        assert_eq!(short_flags[0].text, "-m");
        assert_eq!(long_flags[0].text, "--amend");
    }

    #[test]
    fn test_tokenize_comment() {
        let highlighter = SmartHighlighter::new(vec![]);
        let tokens = highlighter.tokenize("echo hello # this is a comment");

        let comments: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type == TokenType::Comment)
            .collect();
        assert_eq!(comments.len(), 1);
        assert!(comments[0].text.starts_with('#'));
    }

    #[test]
    fn test_theme_default() {
        let theme = SyntaxTheme::default();
        assert_eq!(theme.command.foreground, Some(Color::Yellow));
    }

    #[test]
    fn test_is_system_command() {
        let highlighter = SmartHighlighter::new(vec![]);
        assert!(highlighter.is_system_command("ls"));
        assert!(highlighter.is_system_command("git"));
        assert!(highlighter.is_system_command("docker"));
        assert!(!highlighter.is_system_command("nonexistent_cmd_xyz"));
    }
}
