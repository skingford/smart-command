//! Rich UI Enhancements for Smart Command
//!
//! Provides enhanced user interface components including:
//! - Preview pane for completion documentation
//! - Grouped completions by category
//! - Inline hints and suggestions
//! - Progress indicators and spinners

#![allow(dead_code)]

use nu_ansi_term::{Color, Style};
use reedline::{Hinter, History};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Completion group categories for organized display
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompletionGroup {
    /// Subcommands of current command
    Subcommand,
    /// Command flags/options
    Flag,
    /// File paths
    Path,
    /// Git-related completions (branches, remotes, etc.)
    Git,
    /// Docker-related completions
    Docker,
    /// Environment variables
    Environment,
    /// Process names/PIDs
    Process,
    /// SSH hosts
    Ssh,
    /// Package names (npm, cargo, etc.)
    Package,
    /// History suggestions
    History,
    /// AI-suggested commands
    AiSuggestion,
    /// Custom provider
    Custom(String),
}

impl CompletionGroup {
    /// Get the display name for this group
    pub fn display_name(&self) -> &str {
        match self {
            Self::Subcommand => "Subcommands",
            Self::Flag => "Options",
            Self::Path => "Files",
            Self::Git => "Git",
            Self::Docker => "Docker",
            Self::Environment => "Environment",
            Self::Process => "Processes",
            Self::Ssh => "SSH Hosts",
            Self::Package => "Packages",
            Self::History => "History",
            Self::AiSuggestion => "Suggestions",
            Self::Custom(name) => name.as_str(),
        }
    }

    /// Get the icon for this group
    pub fn icon(&self) -> &str {
        match self {
            Self::Subcommand => "",
            Self::Flag => "󰘵",
            Self::Path => "",
            Self::Git => "",
            Self::Docker => "",
            Self::Environment => "",
            Self::Process => "",
            Self::Ssh => "󰣀",
            Self::Package => "",
            Self::History => "",
            Self::AiSuggestion => "",
            Self::Custom(_) => "󰘳",
        }
    }

    /// Get the style for this group's header
    pub fn header_style(&self) -> Style {
        match self {
            Self::Subcommand => Style::new().fg(Color::Cyan).bold(),
            Self::Flag => Style::new().fg(Color::Yellow).bold(),
            Self::Path => Style::new().fg(Color::Blue).bold(),
            Self::Git => Style::new().fg(Color::Magenta).bold(),
            Self::Docker => Style::new().fg(Color::LightBlue).bold(),
            Self::Environment => Style::new().fg(Color::Green).bold(),
            Self::Process => Style::new().fg(Color::Red).bold(),
            Self::Ssh => Style::new().fg(Color::LightMagenta).bold(),
            Self::Package => Style::new().fg(Color::LightGreen).bold(),
            Self::History => Style::new().fg(Color::DarkGray).bold(),
            Self::AiSuggestion => Style::new().fg(Color::Purple).bold(),
            Self::Custom(_) => Style::new().fg(Color::White).bold(),
        }
    }
}

/// A grouped completion item with metadata
#[derive(Debug, Clone)]
pub struct GroupedCompletion {
    /// The completion text
    pub value: String,
    /// Display text (may differ from value)
    pub display: String,
    /// Description/documentation
    pub description: Option<String>,
    /// The group this completion belongs to
    pub group: CompletionGroup,
    /// Priority within group (higher = more prominent)
    pub priority: i32,
    /// Optional preview content
    pub preview: Option<String>,
}

impl GroupedCompletion {
    pub fn new(value: impl Into<String>, group: CompletionGroup) -> Self {
        let value = value.into();
        Self {
            display: value.clone(),
            value,
            description: None,
            group,
            priority: 0,
            preview: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.display = display.into();
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_preview(mut self, preview: impl Into<String>) -> Self {
        self.preview = Some(preview.into());
        self
    }
}

/// Completion groups manager for organizing completions
#[derive(Debug, Default)]
pub struct CompletionGroups {
    groups: HashMap<CompletionGroup, Vec<GroupedCompletion>>,
    order: Vec<CompletionGroup>,
}

impl CompletionGroups {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
            order: vec![
                CompletionGroup::Subcommand,
                CompletionGroup::Flag,
                CompletionGroup::Git,
                CompletionGroup::Docker,
                CompletionGroup::Ssh,
                CompletionGroup::Environment,
                CompletionGroup::Process,
                CompletionGroup::Package,
                CompletionGroup::Path,
                CompletionGroup::History,
                CompletionGroup::AiSuggestion,
            ],
        }
    }

    /// Add a completion to the appropriate group
    pub fn add(&mut self, completion: GroupedCompletion) {
        let group = completion.group.clone();
        self.groups.entry(group).or_default().push(completion);
    }

    /// Add multiple completions
    pub fn add_all(&mut self, completions: impl IntoIterator<Item = GroupedCompletion>) {
        for c in completions {
            self.add(c);
        }
    }

    /// Sort completions within each group by priority
    pub fn sort(&mut self) {
        for items in self.groups.values_mut() {
            items.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.display.cmp(&b.display)));
        }
    }

    /// Get all groups in display order
    pub fn iter_groups(&self) -> impl Iterator<Item = (&CompletionGroup, &Vec<GroupedCompletion>)> {
        self.order
            .iter()
            .filter_map(|g| self.groups.get(g).map(|items| (g, items)))
            .chain(
                self.groups
                    .iter()
                    .filter(|(g, _)| !self.order.contains(g)),
            )
    }

    /// Get total completion count
    pub fn total_count(&self) -> usize {
        self.groups.values().map(|v| v.len()).sum()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.groups.values().all(|v| v.is_empty())
    }

    /// Format as a styled string for display
    pub fn format_grouped(&self, max_per_group: usize) -> String {
        let mut output = String::new();

        for (group, items) in self.iter_groups() {
            if items.is_empty() {
                continue;
            }

            // Group header
            let header_style = group.header_style();
            output.push_str(&format!(
                "\n{} {} {}\n",
                header_style.paint(group.icon()),
                header_style.paint(group.display_name()),
                Style::new()
                    .fg(Color::DarkGray)
                    .paint(format!("({})", items.len()))
            ));

            // Items
            for (i, item) in items.iter().enumerate() {
                if i >= max_per_group {
                    output.push_str(&format!(
                        "  {} ... and {} more\n",
                        Style::new().fg(Color::DarkGray).paint(""),
                        items.len() - max_per_group
                    ));
                    break;
                }

                let value_style = Style::new().fg(Color::White);
                let desc_style = Style::new().fg(Color::DarkGray);

                if let Some(desc) = &item.description {
                    output.push_str(&format!(
                        "  {} {} {}\n",
                        value_style.paint(&item.display),
                        desc_style.paint("─"),
                        desc_style.paint(desc)
                    ));
                } else {
                    output.push_str(&format!("  {}\n", value_style.paint(&item.display)));
                }
            }
        }

        output
    }
}

/// Preview pane for showing documentation
#[derive(Debug)]
pub struct PreviewPane {
    /// Maximum width of the preview
    pub max_width: usize,
    /// Maximum height (lines)
    pub max_height: usize,
    /// Current preview content
    content: Option<PreviewContent>,
}

#[derive(Debug, Clone)]
pub struct PreviewContent {
    /// Title of the preview
    pub title: String,
    /// Main content (may be multiline)
    pub body: String,
    /// Optional syntax highlighting hint
    pub syntax: Option<String>,
    /// Optional examples
    pub examples: Vec<String>,
    /// Optional "see also" references
    pub see_also: Vec<String>,
}

impl PreviewPane {
    pub fn new() -> Self {
        Self {
            max_width: 60,
            max_height: 15,
            content: None,
        }
    }

    pub fn with_dimensions(mut self, width: usize, height: usize) -> Self {
        self.max_width = width;
        self.max_height = height;
        self
    }

    /// Set preview content
    pub fn set_content(&mut self, content: PreviewContent) {
        self.content = Some(content);
    }

    /// Clear preview content
    pub fn clear(&mut self) {
        self.content = None;
    }

    /// Generate preview for a command/flag
    pub fn preview_for_command(name: &str, description: &str, flags: &[(String, String)]) -> PreviewContent {
        let mut body = description.to_string();

        if !flags.is_empty() {
            body.push_str("\n\nOptions:");
            for (flag, desc) in flags.iter().take(5) {
                body.push_str(&format!("\n  {} - {}", flag, desc));
            }
            if flags.len() > 5 {
                body.push_str(&format!("\n  ... and {} more", flags.len() - 5));
            }
        }

        PreviewContent {
            title: name.to_string(),
            body,
            syntax: None,
            examples: vec![],
            see_also: vec![],
        }
    }

    /// Generate preview for a flag
    pub fn preview_for_flag(flag: &str, description: &str, value_hint: Option<&str>) -> PreviewContent {
        let mut body = description.to_string();

        if let Some(hint) = value_hint {
            body.push_str(&format!("\n\nExpects: {}", hint));
        }

        PreviewContent {
            title: flag.to_string(),
            body,
            syntax: None,
            examples: vec![],
            see_also: vec![],
        }
    }

    /// Render the preview pane as a string
    pub fn render(&self) -> Option<String> {
        let content = self.content.as_ref()?;

        let mut output = String::new();
        let title_style = Style::new().fg(Color::Cyan).bold();
        let body_style = Style::new().fg(Color::White);
        let example_style = Style::new().fg(Color::Green);
        let border_style = Style::new().fg(Color::DarkGray);

        // Top border
        let width = self.max_width.min(content.title.len() + 4).max(40);
        output.push_str(&format!(
            "{}\n",
            border_style.paint("─".repeat(width))
        ));

        // Title
        output.push_str(&format!("{}\n", title_style.paint(&content.title)));

        // Body (wrap to max_width)
        for line in content.body.lines().take(self.max_height - 4) {
            if line.len() > self.max_width {
                output.push_str(&format!(
                    "{}\n",
                    body_style.paint(&line[..self.max_width - 3])
                ));
                output.push_str(&format!(
                    "   {}\n",
                    body_style.paint(line[self.max_width - 3..].trim())
                ));
            } else {
                output.push_str(&format!("{}\n", body_style.paint(line)));
            }
        }

        // Examples
        if !content.examples.is_empty() {
            output.push_str(&format!("\n{}\n", title_style.paint("Examples:")));
            for example in content.examples.iter().take(3) {
                output.push_str(&format!("  {}\n", example_style.paint(format!("$ {}", example))));
            }
        }

        // Bottom border
        output.push_str(&format!(
            "{}",
            border_style.paint("─".repeat(width))
        ));

        Some(output)
    }
}

impl Default for PreviewPane {
    fn default() -> Self {
        Self::new()
    }
}

/// Inline hint provider that shows suggestions as the user types
pub struct InlineHinter {
    /// Available commands for hinting
    commands: Arc<RwLock<Vec<String>>>,
    /// History-based hints
    history_hints: Arc<RwLock<Vec<String>>>,
    /// Current hint style
    hint_style: Style,
    /// Minimum prefix length to show hints
    min_prefix: usize,
}

impl InlineHinter {
    pub fn new(commands: Vec<String>) -> Self {
        Self {
            commands: Arc::new(RwLock::new(commands)),
            history_hints: Arc::new(RwLock::new(Vec::new())),
            hint_style: Style::new().fg(Color::DarkGray).italic(),
            min_prefix: 2,
        }
    }

    /// Update available commands
    pub fn set_commands(&self, commands: Vec<String>) {
        if let Ok(mut cmds) = self.commands.write() {
            *cmds = commands;
        }
    }

    /// Add history-based hints
    pub fn add_history_hint(&self, hint: String) {
        if let Ok(mut hints) = self.history_hints.write() {
            hints.retain(|h| h != &hint);
            hints.insert(0, hint);
            hints.truncate(100); // Keep last 100 history hints
        }
    }

    /// Find the best hint for current input
    fn find_hint(&self, line: &str) -> Option<String> {
        if line.len() < self.min_prefix {
            return None;
        }

        let line_lower = line.to_lowercase();

        // First check history (most relevant)
        if let Ok(hints) = self.history_hints.read() {
            for hint in hints.iter() {
                if hint.to_lowercase().starts_with(&line_lower) && hint.len() > line.len() {
                    return Some(hint[line.len()..].to_string());
                }
            }
        }

        // Then check commands
        if let Ok(commands) = self.commands.read() {
            for cmd in commands.iter() {
                if cmd.to_lowercase().starts_with(&line_lower) && cmd.len() > line.len() {
                    return Some(cmd[line.len()..].to_string());
                }
            }
        }

        None
    }
}

