use crate::command_def::CommandSpec;
use std::fs;
use std::path::{Path, PathBuf};

/// Get the user's custom definitions directory
pub fn get_user_definitions_dir() -> PathBuf {
    dirs::config_dir()
        .map(|p| p.join("smart-command").join("definitions"))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|p| p.join(".config").join("smart-command").join("definitions"))
                .unwrap_or_else(|| PathBuf::from("./definitions"))
        })
}

/// Check if a command definition exists in any of the search paths
pub fn command_exists(name: &str) -> Option<PathBuf> {
    let candidates = [
        std::env::current_dir().ok().map(|p| p.join("definitions")),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("definitions"))),
        dirs::config_dir().map(|p| p.join("smart-command").join("definitions")),
        dirs::home_dir().map(|p| p.join(".config").join("smart-command").join("definitions")),
        Some(PathBuf::from("/usr/share/smart-command/definitions")),
        Some(PathBuf::from("/usr/local/share/smart-command/definitions")),
    ];

    for candidate in candidates.into_iter().flatten() {
        let file_path = candidate.join(format!("{}.yaml", name));
        if file_path.exists() {
            return Some(file_path);
        }
    }
    None
}

/// Save a CommandSpec to the user's definitions directory.
///
/// # Arguments
/// * `spec` - The command specification to save
///
/// # Returns
/// * `Ok(PathBuf)` - The path where the file was saved
/// * `Err(String)` - Error message if saving failed
///
/// # Note
/// This will overwrite any existing file with the same command name.
/// The command name is sanitized to prevent path traversal attacks.
pub fn save_command(spec: &CommandSpec) -> Result<PathBuf, String> {
    let dir = get_user_definitions_dir();

    // Sanitize command name - only allow alphanumeric, dash, underscore, dot
    let sanitized_name: String = spec
        .name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();

    if sanitized_name.is_empty() {
        return Err("Invalid command name: contains no valid characters".to_string());
    }

    if sanitized_name != spec.name {
        return Err(format!(
            "Command name '{}' contains invalid characters. Only alphanumeric, dash, underscore, and dot are allowed.",
            spec.name
        ));
    }

    // Prevent path traversal
    if sanitized_name.contains("..") || sanitized_name.starts_with('.') {
        return Err("Command name cannot start with '.' or contain '..'".to_string());
    }

    // Ensure directory exists
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    let file_path = dir.join(format!("{}.yaml", sanitized_name));

    // Serialize to YAML
    let yaml =
        serde_yaml::to_string(spec).map_err(|e| format!("Failed to serialize to YAML: {}", e))?;

    // Write to file
    fs::write(&file_path, yaml).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(file_path)
}

/// Load a single command from a file path
#[allow(dead_code)]
pub fn load_command_from_file(path: &Path) -> Result<CommandSpec, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    serde_yaml::from_str(&content).map_err(|e| format!("Failed to parse YAML: {}", e))
}

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
