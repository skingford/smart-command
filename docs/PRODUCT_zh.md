# Smart Command 产品文档（中文）

版本：0.1.0  
最后更新：2026-02-05  
二进制：`sc`（包名 smart-command）  

---

## 目录

1. 项目概述
2. 功能清单
3. 操作文档
4. 配置与路径
5. 命令定义与扩展
6. 平台支持与发布

---

## 1. 项目概述

Smart Command（`sc`）是一个基于 Rust 的智能 Shell，核心目标是提供“上下文感知命令补全 + 快速搜索 + AI 辅助”。它通过 YAML 命令定义文件实现可扩展的命令/参数/示例描述，并在 REPL 中提供快捷操作与增强提示。

---

## 2. 功能清单

### 2.1 智能补全与搜索
- **Tab 补全**：按 `Tab` 显示子命令/参数/路径建议
- **短参数组合补全**：如 `-zxvf` 组合参数智能建议
- **命令帮助模式**：`command ?` 显示分类选项（子命令、参数、常用类别）
- **模糊搜索**：`/keyword` 搜索命令名/描述/示例
- **搜索选择器**：搜索结果支持数字执行、`e<num>` 编辑、Enter 取消

### 2.2 AI 能力
- **AI 命令生成**：`?ai <query>` 或 `Alt+L`
- **AI 会话模式**：`ai on` 进入对话式命令生成
- **错误解释与修复**：`explain` / `??` 解释最近错误；失败后可触发修复建议 `fix`
- **AI 文档生成**：`learn <command>` / `doc <command>` 生成命令定义并可保存

### 2.3 效率增强
- **示例浏览**：`example`/`ex` 查看与搜索命令示例
- **别名**：`alias` / `unalias` 管理快捷命令
- **书签**：`bookmark` / `bm` 管理目录；`@name` 快速跳转
- **片段**：`snippet` / `snip` 管理模板；`:<snippet>` 一键展开
- **命令计时**：`time`/`timer` 查看执行统计与慢命令

### 2.4 智能上下文与动态补全
- **项目类型识别**：Rust/Node/Python/Go/Java/Ruby，用于补全排序提升
- **动态补全提供者**：git 分支、docker 容器、k8s 资源、npm 包、make target、env/ssh/进程等

### 2.5 安全与可靠性
- **危险命令提示**：识别 `rm -rf`、`dd`、`mkfs`、`git push -f` 等并警告确认
- **语法校验**：未闭合引号/括号/重定向/管道会阻止提交
- **拼写纠错**：命令拼写错误给出“Did you mean”建议

### 2.6 可配置与体验
- **多语言**：中文/英文切换
- **主题高亮**：default / nord / dracula
- **历史持久化**：跨会话保存历史
- **自动升级**：版本检查与 `sc upgrade`

---

## 3. 操作文档

### 3.1 安装

**macOS / Linux:**
```bash
curl -sSL https://raw.githubusercontent.com/skingford/smart-command/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/skingford/smart-command/main/install.ps1 | iex
```

**源码编译：**
```bash
cargo build --release
sudo cp target/release/sc /usr/local/bin/
```

> 安装二进制后需安装 definitions：
```bash
sc install --skip-bin
```

---

### 3.2 启动与退出

```bash
sc            # 启动 REPL
exit          # 退出（或 Ctrl+D）
```

---

### 3.3 快捷键（REPL）

- `Tab`：补全菜单
- `/keyword`：搜索命令
- `?query`：本地自然语言模板（非 AI）
- `?ai query`：AI 命令生成
- `Alt+L`：快速 AI 输入
- `Ctrl+R`：历史前缀搜索
- `Alt+H`：历史菜单
- `Ctrl+C`：清除当前行
- `Ctrl+D`：退出

---

### 3.4 REPL 内置命令

**示例与帮助**
```bash
example                # 列出所有有示例的命令
example git            # 显示 git 示例
example search clone   # 搜索示例
command ?              # 命令分类帮助
```

**配置**
```bash
config check           # 校验配置
config show            # 显示配置内容
config path            # 显示配置路径
config edit            # 用 $EDITOR 编辑配置
config init            # 初始化配置文件
config example         # 输出示例配置
config set-lang zh     # 切换语言
```

**AI 管理**
```bash
ai status              # 当前 AI 状态
ai list                # 列出已配置提供商
ai use <name>          # 切换提供商
ai test                # 测试连接
ai providers           # 列出支持的 provider 类型
ai enable|disable      # 开关 AI
ai on                  # 进入 AI 会话模式
```

**AI 会话模式命令（ai on 后）**
```bash
/exit   # 退出 AI 会话
/clear  # 清空上下文
/help   # 帮助
```

**错误解释与修复**
```bash
explain    # 解释最近错误
??         # explain 的别名
```

**上下文统计**
```bash
context show|clear|errors
```

**别名 / 书签 / 片段**
```bash
alias / unalias
bookmark / bm / unbookmark / unbm
@name                     # 跳转到书签目录
snippet / snip
:snippet                  # 展开片段
```

**计时与统计**
```bash
time stats
 time slow [N]
 time avg <pattern>
```

**插件**
```bash
plugin list|enable|disable|reload|path
```

---

### 3.5 CLI 子命令（非 REPL）

```bash
sc search <query>                # 搜索命令
sc list                          # 列出所有命令
sc example [cmd ...]             # 示例（支持 -s 搜索）
sc completions <bash|zsh|fish|pwsh>
sc config show|generate|path
sc install [--skip-bin|--skip-definitions|...]
sc upgrade [--check|--force|-y|--to <version>]
sc -c "git status"              # 执行单条命令后退出
```

---

## 4. 配置与路径

**配置文件**
- `~/.config/smart-command/config.toml`

**历史记录**
- `~/.smart_command_history`

**定义文件搜索路径（按优先级）**
1. `./definitions/`
2. 可执行文件目录 + `/definitions/`
3. `~/.config/smart-command/definitions/`
4. `/usr/share/smart-command/definitions/`
5. `/usr/local/share/smart-command/definitions/`

**别名/书签/片段**
- `~/.config/smart-command/aliases.yaml`
- `~/.config/smart-command/bookmarks.yaml`
- `~/.config/smart-command/snippets.yaml`

**插件目录**
- `~/.config/smart-command/plugins/`

---

## 5. 命令定义与扩展

- 命令定义位于 `definitions/*.yaml`（当前仓库内置 78 个）
- YAML 支持：`description.en/zh`、`subcommands`、`flags`、`examples`、`is_path_completion`
- 添加/修改定义后需要重启 `sc` 以加载

**示例结构：**
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

---

## 6. 平台支持与发布

**支持平台**
- macOS (x86_64 / ARM64)
- Linux (x86_64 / ARM64)
- Windows (x86_64)

**发布方式**
- 推送 `v*` tag 触发 GitHub Actions 自动构建与发布

