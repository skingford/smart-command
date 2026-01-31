//! Colored output utilities following CLI development standards
//!
//! Output format standards:
//! - Success: green with checkmark
//! - Warning: yellow with warning sign
//! - Error: red with X mark
//! - Info: blue with info sign

use nu_ansi_term::{Color, Style};
use std::env;

/// Output style definitions following skill standards
pub struct Output;

impl Output {
    /// Success output: green with checkmark
    pub fn success(msg: &str) {
        let style = Style::new().fg(Color::Green);
        println!("{} {}", style.paint("✓"), style.paint(msg));
    }

    /// Warning output: yellow with warning sign
    pub fn warn(msg: &str) {
        let style = Style::new().fg(Color::Yellow);
        println!("{} {}", style.paint("⚠"), style.paint(msg));
    }

    /// Error output: red with X mark
    pub fn error(msg: &str) {
        let style = Style::new().fg(Color::Red);
        eprintln!("{} {}", style.paint("✗"), style.paint(msg));
    }

    /// Info output: blue with info sign
    pub fn info(msg: &str) {
        let style = Style::new().fg(Color::Blue);
        println!("{} {}", style.paint("ℹ"), style.paint(msg));
    }

    /// Dim/muted output for secondary information
    pub fn dim(msg: &str) {
        let style = Style::new().fg(Color::DarkGray);
        println!("{}", style.paint(msg));
    }

    /// Display startup banner with ASCII art logo
    pub fn banner() {
        let logo_style = Style::new().fg(Color::Cyan).bold();
        let accent_style = Style::new().fg(Color::Green).bold();
        let version_style = Style::new().fg(Color::Green);
        let cwd_style = Style::new().fg(Color::Yellow);

        // ASCII art logo - Smart Command: Terminal + Intelligence theme
        // Combines prompt symbol, circuit pattern, and "SC" monogram
        let logo = r#"
   ╭───────────────────────────────────────╮
   │  ┏━━┓╺┳╸┏━┓┏━┓╻╻ ╻┏━┓╻  ┏━┓╺┳╸╻┏━┓  │
   │  ┗━━┓ ┃ ┣━┫┣┳┛┃┃┏┛┣━┫┃  ┃ ┃┃┃┣┻┓  │
   │  ┗━━┛ ╹ ╹ ╹╹┗╸╹┗╸ ╹ ╹┗━╸┗━┛╹╹╹┗━┛  │
   │         ❯ Smart Command Shell      │
   ╰───────────────────────────────────────╯
"#;

        println!("{}", logo_style.paint(logo));
        println!(
            "    {} {}",
            accent_style.paint("⚡"),
            version_style.paint(format!("v{} · AI-Powered Intelligent Shell", env!("CARGO_PKG_VERSION")))
        );

        // Show current working directory
        if let Ok(cwd) = env::current_dir() {
            let cwd_display = dirs::home_dir()
                .and_then(|home| cwd.strip_prefix(&home).ok())
                .map(|rel| format!("~/{}", rel.display()))
                .unwrap_or_else(|| cwd.display().to_string());
            println!("    {}", cwd_style.paint(format!("📁 {}", cwd_display)));
        }

        println!();
    }

    /// Styled prompt display
    pub fn prompt(cwd: &str, branch: Option<&str>) -> String {
        let cwd_style = Style::new().fg(Color::Cyan).bold();
        let branch_style = Style::new().fg(Color::Magenta);

        let branch_info = branch
            .map(|b| format!(" {}", branch_style.paint(format!("({})", b))))
            .unwrap_or_default();

        format!("{}{}", cwd_style.paint(cwd), branch_info)
    }

    /// Format a command suggestion
    pub fn command(cmd: &str) -> String {
        let style = Style::new().fg(Color::Yellow).bold();
        style.paint(cmd).to_string()
    }

    /// Format a file path
    #[allow(dead_code)]
    pub fn path(p: &str) -> String {
        let style = Style::new().fg(Color::Cyan).underline();
        style.paint(p).to_string()
    }

    /// Format search results with match highlighting
    pub fn search_result(index: usize, cmd: &str, match_type: &str, desc: &str) {
        let idx_style = Style::new().fg(Color::Yellow).bold();
        let match_style = Style::new().fg(Color::DarkGray);
        let cmd_style = Style::new().fg(Color::Green).bold();
        let desc_style = Style::new().fg(Color::White);

        println!(
            "{}. {} {}",
            idx_style.paint(index.to_string()),
            match_style.paint(format!("[{}]", match_type)),
            cmd_style.paint(cmd)
        );
        println!("   {}", desc_style.paint(desc));
    }

    /// Format exit code display
    pub fn exit_code(code: i32) {
        if code != 0 {
            let style = Style::new().fg(Color::Red);
            eprintln!("{}", style.paint(format!("Exit: {}", code)));
        }
    }

    /// Display step header for AI command execution
    pub fn step(step_num: usize, total_steps: usize, description: &str) {
        let step_style = Style::new().fg(Color::Cyan).bold();
        let desc_style = Style::new().fg(Color::White);

        if total_steps > 1 {
            println!(
                "\n{} {} {}",
                step_style.paint("⟹"),
                step_style.paint(format!("Step {}/{}:", step_num, total_steps)),
                desc_style.paint(description)
            );
        } else {
            println!(
                "\n{} {}",
                step_style.paint("⟹"),
                desc_style.paint(description)
            );
        }
    }

    /// Display command being executed
    pub fn executing(cmd: &str) {
        let prompt_style = Style::new().fg(Color::DarkGray);
        let cmd_style = Style::new().fg(Color::Yellow).bold();
        println!(
            "  {} {}",
            prompt_style.paint("$"),
            cmd_style.paint(cmd)
        );
        // Print separator line
        let separator = Style::new().fg(Color::DarkGray);
        println!("  {}", separator.paint("─".repeat(50)));
    }

    /// Display execution result
    pub fn exec_result(success: bool, exit_code: Option<i32>) {
        let separator = Style::new().fg(Color::DarkGray);
        println!("  {}", separator.paint("─".repeat(50)));

        if success {
            let style = Style::new().fg(Color::Green);
            println!("  {} {}", style.paint("✓"), style.paint("Done"));
        } else if let Some(code) = exit_code {
            let style = Style::new().fg(Color::Red);
            println!("  {} {} (exit: {})", style.paint("✗"), style.paint("Failed"), code);
        } else {
            let style = Style::new().fg(Color::Red);
            println!("  {} {}", style.paint("✗"), style.paint("Failed"));
        }
    }

    /// Display Active AI hint after command error
    pub fn active_ai_hint() {
        let hint_style = Style::new().fg(Color::Cyan);
        let key_style = Style::new().fg(Color::Yellow).bold();
        let dim_style = Style::new().fg(Color::DarkGray);

        println!(
            "  {} {} {} {} {} {} {}",
            hint_style.paint("[AI]"),
            dim_style.paint("Press"),
            key_style.paint("e"),
            dim_style.paint("explain /"),
            key_style.paint("f"),
            dim_style.paint("fix /"),
            key_style.paint("Enter"),
            // dim_style.paint("skip")
        );
    }

    /// Display Active AI explanation
    pub fn active_ai_explain(explanation: &str) {
        let header_style = Style::new().fg(Color::Cyan).bold();
        let text_style = Style::new().fg(Color::White);

        println!();
        println!("{} {}", header_style.paint("💡"), header_style.paint("Error Explanation:"));
        for line in explanation.lines() {
            println!("   {}", text_style.paint(line));
        }
        println!();
    }

