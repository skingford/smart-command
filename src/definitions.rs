use crate::command_def::{CommandSpec, FlagSpec};

pub fn other_specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("ls", "List directory contents")
            .flag(FlagSpec { long: Some("all".to_string()), short: Some('a'), description: "Include entries starting with .".into(), takes_value: false })
            .field("is_path_completion", true),
        CommandSpec::new("cd", "Change the shell working directory")
            .field("is_path_completion", true),
    ]
}
