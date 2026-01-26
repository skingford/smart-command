# Smart Command 产品文档

> 一个基于 Rust 的智能 Shell，提供上下文感知的命令补全功能

**版本**: 0.1.0
**最后更新**: 2026-01-26

---

## 目录

1. [项目概述](#项目概述)
2. [功能特性](#功能特性)
3. [系统架构](#系统架构)
4. [核心模块详解](#核心模块详解)
5. [命令定义系统](#命令定义系统)
6. [配置系统](#配置系统)
7. [安装部署](#安装部署)
8. [开发指南](#开发指南)
9. [维护指南](#维护指南)
10. [常见问题](#常见问题)

---

## 项目概述

### 简介

Smart Command 是一个智能命令行 Shell，使用 Rust 构建，核心特性是提供上下文感知的命令补全。它基于 `reedline` 库实现 REPL 交互界面，支持模糊匹配搜索命令、子命令、参数和路径。

### 核心价值

- **智能补全**: 根据当前输入上下文，提供精准的命令、子命令、参数补全
- **模糊搜索**: 使用 `/` 前缀可模糊搜索所有可用命令
- **多语言支持**: 内置中英文双语支持
- **危险命令保护**: 自动识别并警告危险命令
- **可扩展**: 通过 YAML 文件轻松添加新命令定义

### 技术栈

| 组件 | 技术选型 |
|------|----------|
| 语言 | Rust |
| REPL | reedline 0.38 |
| 终端处理 | crossterm 0.27 |
| CLI 解析 | clap 4.5 |
| 配置管理 | config 0.14 |
| 序列化 | serde + serde_yaml |
| 模糊匹配 | fuzzy-matcher 0.3 |
| 错误处理 | anyhow + thiserror |
| 日志 | tracing |

---

## 功能特性

### 1. 智能命令补全

#### Tab 补全
- 按下 Tab 键触发上下文感知补全
- 自动识别当前命令层级，提供相应的子命令、参数建议
- 支持路径补全（当命令定义中 `is_path_completion: true` 时）

#### 模糊搜索模式
- 输入 `/` 前缀进入搜索模式
- 在所有命令名称、描述、示例中进行模糊搜索
- 示例: `/gco` 可匹配 `git checkout`

#### 短参数组合
- 支持短参数组合输入，如 `-am` 等同于 `-a -m`
- 带值参数会自动终止组合链

### 2. 交互式提示符

```
~/projects/smart-command (main) ❯
```

- 显示当前工作目录（支持 `~` 展开）
- 显示 Git 分支名（可配置关闭）
- 自定义提示符符号

### 3. 危险命令保护

自动检测以下危险命令模式：
- `rm -rf` - 递归强制删除
- `dd` - 磁盘写入
- `mkfs` - 格式化文件系统
- `chmod -R 777` - 危险权限修改
- `git push -f` - 强制推送
- `git reset --hard` - 硬重置
- `DROP TABLE` / `TRUNCATE` - SQL 危险操作
- Fork bomb 等

执行前会显示警告并要求确认。

### 4. 多语言支持

- 支持英文 (en) 和中文 (zh)
- 自动从 `LANG` 环境变量检测语言
- 可通过配置文件或命令行参数切换
- 所有命令描述、示例均支持双语

### 5. 命令历史

- 文件持久化历史记录（默认 `~/.smart_command_history`）
- 可配置历史条目数量（默认 1000）
- 支持历史搜索

---

## 系统架构

### 整体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                        用户输入/输出                              │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                    main.rs (REPL 主循环)                         │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────┐     │
│  │ SmartPrompt │  │ Keybindings  │  │ Command Execution   │     │
│  └─────────────┘  └──────────────┘  └─────────────────────┘     │
└─────────────────────────────────────────────────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
┌───────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  completer.rs │     │    config.rs    │     │   output.rs     │
│  智能补全引擎  │     │    配置管理      │     │   格式化输出     │
└───────────────┘     └─────────────────┘     └─────────────────┘
        │                       │
        ▼                       ▼
┌───────────────┐     ┌─────────────────┐
│  loader.rs    │     │ AppConfig       │
│  YAML 加载器  │     │ PromptConfig    │
└───────────────┘     └─────────────────┘
        │
        ▼
┌───────────────────────────────────────────────────────────────┐
│                    definitions/*.yaml                          │
│               (67 个命令定义文件)                               │
└───────────────────────────────────────────────────────────────┘
```

### 项目目录结构

```
smart-command/
├── src/                      # Rust 源代码
│   ├── main.rs              # REPL 主循环、命令执行
│   ├── completer.rs         # 智能补全引擎
│   ├── command_def.rs       # 命令规格定义
│   ├── loader.rs            # YAML 定义加载器
│   ├── cli.rs               # CLI 参数解析
│   ├── config.rs            # 配置管理
│   ├── definitions.rs       # 内置默认命令定义
│   ├── output.rs            # 格式化输出工具
│   ├── install.rs           # 安装处理器
│   └── error.rs             # 错误类型定义
├── definitions/              # 命令定义文件 (67 个)
│   ├── git.yaml             # Git 命令 (1377 行)
│   ├── docker.yaml          # Docker 命令
│   ├── cargo.yaml           # Cargo 命令
│   └── ...                  # 其他命令
├── Cargo.toml               # 项目依赖配置
├── CLAUDE.md                # Claude 开发指导
├── README.md                # 用户文档
└── strings.yaml             # 国际化字符串
```

---

## 核心模块详解

### 1. main.rs - REPL 主循环

**职责**: 程序入口、交互循环、命令执行

**关键组件**:

```rust
// Shell 状态枚举
enum ShellState {
    Normal,
    SelectingSearchResult,
}

// 智能提示符
struct SmartPrompt {
    cwd: String,
    show_git_branch: bool,
    show_cwd: bool,
    indicator: String,
}
```

**核心流程**:
1. 解析 CLI 参数
2. 加载配置
3. 初始化 reedline 编辑器（补全器、提示符、快捷键）
4. 进入 REPL 循环
5. 处理用户输入，执行命令

**快捷键绑定**:
| 按键 | 功能 |
|------|------|
| Tab | 触发补全菜单/循环选择 |
| / | 进入搜索模式 |
| Alt+H | 历史菜单 |
| Ctrl-C/D | 退出 |

**特殊命令处理**:
- `cd` - 内置实现，支持 `cd -` 返回上一目录
- `config` - 运行时配置修改
- `exit` - 退出 Shell

### 2. completer.rs - 智能补全引擎

**职责**: 实现 `reedline::Completer` trait，提供上下文感知补全

**核心数据结构**:

```rust
pub struct SmartCompleter {
    commands: Arc<RwLock<Vec<CommandSpec>>>,  // 命令规格列表
    lang: Arc<RwLock<String>>,                // 当前语言
}
```

**补全算法**:

1. **搜索模式** (`/` 前缀):
   - 使用 `SkimMatcherV2` 进行模糊匹配
   - 搜索范围: 命令名、描述、示例
   - 返回匹配类型: Command / Description / Example

2. **Tab 补全**:
   - 解析已输入的 tokens
   - 沿命令树下降到当前层级
   - 收集当前层级的子命令、参数
   - 匹配已输入前缀进行过滤
   - 无匹配时回退到路径补全

**补全优先级**:
1. 精确匹配的子命令
2. 前缀匹配的子命令
3. 长参数 (`--flag`)
4. 短参数 (`-f`)
5. 匹配的示例
6. 文件路径补全

### 3. command_def.rs - 命令规格定义

**职责**: 定义命令的数据结构

**核心结构**:

```rust
// 国际化字符串
pub enum I18nString {
    Simple(String),
    Multilingual(HashMap<String, String>),
}

// 命令规格
pub struct CommandSpec {
    pub name: String,
    pub description: I18nString,
    pub subcommands: Vec<CommandSpec>,
    pub flags: Vec<FlagSpec>,
    pub examples: Vec<Example>,
    pub is_path_completion: bool,
}

// 参数规格
pub struct FlagSpec {
    pub long: Option<String>,
    pub short: Option<char>,
    pub description: I18nString,
    pub takes_value: bool,
}

// 使用示例
pub struct Example {
    pub scenario: I18nString,
    pub cmd: String,
}
```

### 4. loader.rs - YAML 加载器

**职责**: 扫描并加载命令定义文件

**定义文件搜索路径** (按优先级):
1. `./definitions/` - 当前工作目录
2. `<executable_dir>/definitions/` - 可执行文件目录
3. `~/.config/smart-command/definitions/` - 用户配置目录
4. `/usr/share/smart-command/definitions/` - 系统共享目录
5. `/usr/local/share/smart-command/definitions/` - 本地安装目录

**加载流程**:
1. 按优先级查找 `definitions/` 目录
2. 扫描目录中所有 `*.yaml` 文件
3. 反序列化为 `CommandSpec` 结构
4. 返回命令列表供补全器使用

### 5. config.rs - 配置管理

**职责**: 多源配置加载与管理

**配置结构**:

```rust
pub struct AppConfig {
    pub lang: String,              // 语言 (en/zh)
    pub history_path: PathBuf,     // 历史文件路径
    pub history_size: usize,       // 历史条目数
    pub danger_protection: bool,   // 危险命令保护
    pub log_level: String,         // 日志级别
    pub definitions_dir: Option<PathBuf>,  // 自定义定义目录
    pub prompt: PromptConfig,      // 提示符配置
}

pub struct PromptConfig {
    pub show_git_branch: bool,     // 显示 Git 分支
    pub show_cwd: bool,            // 显示当前目录
    pub indicator: String,         // 提示符符号
}
```

**配置优先级**:
1. 环境变量 (`SMART_CMD_*`)
2. 配置文件 (`~/.config/smart-command/config.toml`)
3. 默认值

**配置文件示例**:

```toml
lang = "zh"
history_size = 2000
danger_protection = true
log_level = "info"

[prompt]
show_git_branch = true
show_cwd = true
indicator = "❯"
```

### 6. output.rs - 格式化输出

**职责**: 提供统一的彩色输出工具

**输出方法**:
| 方法 | 样式 | 用途 |
|------|------|------|
| `success()` | 绿色 ✓ | 成功消息 |
| `warn()` | 黄色 ⚠ | 警告消息 |
| `error()` | 红色 ✗ | 错误消息 |
| `info()` | 蓝色 ℹ | 信息消息 |
| `dim()` | 灰色 | 次要文本 |
| `banner()` | ASCII Art | 启动 Logo |
| `command()` | 黄色高亮 | 命令显示 |
| `path()` | 青色下划线 | 路径显示 |

**危险命令检测**:

```rust
pub fn is_dangerous_command(cmd: &str) -> bool;
pub fn get_danger_warning(cmd: &str) -> String;
```

### 7. error.rs - 错误处理

**职责**: 定义应用级错误类型

**错误类型**:

```rust
// 应用错误
pub enum AppError {
    Config(String),           // 配置加载错误
    Io(String),              // IO 错误
    DefinitionParse(PathBuf, String),  // YAML 解析错误
    DirectoryNotFound(PathBuf),        // 目录不存在
    CommandExecution(String),          // 命令执行错误
    InvalidCommand(String),            // 无效命令
    History(String),                   // 历史记录错误
    CompletionGeneration(String),      // 补全生成错误
}

// 命令执行错误
pub enum CommandError {
    NotFound(String),        // 命令未找到
    PermissionDenied(String),  // 权限不足
    Timeout(String),         // 执行超时
    Interrupted,             // 用户中断
    InvalidArgs(String),     // 参数错误
    WorkingDir(String),      // 工作目录错误
}
```

### 8. install.rs - 安装处理器

**职责**: 处理全局安装

**安装位置** (按优先级):
- `~/.cargo/bin` (如果存在)
- `~/.local/bin`
- `~/bin`
- Windows: `%APPDATA%/Programs/smart-command/bin`

**安装内容**:
1. 二进制文件
2. 命令定义文件 -> `~/.config/smart-command/definitions/`

---

## 命令定义系统

### YAML 定义格式

```yaml
name: git
description:
  en: Distributed version control system
  zh: 分布式版本控制系统
subcommands:
  - name: commit
    description:
      en: Record changes to the repository
      zh: 记录变更到仓库
    flags:
      - long: message
        short: m
        description:
          en: Commit message
          zh: 提交信息
        takes_value: true
      - long: all
        short: a
        description:
          en: Stage all modified and deleted paths
          zh: 暂存所有修改和删除的文件
    examples:
      - cmd: git commit -m "fix: bug"
        scenario:
          en: Commit with message
          zh: 带消息提交
  - name: add
    description:
      en: Add file contents to the index
      zh: 添加文件到暂存区
    is_path_completion: true
```

### 现有命令定义 (67 个)

**开发工具**:
- `git.yaml` - Git 版本控制 (1377 行，40+ 子命令)
- `cargo.yaml` - Rust 构建工具
- `npm.yaml` / `yarn.yaml` / `pnpm.yaml` - Node.js 包管理
- `pip.yaml` - Python 包管理
- `go.yaml` - Go 工具链

**容器/云**:
- `docker.yaml` - 容器管理
- `kubectl.yaml` - Kubernetes CLI
- `ufw.yaml` - 防火墙管理

**文本处理**:
- `grep.yaml` - 模式匹配
- `sed.yaml` - 流编辑器
- `awk.yaml` - 文本处理
- `jq.yaml` - JSON 处理

**文件管理**:
- `ls.yaml` / `mkdir.yaml` / `rm.yaml` / `cp.yaml` / `mv.yaml`
- `find.yaml` - 文件搜索
- `tar.yaml` - 归档工具

**系统工具**:
- `chmod.yaml` / `chown.yaml` - 权限管理
- `ps.yaml` / `kill.yaml` - 进程管理
- `ssh.yaml` - 远程访问

### 添加新命令定义

1. 在 `definitions/` 目录创建 `<command>.yaml` 文件
2. 按照上述格式定义命令结构
3. 重新启动 Smart Command，自动加载

**最佳实践**:
- 为需要文件补全的命令设置 `is_path_completion: true`
- 提供中英文双语描述
- 添加常用示例

---

## 配置系统

### 配置文件位置

```
~/.config/smart-command/config.toml
```

### 完整配置项

```toml
# 显示语言: en (英文) 或 zh (中文)
lang = "en"

# 命令历史文件路径
history_path = "~/.smart_command_history"

# 历史记录条目数量
history_size = 1000

# 启用危险命令保护
danger_protection = true

# 日志级别: trace / debug / info / warn / error
log_level = "info"

# 自定义命令定义目录 (可选)
# definitions_dir = "/path/to/custom/definitions"

# 提示符配置
[prompt]
# 显示 Git 分支名
show_git_branch = true

# 显示当前工作目录
show_cwd = true

# 提示符符号
indicator = "❯"
```

### 环境变量

| 环境变量 | 对应配置 |
|----------|----------|
| `SMART_CMD_LANG` | lang |
| `SMART_CMD_HISTORY_PATH` | history_path |
| `SMART_CMD_HISTORY_SIZE` | history_size |
| `SMART_CMD_DANGER_PROTECTION` | danger_protection |
| `SMART_CMD_LOG_LEVEL` | log_level |
| `SMART_CMD_DEFINITIONS_DIR` | definitions_dir |

---

## 安装部署

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/your-org/smart-command
cd smart-command

# 构建 Release 版本
cargo build --release

# 安装到系统
cargo run -- install
```

### CLI 命令

```bash
# 启动交互式 Shell
smart-command

# 执行单条命令
smart-command -c "ls -la"

# 搜索命令
smart-command search git

# 列出所有命令
smart-command list

# 生成 Shell 补全脚本
smart-command completions bash > ~/.bash_completion.d/smart-command

# 查看配置
smart-command config show

# 生成示例配置
smart-command config generate

# 安装到系统
smart-command install
```

### 安装选项

```bash
# 自定义安装路径
smart-command install \
  --bin-dir ~/.local/bin \
  --definitions-dir ~/.config/smart-command/definitions

# 仅安装定义文件
smart-command install --skip-bin

# 仅安装二进制
smart-command install --skip-definitions
```

---

## 开发指南

### 开发环境设置

```bash
# 安装 Rust (如果尚未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 克隆仓库
git clone https://github.com/your-org/smart-command
cd smart-command

# 构建并运行
cargo run
```

### 常用开发命令

```bash
# 快速检查 (不生成二进制)
cargo check

# 构建调试版本
cargo build

# 构建发布版本
cargo build --release

# 运行测试
cargo test

# 代码格式化
cargo fmt

# 代码检查
cargo clippy
```

### 添加新功能

#### 1. 添加新的内置命令

编辑 `src/main.rs`，在命令执行部分添加处理逻辑:

```rust
// 在 handle_command 函数中
if cmd.starts_with("mycommand") {
    // 处理自定义命令
    return;
}
```

#### 2. 添加新的配置项

1. 在 `src/config.rs` 的 `AppConfig` 结构中添加字段
2. 更新 `Default` 实现
3. 更新配置加载逻辑

#### 3. 添加新的错误类型

在 `src/error.rs` 中扩展 `AppError` 或 `CommandError` 枚举。

### 代码规范

- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 进行代码检查
- 所有公共 API 添加文档注释
- 错误处理使用 `?` 操作符传播

---

## 维护指南

### 日常维护任务

#### 1. 更新命令定义

当命令行工具更新时，需要更新相应的 YAML 定义:

```bash
# 编辑定义文件
vim definitions/git.yaml

# 测试加载
cargo run
```

#### 2. 依赖更新

```bash
# 检查过期依赖
cargo outdated

# 更新依赖
cargo update

# 测试
cargo test
```

#### 3. 日志查看

设置日志级别查看详细信息:

```bash
SMART_CMD_LOG_LEVEL=debug smart-command
```

### 问题排查

#### 命令补全不工作

1. 检查定义文件是否存在: `ls definitions/`
2. 检查 YAML 语法: `cargo run` 查看加载错误
3. 检查配置中的 `definitions_dir`

#### 配置不生效

1. 检查配置文件路径: `smart-command config path`
2. 验证 TOML 语法
3. 检查环境变量是否覆盖

#### 性能问题

1. 减少命令定义文件数量
2. 检查是否有超大 YAML 文件
3. 使用 `cargo build --release` 构建

### 版本发布流程

1. 更新 `Cargo.toml` 版本号
2. 更新 CHANGELOG
3. 运行测试: `cargo test`
4. 构建发布版本: `cargo build --release`
5. 创建 Git tag
6. 发布到 crates.io (如适用)

---

## 常见问题

### Q: 如何添加自定义命令?

在 `definitions/` 目录创建 YAML 文件，按照格式定义命令结构，重启 Smart Command 即可。

### Q: 如何切换语言?

```bash
# 方法 1: 配置文件
echo 'lang = "zh"' >> ~/.config/smart-command/config.toml

# 方法 2: 环境变量
export SMART_CMD_LANG=zh

# 方法 3: 命令行参数
smart-command --lang zh
```

### Q: 如何禁用危险命令保护?

```bash
# 命令行参数
smart-command --no-danger-protection

# 或配置文件
echo 'danger_protection = false' >> ~/.config/smart-command/config.toml
```

### Q: 补全菜单太小/太大怎么办?

目前补全菜单大小由 reedline 库控制，暂不支持自定义。

### Q: 如何贡献命令定义?

1. Fork 仓库
2. 在 `definitions/` 添加或修改 YAML 文件
3. 提交 Pull Request

### Q: 支持哪些 Shell 补全?

支持生成以下 Shell 的补全脚本:
- Bash
- Zsh
- Fish
- PowerShell

---

## 附录

### A. 键盘快捷键

| 快捷键 | 功能 |
|--------|------|
| Tab | 触发/循环补全 |
| / + 搜索词 | 模糊搜索命令 |
| Alt+H | 历史菜单 |
| Ctrl+C | 取消当前输入 |
| Ctrl+D | 退出 Shell |
| Ctrl+A | 行首 |
| Ctrl+E | 行尾 |
| Ctrl+U | 删除到行首 |
| Ctrl+K | 删除到行尾 |
| Ctrl+W | 删除前一个单词 |
| Ctrl+R | 历史搜索 |

### B. 危险命令模式列表

- `rm -rf`
- `dd if=`
- `mkfs`
- `chmod -R 777`
- `chmod -R 000`
- `:(){ :|:& };:` (Fork bomb)
- `> /dev/sda`
- `mv /* `
- `wget | sh`
- `curl | sh`
- `git push -f`
- `git reset --hard`
- `DROP TABLE`
- `TRUNCATE TABLE`

### C. 相关资源

- [reedline 文档](https://docs.rs/reedline)
- [clap 文档](https://docs.rs/clap)
- [serde_yaml 文档](https://docs.rs/serde_yaml)

---

*文档由 Claude Code 生成，如有问题请提交 Issue。*