impl Hinter for InlineHinter {
    fn handle(
        &mut self,
        line: &str,
        _pos: usize,
        _history: &dyn History,
        _use_ansi_coloring: bool,
        _cwd: &str,
    ) -> String {
        self.find_hint(line)
            .map(|h| self.hint_style.paint(h).to_string())
            .unwrap_or_default()
    }

    fn complete_hint(&self) -> String {
        String::new()
    }

    fn next_hint_token(&self) -> String {
        String::new()
    }
}

/// Progress indicator for async operations
#[derive(Debug, Clone)]
pub struct ProgressIndicator {
    /// Current message
    message: String,
    /// Spinner frames
    frames: Vec<&'static str>,
    /// Current frame index
    current_frame: usize,
    /// Style for the spinner
    style: Style,
}

impl ProgressIndicator {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            current_frame: 0,
            style: Style::new().fg(Color::Cyan),
        }
    }

    /// Advance to next frame
    pub fn tick(&mut self) {
        self.current_frame = (self.current_frame + 1) % self.frames.len();
    }

    /// Render the current frame
    pub fn render(&self) -> String {
        format!(
            "{} {}",
            self.style.paint(self.frames[self.current_frame]),
            self.message
        )
    }

    /// Update the message
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Render success state
    pub fn success(&self, message: impl Into<String>) -> String {
        format!(
            "{} {}",
            Style::new().fg(Color::Green).paint("✓"),
            message.into()
        )
    }

    /// Render error state
    pub fn error(&self, message: impl Into<String>) -> String {
        format!(
            "{} {}",
            Style::new().fg(Color::Red).paint("✗"),
            message.into()
        )
    }
}

impl Default for ProgressIndicator {
    fn default() -> Self {
        Self::new("Loading...")
    }
}

/// Keyboard shortcut display helper
#[derive(Debug, Clone)]
pub struct KeyHint {
    pub key: String,
    pub action: String,
}

impl KeyHint {
    pub fn new(key: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            action: action.into(),
        }
    }

    pub fn render(&self) -> String {
        let key_style = Style::new().fg(Color::Yellow).bold();
        let action_style = Style::new().fg(Color::DarkGray);
        format!("{} {}", key_style.paint(&self.key), action_style.paint(&self.action))
    }
}

/// Help bar showing available keyboard shortcuts
pub struct HelpBar {
    hints: Vec<KeyHint>,
}

impl HelpBar {
    pub fn new() -> Self {
        Self {
            hints: vec![
                KeyHint::new("Tab", "Complete"),
                KeyHint::new("/", "Search"),
                KeyHint::new("↑↓", "History"),
                KeyHint::new("Ctrl+D", "Exit"),
            ],
        }
    }

    pub fn with_hints(hints: Vec<KeyHint>) -> Self {
        Self { hints }
    }

    pub fn render(&self) -> String {
        let separator = Style::new().fg(Color::DarkGray).paint(" │ ");
        self.hints
            .iter()
            .map(|h| h.render())
            .collect::<Vec<_>>()
            .join(&separator.to_string())
    }
}

impl Default for HelpBar {
    fn default() -> Self {
        Self::new()
    }
}

/// Status line component for showing context information
pub struct StatusLine {
    /// Left-aligned items
    left: Vec<String>,
    /// Right-aligned items
    right: Vec<String>,
    /// Maximum width
    max_width: usize,
}

impl StatusLine {
    pub fn new(width: usize) -> Self {
        Self {
            left: Vec::new(),
            right: Vec::new(),
            max_width: width,
        }
    }

    pub fn add_left(&mut self, item: impl Into<String>) -> &mut Self {
        self.left.push(item.into());
        self
    }

    pub fn add_right(&mut self, item: impl Into<String>) -> &mut Self {
        self.right.push(item.into());
        self
    }