    /// Display Active AI fix suggestion
    pub fn active_ai_fix(fix_command: &str) {
        let header_style = Style::new().fg(Color::Green).bold();
        let cmd_style = Style::new().fg(Color::Yellow).bold();

        println!();
        println!("{} {}", header_style.paint("🔧"), header_style.paint("Suggested Fix:"));
        println!("   $ {}", cmd_style.paint(fix_command));
        println!();
    }

    /// Display next command suggestion (ghost text style)
    pub fn next_command_hint(suggestion: &str, confidence: f64) {
        let hint_style = Style::new().fg(Color::DarkGray).italic();
        let confidence_style = if confidence >= 0.7 {
            Style::new().fg(Color::Green)
        } else {
            Style::new().fg(Color::DarkGray)
        };

        let confidence_indicator = if confidence >= 0.8 {
            "●●●"
        } else if confidence >= 0.6 {
            "●●○"
        } else {
            "●○○"
        };

        println!(
            "  {} {} {}",
            hint_style.paint("→"),
            hint_style.paint(suggestion),
            confidence_style.paint(confidence_indicator)
        );
    }

    /// Display session context summary
    pub fn session_summary(total: usize, failed: usize, duration: &str) {
        let style = Style::new().fg(Color::DarkGray);
        let success_rate = if total > 0 {
            ((total - failed) as f64 / total as f64 * 100.0) as u32
        } else {
            100
        };

        println!(
            "  {}",
            style.paint(format!(
                "Session: {} commands, {}% success, {}",
                total, success_rate, duration
            ))
        );
    }

    /// Display quick error hint (without AI)
    pub fn quick_error_hint(hint: &str) {
        let style = Style::new().fg(Color::Yellow);
        println!("  {} {}", style.paint("💡"), style.paint(hint));
    }

    /// Display upgrade available notification
    pub fn upgrade_available(current: &str, latest: &str) {
        let style = Style::new().fg(Color::Yellow).bold();
        let version_current = Style::new().fg(Color::DarkGray);
        let version_new = Style::new().fg(Color::Green).bold();
        let cmd_style = Style::new().fg(Color::Cyan);

        println!();
        println!(
            "  {} {} {} {} {}",
            style.paint("⬆"),
            style.paint("发现新版本:"),
            version_current.paint(current),
            style.paint("→"),
            version_new.paint(latest)
        );
        println!(
            "    运行 {} 进行升级",
            cmd_style.paint("sc upgrade")
        );
        println!();
    }
}

/// Dangerous command patterns that require confirmation
pub static DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf",
    "rm -fr",
    "rm --recursive --force",
    "dd if=",
    "mkfs",
    "> /dev/sd",
    "chmod -R 777",
    "chmod -R 000",
    ":(){:|:&};:", // Fork bomb
    "mv /* ",
    "mv / ",
    "rm /*",
    "rm /",
    "git push --force",
    "git push -f",
    "git reset --hard",
    "DROP TABLE",
    "DROP DATABASE",
    "TRUNCATE TABLE",
];

