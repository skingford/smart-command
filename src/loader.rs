use std::fs;
use std::path::Path;
use crate::command_def::CommandSpec;

pub fn load_commands<P: AsRef<Path>>(dir: P) -> Vec<CommandSpec> {
    let mut commands = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    match serde_yaml::from_str::<CommandSpec>(&content) {
                        Ok(cmd) => {
                            println!("Loaded command: {}", cmd.name);
                            commands.push(cmd);
                        }
                        Err(e) => {
                            eprintln!("Failed to parse {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    } else {
        eprintln!("Warning: definitions directory not found.");
    }
    
    commands
}