    pub fn render(&self) -> String {
        let separator = Style::new().fg(Color::DarkGray).paint(" | ");

        let left_str = self.left.join(&separator.to_string());
        let right_str = self.right.join(&separator.to_string());

        // Calculate visible lengths (without ANSI codes)
        let left_len = strip_ansi(&left_str).len();
        let right_len = strip_ansi(&right_str).len();

        let padding = if left_len + right_len < self.max_width {
            self.max_width - left_len - right_len
        } else {
            1
        };

        format!("{}{}{}", left_str, " ".repeat(padding), right_str)
    }
}

/// Strip ANSI escape codes from a string
fn strip_ansi(s: &str) -> Cow<'_, str> {
    // Simple pattern to strip ANSI codes
    let mut result = String::new();
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }

    Cow::Owned(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_group_display() {
        assert_eq!(CompletionGroup::Subcommand.display_name(), "Subcommands");
        assert_eq!(CompletionGroup::Flag.display_name(), "Options");
        assert_eq!(CompletionGroup::Git.display_name(), "Git");
    }

    #[test]
    fn test_grouped_completion_builder() {
        let completion = GroupedCompletion::new("test", CompletionGroup::Subcommand)
            .with_description("A test completion")
            .with_priority(10);

        assert_eq!(completion.value, "test");
        assert_eq!(completion.description, Some("A test completion".to_string()));
        assert_eq!(completion.priority, 10);
    }

    #[test]
    fn test_completion_groups() {
        let mut groups = CompletionGroups::new();

        groups.add(GroupedCompletion::new("commit", CompletionGroup::Subcommand));
        groups.add(GroupedCompletion::new("push", CompletionGroup::Subcommand));
        groups.add(GroupedCompletion::new("--verbose", CompletionGroup::Flag));

        assert_eq!(groups.total_count(), 3);
        assert!(!groups.is_empty());
    }

    #[test]
    fn test_preview_pane() {
        let mut pane = PreviewPane::new();

        let content = PreviewPane::preview_for_command(
            "git commit",
            "Record changes to the repository",
            &[
                ("-m".to_string(), "Commit message".to_string()),
                ("-a".to_string(), "Stage all modified files".to_string()),
            ],
        );

        pane.set_content(content);
        let rendered = pane.render();

        assert!(rendered.is_some());
        let text = rendered.unwrap();
        assert!(text.contains("git commit"));
        assert!(text.contains("Record changes"));
    }

    #[test]
    fn test_inline_hinter() {
        let hinter = InlineHinter::new(vec!["git".to_string(), "grep".to_string()]);

        let hint = hinter.find_hint("gi");
        assert_eq!(hint, Some("t".to_string()));

        let hint = hinter.find_hint("gr");
        assert_eq!(hint, Some("ep".to_string()));

        let hint = hinter.find_hint("x");
        assert!(hint.is_none());
    }

    #[test]
    fn test_progress_indicator() {
        let mut progress = ProgressIndicator::new("Loading");

        let frame1 = progress.render();
        progress.tick();
        let frame2 = progress.render();

        // Frames should be different after tick
        assert_ne!(frame1, frame2);
    }

    #[test]
    fn test_key_hint() {
        let hint = KeyHint::new("Tab", "Complete");
        let rendered = hint.render();

        // Should contain both key and action
        assert!(rendered.contains("Tab") || strip_ansi(&rendered).contains("Tab"));
    }

    #[test]
    fn test_help_bar() {
        let bar = HelpBar::new();
        let rendered = bar.render();

        // Should contain default hints
        assert!(strip_ansi(&rendered).contains("Tab"));
        assert!(strip_ansi(&rendered).contains("Exit"));
    }

    #[test]
    fn test_status_line() {
        let mut status = StatusLine::new(80);
        status.add_left("main");
        status.add_right("git");

        let rendered = status.render();
        let stripped = strip_ansi(&rendered);

        assert!(stripped.contains("main"));
        assert!(stripped.contains("git"));
    }

    #[test]
    fn test_strip_ansi() {
        let with_ansi = "\x1b[31mred\x1b[0m text";
        let stripped = strip_ansi(with_ansi);
        assert_eq!(stripped.as_ref(), "red text");
    }
}