/// Display categorized help for a command
pub fn display_categorized_help(
    command_path: &str,
    subcommands: &[crate::command_def::CommandSpec],
    flags: &[crate::command_def::FlagSpec],
    lang: &str,
) {
    use crate::command_def::FlagCategory;
    use std::collections::BTreeMap;

    let header_style = Style::new().fg(Color::Cyan).bold();
    let category_style = Style::new().fg(Color::Yellow).bold();
    let flag_style = Style::new().fg(Color::Green);
    let desc_style = Style::new().fg(Color::White);
    let dim_style = Style::new().fg(Color::DarkGray);

    println!();
    println!(
        "{} {}",
        header_style.paint("?"),
        header_style.paint(format!("Help for '{}'", command_path))
    );
    println!();

    // Display subcommands if any
    if !subcommands.is_empty() {
        let sub_header = if lang == "zh" { "子命令" } else { "Subcommands" };
        println!("{}", category_style.paint(format!("  {} ──────────────────────", sub_header)));

        for sub in subcommands {
            println!(
                "    {}  {}",
                flag_style.paint(format!("{:<16}", sub.name)),
                desc_style.paint(sub.description.get(lang))
            );
        }
        println!();
    }

    // Group flags by category
    let mut grouped: BTreeMap<u8, (FlagCategory, Vec<&crate::command_def::FlagSpec>)> =
        BTreeMap::new();

    for flag in flags {
        let order = flag.category.sort_order();
        grouped
            .entry(order)
            .or_insert_with(|| (flag.category.clone(), Vec::new()))
            .1
            .push(flag);
    }

    // Display each category
    for (_order, (category, category_flags)) in grouped {
        let cat_name = category.display_name(lang);
        println!("{}", category_style.paint(format!("  {} ──────────────────────", cat_name)));

        for flag in category_flags {
            // Format the flag display
            let short = flag.short.map(|c| format!("-{}", c)).unwrap_or_default();
            let long = flag.long.as_ref().map(|l| format!("--{}", l)).unwrap_or_default();

            let flag_display = match (flag.short, flag.long.as_ref()) {
                (Some(_), Some(_)) => format!("{}, {}", short, long),
                (Some(_), None) => short,
                (None, Some(_)) => long,
                (None, None) => continue,
            };

            // Add value indicator if needed
            let flag_with_value = if flag.takes_value {
                format!("{} <value>", flag_display)
            } else {
                flag_display
            };

            println!(
                "    {}  {}",
                flag_style.paint(format!("{:<24}", flag_with_value)),
                desc_style.paint(flag.description.get(lang))
            );
        }
        println!();
    }

    // Show tip
    let tip = if lang == "zh" {
        "提示: 输入命令后按 Tab 查看补全建议"
    } else {
        "Tip: Press Tab after command for completion suggestions"
    };
    println!("  {}", dim_style.paint(tip));
    println!();
}

/// Check if a command is potentially dangerous
pub fn is_dangerous_command(cmd: &str) -> bool {
    let cmd_lower = cmd.to_lowercase();
    DANGEROUS_PATTERNS
        .iter()
        .any(|pattern| cmd_lower.contains(&pattern.to_lowercase()))
}

/// Get danger warning message for a command
pub fn get_danger_warning(cmd: &str) -> Option<String> {
    if !is_dangerous_command(cmd) {
        return None;
    }

    let cmd_lower = cmd.to_lowercase();

    // Provide specific warnings for known patterns
    if cmd_lower.contains("rm -rf") || cmd_lower.contains("rm -fr") {
        Some("This command recursively deletes files without confirmation.".to_string())
    } else if cmd_lower.contains("dd if=") {
        Some("dd can overwrite entire disks. Double-check the target device.".to_string())
    } else if cmd_lower.contains("mkfs") {
        Some("This will format the specified device, destroying all data.".to_string())
    } else if cmd_lower.contains("git push --force") || cmd_lower.contains("git push -f") {
        Some("Force push will overwrite remote history. This may affect collaborators.".to_string())
    } else if cmd_lower.contains("git reset --hard") {
        Some("This will discard all uncommitted changes permanently.".to_string())
    } else if cmd_lower.contains("drop table") || cmd_lower.contains("drop database") {
        Some("This SQL command will permanently delete database objects.".to_string())
    } else if cmd_lower.contains("chmod -r 777") {
        Some("Setting world-writable permissions recursively is a security risk.".to_string())
    } else {
        Some("This command may have irreversible effects.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dangerous_detection() {
        assert!(is_dangerous_command("rm -rf /"));
        assert!(is_dangerous_command("sudo rm -rf /home"));
        assert!(is_dangerous_command("git push --force origin main"));
        assert!(!is_dangerous_command("ls -la"));
        assert!(!is_dangerous_command("git status"));
    }

    #[test]
    fn test_danger_warning() {
        assert!(get_danger_warning("rm -rf /").is_some());
        assert!(get_danger_warning("git push -f").is_some());
        assert!(get_danger_warning("ls").is_none());
    }
}
