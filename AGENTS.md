# Repository Guidelines

## Project Structure & Module Organization
The core Rust sources live in `src/`, with the REPL loop in `src/main.rs`, completion logic in `src/completer.rs`, YAML loading in `src/loader.rs`, and command models in `src/command_def.rs`. Fallback definitions are in `src/definitions.rs`. Command definitions are stored as YAML in `definitions/`. Dependency metadata is in `Cargo.toml`, and build artifacts are written to `target/`.

## Build, Test, and Development Commands
- `cargo build`: compile a debug build for local development.
- `cargo build --release`: create the optimized binary at `target/release/smart-command`.
- `cargo run`: build and launch the shell in one step.
- `cargo check`: fast type-checking without full codegen.
- `cargo test`: run unit tests in module-local `tests` blocks.

## Coding Style & Naming Conventions
Follow Rust 2021 defaults and `rustfmt` style (4-space indentation, trailing commas). Use `snake_case` for modules/functions, `PascalCase` for types, and `SCREAMING_SNAKE_CASE` for constants. YAML definition files should be lower-case and match the command name (e.g., `definitions/git.yaml`), include `description.en` and `description.zh`, and use `subcommands`, `flags`, and `is_path_completion` consistently.

## Testing Guidelines
Unit tests live alongside code in `#[cfg(test)] mod tests` and are named `test_*` (see `src/cli.rs` for examples). Keep tests focused on parsing, completion behavior, or config defaults. Run `cargo test` after changes that affect command loading or completion logic.

## Commit & Pull Request Guidelines
Commit subjects in this repo commonly use a short prefix like `feat: ...`; keep them concise, lower-case, and feel free to use Chinese when appropriate. PRs should describe the change, the motivation, and the validation steps (e.g., `cargo test` or manual REPL checks). Call out any new or updated YAML definitions and include an example command to try.

## Configuration & Data Files
Definitions are loaded from `./definitions/`, the executable-adjacent `definitions/`, or user/system config directories (e.g., `~/.config/smart-command/definitions/`). When adding or updating a definition, restart the shell to reload.

## Requirement

- Always respond in Chinese-simplified
