# Smart Command

An intelligent shell with context-aware command completion, fuzzy search, and multi-language support.

## Features

- **Smart Completion**: Tab completion with subcommand and flag suggestions
- **Fuzzy Search**: Type `/keyword` to search across commands, descriptions, and examples
- **Multi-language**: Supports English and Chinese descriptions
- **Git Integration**: Shows current branch in prompt
- **History Persistence**: Command history saved across sessions
- **47+ Command Definitions**: Pre-configured support for git, cargo, docker, kubectl, npm, and more

## Quick Start

```bash
# Clone and build
git clone https://github.com/kingford/smart-command.git
cd smart-command
cargo build --release

# Run
./target/release/smart-command
```

## Installation

### macOS / Linux / Windows (Recommended: Cargo Install + Built-in Installer)

```bash
# Ensure Rust is installed (see https://rustup.rs)

# Install globally (cargo)
cargo install --path .

# Install definitions (required for completions)
smart-command install --skip-bin

# Verify installation
smart-command --version
```

### Manual Installation

```bash
# Build release version
cargo build --release

# Copy to system path
sudo cp target/release/smart-command /usr/local/bin/

# Copy definitions (required)
sudo mkdir -p /usr/share/smart-command
sudo cp -r definitions /usr/share/smart-command/
```

### System Package (Debian/Ubuntu)

```bash
# Install cargo-deb
cargo install cargo-deb

# Build .deb package
cargo deb

# Install
sudo dpkg -i target/debian/smart-command_*.deb
```

## Usage

### Basic Commands

```bash
# Start the shell
smart-command

# Tab completion
git <TAB>           # Shows git subcommands
git commit -<TAB>   # Shows commit flags

# Fuzzy search
/commit             # Search for "commit" across all commands
/压缩               # Search in Chinese

# Change language
config set-lang zh  # Switch to Chinese
config set-lang en  # Switch to English

# Navigation
cd -                # Go to previous directory
cd ~/projects       # Tilde expansion
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Tab` | Trigger completion menu |
| `/` + keyword | Search commands |
| `Ctrl+C` | Clear current line |
| `Ctrl+D` | Exit shell |
| `↑` / `↓` | Navigate history |

### Search Results

When searching with `/keyword`:
- Type a **number** to execute the command directly
- Type `e<num>` (e.g., `e1`) to view the command for editing
- Press **Enter** to cancel

## Command Definitions

Definitions are YAML files in the `definitions/` directory:

```yaml
name: git
description:
  en: "Distributed version control system"
  zh: "分布式版本控制系统"
subcommands:
  - name: commit
    description:
      en: "Record changes"
      zh: "记录变更"
    flags:
      - long: message
        short: m
        description:
          en: "Commit message"
          zh: "提交信息"
```

### Adding Custom Commands

1. Create a YAML file in one of these locations:
   - `./definitions/` (current directory)
   - `~/.config/smart-command/definitions/`
   - `/usr/share/smart-command/definitions/`

2. Restart smart-command to load new definitions

## Configuration

### Definition Paths

Smart Command searches for definitions in this order:
1. `./definitions/` (current working directory)
2. Executable directory + `/definitions/`
3. `~/.config/smart-command/definitions/`
4. `/usr/share/smart-command/definitions/`
5. `/usr/local/share/smart-command/definitions/`

### History File

History is saved to `~/.smart_command_history` (1000 entries max).

## Building from Source

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Check for issues
cargo check
```

## Project Structure

```
smart-command/
├── src/
│   ├── main.rs          # REPL loop, prompt, command execution
│   ├── completer.rs     # Smart completion with fuzzy matching
│   ├── command_def.rs   # Command specification structures
│   ├── loader.rs        # YAML definition loader
│   └── definitions.rs   # Fallback command definitions
├── definitions/         # YAML command definitions (47 files)
└── Cargo.toml          # Dependencies and metadata
```

## Dependencies

- [reedline](https://crates.io/crates/reedline) - Interactive line editor
- [fuzzy-matcher](https://crates.io/crates/fuzzy-matcher) - Fuzzy string matching
- [serde_yaml](https://crates.io/crates/serde_yaml) - YAML parsing
- [dirs](https://crates.io/crates/dirs) - Platform directories

## License

MIT
