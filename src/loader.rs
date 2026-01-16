use crate::command_def::CommandSpec;
use std::fs;
use std::path::{Path, PathBuf};

/// Find the definitions directory from multiple candidate paths
fn find_definitions_dir() -> Option<PathBuf> {
    let candidates = [
        // 1. Current working directory
        std::env::current_dir().ok().map(|p| p.join("definitions")),
        // 2. Executable directory (for installed binaries)
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("definitions"))),
        // 3. User config directory
        dirs::config_dir().map(|p| p.join("smart-command").join("definitions")),
        // 4. Home directory config
        dirs::home_dir().map(|p| p.join(".config").join("smart-command").join("definitions")),
        // 5. System-wide directory (Unix)
        Some(PathBuf::from("/usr/share/smart-command/definitions")),
        // 6. Local system directory (Unix)
        Some(PathBuf::from("/usr/local/share/smart-command/definitions")),
    ];

    candidates
        .into_iter()
        .flatten()
        .find(|p| p.exists() && p.is_dir())
}

/// Load commands from a specific directory
fn load_from_dir<P: AsRef<Path>>(dir: P, commands: &mut Vec<CommandSpec>) -> usize {
    let mut loaded = 0;

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    match serde_yaml::from_str::<CommandSpec>(&content) {
                        Ok(cmd) => {
                            commands.push(cmd);
                            loaded += 1;
                        }
                        Err(e) => {
                            eprintln!("Failed to parse {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    }

    loaded
}

/// Load commands from the first available definitions directory
pub fn load_commands<P: AsRef<Path>>(fallback_dir: P) -> Vec<CommandSpec> {
    let mut commands = Vec::new();

    // Try to find definitions directory
    let definitions_dir =
        find_definitions_dir().unwrap_or_else(|| fallback_dir.as_ref().to_path_buf());

    if definitions_dir.exists() {
        let count = load_from_dir(&definitions_dir, &mut commands);
        if count > 0 {
            println!(
                "Loaded {} commands from: {}",
                count,
                definitions_dir.display()
            );
        }
    } else {
        eprintln!("Warning: definitions directory not found.");
        eprintln!("Searched paths:");
        eprintln!("  • ./definitions/");
        eprintln!("  • ~/.config/smart-command/definitions/");
        eprintln!("  • /usr/share/smart-command/definitions/");
    }

    commands
}
