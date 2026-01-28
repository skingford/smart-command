# Smart Command (sc)

An intelligent shell with context-aware command completion, fuzzy search, and multi-language support.

## Features

- **Smart Completion**: Tab completion with subcommand and flag suggestions
- **Fuzzy Search**: Type `/keyword` to search across commands, descriptions, and examples
- **Multi-language**: Supports English and Chinese descriptions
- **Git Integration**: Shows current branch in prompt
- **History Persistence**: Command history saved across sessions
- **Auto Upgrade**: Built-in version checking and self-update (`sc upgrade`)
- **80+ Command Definitions**: Pre-configured support for git, cargo, docker, kubectl, npm, claude, gemini, and more

## Installation

### Quick Install (Recommended)

**macOS / Linux:**
```bash
curl -sSL https://raw.githubusercontent.com/skingford/smart-command/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/skingford/smart-command/main/install.ps1 | iex
```

---

### macOS

#### Homebrew (Coming Soon)
```bash
brew tap skingford/tap
brew install smart-command
```

#### Binary Download
```bash
# Intel Mac
curl -LO https://github.com/skingford/smart-command/releases/latest/download/sc-x86_64-apple-darwin.tar.gz
tar xzf sc-x86_64-apple-darwin.tar.gz
sudo mv sc /usr/local/bin/

# Apple Silicon (M1/M2/M3/M4)
curl -LO https://github.com/skingford/smart-command/releases/latest/download/sc-aarch64-apple-darwin.tar.gz
tar xzf sc-aarch64-apple-darwin.tar.gz
sudo mv sc /usr/local/bin/
```

---

### Linux

#### Debian / Ubuntu (.deb)
```bash
# Download the latest .deb package
curl -LO https://github.com/skingford/smart-command/releases/latest/download/sc_0.1.0_amd64.deb
sudo dpkg -i sc_0.1.0_amd64.deb
```

#### Binary Download
```bash
# x86_64
curl -LO https://github.com/skingford/smart-command/releases/latest/download/sc-x86_64-unknown-linux-gnu.tar.gz
tar xzf sc-x86_64-unknown-linux-gnu.tar.gz
sudo mv sc /usr/local/bin/

# ARM64 (Raspberry Pi, etc.)
curl -LO https://github.com/skingford/smart-command/releases/latest/download/sc-aarch64-unknown-linux-gnu.tar.gz
tar xzf sc-aarch64-unknown-linux-gnu.tar.gz
sudo mv sc /usr/local/bin/
```

---

### Windows

#### Scoop (Coming Soon)
```powershell
scoop bucket add skingford https://github.com/skingford/scoop-bucket
scoop install smart-command
```

#### Binary Download
```powershell
# Download and extract
Invoke-WebRequest -Uri "https://github.com/skingford/smart-command/releases/latest/download/sc-x86_64-pc-windows-msvc.zip" -OutFile "sc.zip"
Expand-Archive -Path "sc.zip" -DestinationPath "."

# Move to a directory in your PATH, or add the current directory to PATH
Move-Item sc.exe C:\Windows\System32\
```

---

### From Source

#### Cargo Install
```bash
# Requires Rust toolchain (https://rustup.rs)
cargo install --git https://github.com/skingford/smart-command.git

# Or clone and install locally
git clone https://github.com/skingford/smart-command.git
cd smart-command
cargo install --path .
```

#### Build from Source
```bash
git clone https://github.com/skingford/smart-command.git
cd smart-command
cargo build --release

# Install binary
sudo cp target/release/sc /usr/local/bin/

# Install definitions (required for completions)
mkdir -p ~/.config/smart-command
cp -r definitions ~/.config/smart-command/
```

---

### Post-Installation: Install Definitions

After installing the binary, you need command definitions for completions to work:

```bash
# Option 1: Built-in installer (recommended)
sc install --skip-bin

# Option 2: Manual copy
mkdir -p ~/.config/smart-command
cp -r definitions ~/.config/smart-command/
```

---

## Usage

### Basic Commands

```bash
# Start the shell
sc

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

### Upgrade Commands

```bash
# Check for updates
sc upgrade --check

# Upgrade to latest version
sc upgrade

# Skip confirmation
sc upgrade -y

# Force upgrade (reinstall current version)
sc upgrade --force
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

2. Restart sc to load new definitions

## Configuration

### Config File

Configuration is stored in `~/.config/smart-command/config.toml`:

```toml
[general]
language = "en"

[upgrade]
auto_check = true
check_interval_hours = 24
repository = "skingford/smart-command"
include_prerelease = false
```

### Definition Search Paths

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

# Build .deb package (Debian/Ubuntu)
cargo install cargo-deb
cargo deb
```

## Project Structure

```
smart-command/
├── src/
│   ├── main.rs          # REPL loop, prompt, command execution
│   ├── completer.rs     # Smart completion with fuzzy matching
│   ├── command_def.rs   # Command specification structures
│   ├── loader.rs        # YAML definition loader
│   ├── upgrade.rs       # Self-update functionality
│   └── output.rs        # Colored output utilities
├── definitions/         # YAML command definitions (80+ files)
├── scripts/
│   └── test_upgrade.sh  # Upgrade functionality test
├── .github/
│   └── workflows/
│       └── release.yml  # Auto-release on tag push
└── Cargo.toml           # Dependencies and metadata
```

## Supported Platforms

| Platform | Architecture | Status |
|----------|-------------|--------|
| macOS | Intel (x86_64) | ✅ |
| macOS | Apple Silicon (ARM64) | ✅ |
| Linux | x86_64 | ✅ |
| Linux | ARM64 | ✅ |
| Windows | x86_64 | ✅ |

## Release Process

Releases are automated via GitHub Actions. To create a new release:

```bash
# Update version in Cargo.toml, then:
git tag v0.1.1
git push origin v0.1.1
```

This will automatically:
1. Build binaries for all platforms
2. Create checksums
3. Package with definitions
4. Publish a GitHub Release

## Dependencies

- [reedline](https://crates.io/crates/reedline) - Interactive line editor
- [fuzzy-matcher](https://crates.io/crates/fuzzy-matcher) - Fuzzy string matching
- [serde_yaml](https://crates.io/crates/serde_yaml) - YAML parsing
- [clap](https://crates.io/crates/clap) - CLI framework
- [reqwest](https://crates.io/crates/reqwest) - HTTP client for upgrades
- [semver](https://crates.io/crates/semver) - Version parsing

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT
