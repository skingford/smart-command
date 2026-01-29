use crate::command_def::{CommandSpec, FlagCategory, FlagSpec};

pub fn other_specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("ls", "List directory contents")
            .flag(FlagSpec {
                long: Some("all".to_string()),
                short: Some('a'),
                description: "Include entries starting with .".into(),
                takes_value: false,
                value_type: None,
                category: FlagCategory::Common,
            })
            .field("is_path_completion", true),
        CommandSpec::new("cd", "Change the shell working directory")
            .field("is_path_completion", true),
        CommandSpec::new("config", "System configuration").subcommand(
            CommandSpec::new("set-lang", "Set display language")
                .subcommand(CommandSpec::new("en", "English"))
                .subcommand(CommandSpec::new("zh", "Chinese")),
        ),
    ]
}
