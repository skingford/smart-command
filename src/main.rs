use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultPrompt, Reedline, ReedlineEvent, ReedlineMenu, Signal, ExampleHighlighter, Emacs, MenuBuilder
};
use std::process::Command;

mod command_def;
mod definitions;
mod completer;
mod loader;

use completer::SmartCompleter;

fn main() -> reedline::Result<()> {
    let commands = loader::load_commands("definitions");
    let completer = Box::new(SmartCompleter::new(commands));
    let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));

    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        reedline::KeyModifiers::NONE,
        reedline::KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );
    
    // Add Alt+H for history (placeholder as user requested)
    keybindings.add_binding(
        reedline::KeyModifiers::ALT,
        reedline::KeyCode::Char('h'),
        ReedlineEvent::Menu("history_menu".to_string()),
    );

    let highlighter = Box::new(ExampleHighlighter::new(vec![
        "git".to_string(), "tar".to_string(), "ls".to_string(), "cd".to_string(), "cargo".to_string()
    ]));

    let edit_mode = Box::new(Emacs::new(keybindings));

    let mut line_editor = Reedline::create()
        .with_completer(completer)
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_highlighter(highlighter)
        .with_edit_mode(edit_mode);

    let prompt = DefaultPrompt::default();

    println!("Welcome to Smart Command! \nType 'git <tab>' to see magic. Press Ctrl-C or type 'exit' to quit.");

    loop {
        let sig = line_editor.read_line(&prompt)?;
        match sig {
            Signal::Success(buffer) => {
                let trimmed = buffer.trim();
                if trimmed == "exit" { break; }
                if trimmed.is_empty() { continue; }

                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if let Some(cmd) = parts.first() {
                     if *cmd == "cd" {
                         if let Some(path) = parts.get(1) {
                             if let Err(e) = std::env::set_current_dir(path) {
                                 eprintln!("cd: {}", e);
                             }
                         } else if let Some(home) = dirs::home_dir() {
                                 let _ = std::env::set_current_dir(home);
                         }
                         continue;
                     }

                     let status = Command::new("sh")
                        .arg("-c")
                        .arg(&buffer)
                        .status();
                    
                     match status {
                         Ok(_) => {},
                         Err(e) => eprintln!("Error executing command: {}", e),
                     }
                }
            }
            Signal::CtrlD | Signal::CtrlC => {
                println!("\nAborted!");
                break;
            }
        }
    }

    Ok(())
}
