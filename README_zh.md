# Smart Command (sc)

[English](README.md) | **中文**

智能 Shell，提供上下文感知的命令补全、模糊搜索和多语言支持。

## 功能特性

- **智能补全**：Tab 补全，支持子命令和参数建议
- **组合参数补全**：智能建议常用参数组合（如 `tar -zxvf`）
- **帮助模式**：输入 `command ?` 查看分类选项（常用、文件、输出等）
- **上下文感知变量**：智能建议 `export JAVA_HOME=`、`export PATH=` 等
- **AI 命令生成**：按 `Alt+L` 或输入 `?ai <query>` 使用 Claude/Gemini/OpenAI 生成命令
- **模糊搜索**：输入 `/keyword` 搜索命令、描述和示例
- **历史前缀搜索**：`Ctrl+R` 按前缀搜索历史，实时过滤
- **示例浏览器**：使用 `example` 命令查看和搜索命令示例
- **多语言支持**：支持中英文描述
- **Git 集成**：在提示符中显示当前分支
- **历史持久化**：跨会话保存命令历史
- **自动升级**：内置版本检查和自更新（`sc upgrade`）
- **80+ 命令定义**：预配置支持 git、cargo、docker、kubectl、npm、claude、gemini 等

## 安装

### 快速安装（推荐）

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

#### Homebrew（即将推出）
```bash
brew tap skingford/tap
brew install smart-command
```

#### 二进制下载
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
# 下载最新 .deb 包
curl -LO https://github.com/skingford/smart-command/releases/latest/download/sc_0.1.0_amd64.deb
sudo dpkg -i sc_0.1.0_amd64.deb
```

#### 二进制下载
```bash
# x86_64
curl -LO https://github.com/skingford/smart-command/releases/latest/download/sc-x86_64-unknown-linux-gnu.tar.gz
tar xzf sc-x86_64-unknown-linux-gnu.tar.gz
sudo mv sc /usr/local/bin/

# ARM64 (树莓派等)
curl -LO https://github.com/skingford/smart-command/releases/latest/download/sc-aarch64-unknown-linux-gnu.tar.gz
tar xzf sc-aarch64-unknown-linux-gnu.tar.gz
sudo mv sc /usr/local/bin/
```

---

### Windows

#### Scoop（即将推出）
```powershell
scoop bucket add skingford https://github.com/skingford/scoop-bucket
scoop install smart-command
```

#### 二进制下载
```powershell
# 下载并解压
Invoke-WebRequest -Uri "https://github.com/skingford/smart-command/releases/latest/download/sc-x86_64-pc-windows-msvc.zip" -OutFile "sc.zip"
Expand-Archive -Path "sc.zip" -DestinationPath "."

# 移动到 PATH 目录，或将当前目录添加到 PATH
Move-Item sc.exe C:\Windows\System32\
```

---

### 从源码编译

#### Cargo 安装
```bash
# 需要 Rust 工具链 (https://rustup.rs)
cargo install --git https://github.com/skingford/smart-command.git

# 或克隆后本地安装
git clone https://github.com/skingford/smart-command.git
cd smart-command
cargo install --path .
```

#### 源码编译
```bash
git clone https://github.com/skingford/smart-command.git
cd smart-command
cargo build --release

# 安装二进制文件
sudo cp target/release/sc /usr/local/bin/

# 安装定义文件（补全功能必需）
mkdir -p ~/.config/smart-command
cp -r definitions ~/.config/smart-command/
```

---

### 安装后：安装定义文件

安装二进制文件后，需要命令定义文件才能使用补全功能：

```bash
# 方式 1：内置安装程序（推荐）
sc install --skip-bin

# 方式 2：手动复制
mkdir -p ~/.config/smart-command
cp -r definitions ~/.config/smart-command/
```

---

## 使用方法

### 基本命令

```bash
# 启动 shell
sc

# Tab 补全
git <TAB>           # 显示 git 子命令
git commit -<TAB>   # 显示 commit 参数

# 模糊搜索
/commit             # 搜索所有包含 "commit" 的命令
/压缩               # 中文搜索

# 切换语言
config set-lang zh  # 切换到中文
config set-lang en  # 切换到英文

# 目录导航
cd -                # 返回上一个目录
cd ~/projects       # 波浪号展开
```

### 升级命令

```bash
# 检查更新
sc upgrade --check

# 升级到最新版本
sc upgrade

# 跳过确认
sc upgrade -y

# 强制升级（重新安装当前版本）
sc upgrade --force
```

### 示例命令

查看和搜索命令示例，不会影响 Tab 补全：

```bash
# 列出所有有示例的命令
sc example
example              # REPL 模式下

# 显示特定命令的示例
sc example git
sc example docker run
example git          # REPL 模式下

# 按关键词搜索示例
sc example -s "clone"
sc example -s "push"
example search clone # REPL 模式下

# 简写（仅 REPL 模式）
ex git
examples
```

**示例输出：**
```
ℹ Examples for 'git':

   1. git clone [url]
      → 克隆仓库
   2. git commit -m "[message]"
      → 带消息提交
   3. git push -u origin [branch]
      → 推送并设置上游
   ...
