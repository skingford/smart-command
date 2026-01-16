//! Colored output utilities following CLI development standards
//!
//! Output format standards:
//! - Success: green with checkmark
//! - Warning: yellow with warning sign
//! - Error: red with X mark
//! - Info: blue with info sign

use nu_ansi_term::{Color, Style};

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

    /// Bold output for emphasis
    pub fn bold(msg: &str) {
        let style = Style::new().bold();
        println!("{}", style.paint(msg));
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
