use crate::command_def::CommandSpec;
use std::collections::HashSet;
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

/// Get definition directories from multiple candidate paths in priority order
fn definition_dirs_in_priority_order() -> Vec<PathBuf> {
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

    let mut unique_paths = HashSet::new();
    let mut result = Vec::new();

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() && candidate.is_dir() && unique_paths.insert(candidate.clone()) {
            result.push(candidate);
        }
    }

    result
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

/// Load commands from all available definitions directories in priority order.
///
/// If the same command exists in multiple directories, the higher-priority
/// directory (earlier in search order) wins.
pub fn load_commands<P: AsRef<Path>>(fallback_dir: P) -> Vec<CommandSpec> {
    let mut commands = Vec::new();
    let mut loaded_names = HashSet::new();
    let mut loaded_dirs = Vec::new();

    // Load from all available directories in priority order
    for dir in definition_dirs_in_priority_order() {
        let mut dir_commands = Vec::new();
        let count = load_from_dir(&dir, &mut dir_commands);

        let mut accepted = 0;
        for cmd in dir_commands {
            if loaded_names.insert(cmd.name.clone()) {
                commands.push(cmd);
                accepted += 1;
            }
        }

        if count > 0 {
            loaded_dirs.push((dir, accepted));
        }
    }

    // Fallback only when no command loaded from standard search paths
    if commands.is_empty() {
        let fallback = fallback_dir.as_ref();
        if fallback.exists() && fallback.is_dir() {
            let count = load_from_dir(fallback, &mut commands);
            if count > 0 {
                println!("Loaded {} commands from: {}", count, fallback.display());
            }
        } else {
            eprintln!("Warning: definitions directory not found.");
            eprintln!("Searched paths:");
            eprintln!("  • ./definitions/");
            eprintln!("  • ~/.config/smart-command/definitions/");
            eprintln!("  • /usr/share/smart-command/definitions/");
        }
    } else {
        let total = commands.len();
        if loaded_dirs.len() == 1 {
            let (dir, _) = &loaded_dirs[0];
            println!("Loaded {} commands from: {}", total, dir.display());
        } else {
            println!(
                "Loaded {} commands from {} definition directories:",
                total,
                loaded_dirs.len()
            );
            for (dir, accepted) in loaded_dirs {
                println!("  • {} ({})", dir.display(), accepted);
            }
        }
    }

    commands
}