```

### 键盘快捷键

| 按键 | 操作 |
|-----|------|
| `Tab` | 触发补全菜单 |
| `/` + 关键词 | 搜索命令 |
| `?` + 查询 | 自然语言查询（本地模板） |
| `command ?` | 显示命令的分类帮助 |
| `?ai` + 查询 | AI 驱动的命令生成 |
| `Alt+L` | 触发 AI 命令生成 |
| `Ctrl+R` | 历史前缀搜索 |
| `Alt+H` | 历史菜单 |
| `Ctrl+C` | 清除当前行 |
| `Ctrl+D` | 退出 shell |
| `↑` / `↓` | 浏览历史 |

### 内置命令（REPL）

| 命令 | 操作 |
|------|------|
| `example <cmd>` | 显示命令的示例 |
| `example search <query>` | 搜索所有示例 |
| `alias` | 管理命令别名 |
| `bookmark` / `bm` | 管理目录书签 |
| `@<bookmark>` | 跳转到书签目录 |
| `:<snippet>` | 展开保存的代码片段 |

**配置命令：**

| 命令 | 操作 |
|------|------|
| `config check` | 验证配置文件 |
| `config show` | 显示当前配置 |
| `config path` | 显示配置文件路径 |
| `config edit` | 在 $EDITOR 中编辑配置 |
| `config init` | 初始化配置文件 |
| `config example` | 打印示例配置 |
| `config set-lang <lang>` | 切换语言（en/zh） |

**AI 命令：**

| 命令 | 操作 |
|------|------|
| `ai status` | 显示 AI 配置 |
| `ai list` | 列出已配置的提供商 |
| `ai use <provider>` | 切换 AI 提供商 |
| `ai test` | 测试提供商连接 |
| `ai providers` | 显示可用的提供商类型 |
| `ai enable` / `ai disable` | 启用/禁用 AI |
| `?ai <query>` | 使用 AI 生成命令 |

### 搜索结果

使用 `/keyword` 搜索时：
- 输入 **数字** 直接执行命令
- 输入 `e<num>`（如 `e1`）查看命令以进行编辑
- 按 **Enter** 取消

## 命令定义

定义文件是 `definitions/` 目录中的 YAML 文件：

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

### 添加自定义命令

1. 在以下位置之一创建 YAML 文件：
   - `./definitions/`（当前目录）
   - `~/.config/smart-command/definitions/`
   - `/usr/share/smart-command/definitions/`

2. 重启 sc 以加载新定义

## 配置

### 配置文件

配置存储在 `~/.config/smart-command/config.toml`：

```toml
[general]
language = "zh"

[upgrade]
auto_check = true
check_interval_hours = 24
repository = "skingford/smart-command"
include_prerelease = false
```

### 定义文件搜索路径

Smart Command 按以下顺序搜索定义文件：
1. `./definitions/`（当前工作目录）
2. 可执行文件目录 + `/definitions/`
3. `~/.config/smart-command/definitions/`
4. `/usr/share/smart-command/definitions/`
5. `/usr/local/share/smart-command/definitions/`

### 历史文件

历史保存在 `~/.smart_command_history`（最多 1000 条记录）。

## 从源码编译

```bash
# 开发构建
cargo build

# 发布构建（优化）
cargo build --release

# 运行测试
cargo test

# 检查问题
cargo check

# 构建 .deb 包（Debian/Ubuntu）
cargo install cargo-deb
cargo deb
```

## 项目结构

```
smart-command/
├── src/
│   ├── main.rs          # REPL 循环、提示符、命令执行
│   ├── completer.rs     # 智能补全与模糊匹配
│   ├── command_def.rs   # 命令规范结构
│   ├── loader.rs        # YAML 定义加载器
│   ├── upgrade.rs       # 自更新功能
│   └── output.rs        # 彩色输出工具
├── definitions/         # YAML 命令定义（80+ 文件）
├── scripts/
│   └── test_upgrade.sh  # 升级功能测试
├── .github/
│   └── workflows/
│       └── release.yml  # 标签推送时自动发布
└── Cargo.toml           # 依赖和元数据
```

## 支持的平台

| 平台 | 架构 | 状态 |
|------|-----|------|
| macOS | Intel (x86_64) | ✅ |
| macOS | Apple Silicon (ARM64) | ✅ |
| Linux | x86_64 | ✅ |
| Linux | ARM64 | ✅ |
| Windows | x86_64 | ✅ |

## 发布流程

发布通过 GitHub Actions 自动完成。创建新版本：

```bash
# 更新 Cargo.toml 中的版本号，然后：
git tag v0.1.1
git push origin v0.1.1
```

这将自动：
1. 为所有平台构建二进制文件
2. 创建校验和
3. 打包定义文件
4. 发布 GitHub Release

## 依赖

- [reedline](https://crates.io/crates/reedline) - 交互式行编辑器
- [fuzzy-matcher](https://crates.io/crates/fuzzy-matcher) - 模糊字符串匹配
- [serde_yaml](https://crates.io/crates/serde_yaml) - YAML 解析
- [clap](https://crates.io/crates/clap) - CLI 框架
- [reqwest](https://crates.io/crates/reqwest) - HTTP 客户端（用于升级）
- [semver](https://crates.io/crates/semver) - 版本解析

## 贡献

欢迎贡献！请随时提交 Pull Request。

## 许可证

MIT
