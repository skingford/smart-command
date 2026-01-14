# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Smart Command is a Rust-based intelligent shell with context-aware command completion. It uses `reedline` for the REPL interface and provides fuzzy-matched completions for commands, subcommands, flags, and paths.

## Build & Run Commands

```bash
# Build the project
cargo build

# Run the shell
cargo run

# Check for errors (faster than full build)
cargo check
```

## Architecture

### Core Components

**Main REPL Loop** (`src/main.rs`)
- Sets up `reedline` editor with custom keybindings
- Loads command definitions from `definitions/` directory
- Handles special commands like `cd` internally
- Executes external commands via `sh -c`

**Keybindings:**
- `Tab`: Trigger completion menu or cycle through suggestions
- `/`: Auto-trigger completion menu for command search
- `Alt+H`: History menu (placeholder)
- `exit` or `Ctrl-C/D`: Quit the shell

**Command Specification System** (`src/command_def.rs`)
- `CommandSpec`: Hierarchical structure for commands
  - Supports nested subcommands (unlimited depth)
  - Flags with short/long forms
  - `is_path_completion` field enables file/directory completion
- `FlagSpec`: Flag definitions with optional values

**Command Loader** (`src/loader.rs`)
- Scans `definitions/*.yaml` files
- Deserializes YAML into `CommandSpec` structures
- Prints loaded commands at startup for debugging

**Smart Completer** (`src/completer.rs`)
- Implements `reedline::Completer` trait
- **Special `/` prefix mode**: Type `/command` to fuzzy-search all available commands
- **Context-aware completion**:
  1. Descends command tree based on already-typed tokens
  2. Suggests subcommands for current context
  3. Suggests flags (handles both `-a` short and `--all` long forms)
  4. Supports combined short flags (e.g., `-abc`)
  5. Falls back to path completion when `is_path_completion: true`
- Fuzzy matching: Uses case-insensitive prefix matching

**Hardcoded Commands** (`src/definitions.rs`)
- Provides fallback definitions for `ls` and `cd`
- Only used if not already loaded from YAML

### Command Definition Files

YAML files in `definitions/` directory define command structures:

```yaml
name: git
description: Distributed version control system
subcommands:
  - name: commit
    description: Record changes to the repository
    flags:
      - long: message
        short: m
        description: Commit message
      - long: all
        short: a
        description: Stage all modified and deleted paths
  - name: add
    description: Add file contents to the index
    is_path_completion: true
```

**Current definitions:**
- `git.yaml`: Git subcommands and common flags
- `cargo.yaml`: Cargo build/run/check commands
- `tar.yaml`: Tar archiving commands

## Adding New Command Definitions

1. Create a new YAML file in `definitions/` directory
2. Define command structure with subcommands and flags
3. Set `is_path_completion: true` on commands that need file completion (e.g., `git add`)
4. Rebuild and run - loader automatically picks up new files

## Code Conventions

- Commands are loaded dynamically at startup
- The completer uses fuzzy prefix matching (case-insensitive)
- Path completion triggers when no subcommand/flag matches AND `is_path_completion` is true
- Short flags can be combined (e.g., `-am` for `-a -m`)
- Flags that take values stop short-flag chaining
