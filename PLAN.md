# Smart Command AI Enhancement Plan

## Implementation Status

### ✅ Implemented (Sprint 1 - Jan 31, 2026)

#### Phase 1: Active AI - Proactive Error Assistance
- **Error Detection Hook** - Captures exit codes and stderr after each command
- **Quick Error Hints** - Automatic detection of common error types (command not found, permission denied, file not found, syntax error, git error, package error, network error, build error)
- **Proactive AI Prompt** - Shows `[AI] Press e explain / f fix / Enter skip` after errors
- **AI Explain** - Press `e` to get AI explanation of the error
- **AI Fix** - Press `f` to get AI-suggested fix command
- **AI Retry** - Press `r` to retry the original command
- **`explain` / `??` command** - Explain the last error at any time
- **Configuration** - `[ai.active_ai]` section for enabling/disabling, ignore commands

#### Phase 2: Next Command Inline Suggestions
- **Next Command Prediction** - Shows predicted next command after successful commands
- **Error Recovery Prediction** - Suggests fix commands after errors (e.g., `sudo` for permission denied)
- **Confidence Indicator** - Shows confidence level (●●● / ●●○ / ●○○)
- **Common Sequences** - Built-in knowledge of git, cargo, npm, docker workflows
- **Configuration** - `[ai.next_command]` section for delay, min confidence

#### Phase 6: Session Context & Error Explanation
- **Session Context Tracking** - Records all commands with exit codes, stdout, stderr
- **`context show`** - Display session statistics and recent commands
- **`context clear`** - Clear session history
- **`context errors`** - Show recent errors with details
- **Session Stats** - Total commands, failed commands, success rate, duration

### New Files Created
- `src/active_ai.rs` - Active AI module with error detection and AI integration
- `src/session.rs` - Session context tracking and next command prediction

### Modified Files
- `src/config.rs` - Added `ActiveAiConfig` and `NextCommandConfig`
- `src/output.rs` - Added UI methods for Active AI hints and suggestions
- `src/main.rs` - Integrated Active AI, session tracking, explain/context commands

---

## Reference: Warp Terminal AI Features

Based on research of Warp terminal's AI capabilities, here are the key features that make it stand out:

### Warp's Core AI Features
1. **Active AI** - Proactive, contextual suggestions based on command history and errors ✅
2. **Next Command** - AI predicts next command based on session context ✅
3. **Agent Mode (Pair)** - AI plans and executes commands with user approval
4. **Dispatch Mode** - Fully autonomous AI execution with allow/deny lists
5. **Natural Language Input** - Type `#` + description to generate commands (already have `?ai`)
6. **Error Explanation** - Right-click "Ask Warp AI" to explain errors ✅
7. **Workflow Automation** - Save and parameterize command workflows
8. **Model Selection** - Choose between Claude, GPT-4o, Gemini at runtime (already have)

---

## Current Smart Command State

### Already Implemented ✅
- 9 LLM providers (Claude, OpenAI, Gemini, DeepSeek, Qwen, GLM, Ollama, OpenRouter, Custom)
- Real-time streaming responses
- Interactive AI mode (`ai on`)
- `?ai <query>` natural language command generation
- Multi-provider switching (`ai use <provider>`)
- Typo correction with Levenshtein distance
- Command prediction (bigram-based)
- Natural language templates (25+ patterns)
- 11 context providers (git, docker, k8s, npm, ssh, etc.)
- Dangerous command protection
- **Active AI with proactive error suggestions** ✅ NEW
- **Next command predictions** ✅ NEW
- **Session context tracking** ✅ NEW
- **`explain` / `??` command** ✅ NEW
- **`context` command** ✅ NEW

### Remaining Features (Compared to Warp) ❌
1. ~~**Active AI / Proactive Suggestions**~~ ✅ DONE
2. ~~**Next Command Prediction UI**~~ ✅ DONE
3. **Agent/Dispatch Mode** - No autonomous multi-step task execution
4. ~~**Error Context Menu**~~ ✅ DONE (via `explain` command)
5. **Workflow/Runbook System** - No saved parameterized workflows
6. ~~**Session Context Awareness**~~ ✅ DONE
7. **Block Organization** - No grouping of input/output for sharing
8. **Planning Mode** - No step-by-step task planning before execution

---

## Implementation Plan

### Phase 1: Active AI - Proactive Error Assistance
**Goal**: Automatically detect command failures and offer AI assistance

**Features**:
1. **Error Detection Hook**
   - Monitor exit codes after each command
   - Detect common error patterns (permission denied, not found, syntax error)
   - Capture stderr output for context

2. **Proactive Suggestion UI**
   - Show inline suggestion: `[AI] Command failed. Press Alt+E to explain, Alt+F to fix`
   - Non-intrusive notification that doesn't block workflow
   - Configurable: can be disabled in settings

3. **Quick Actions**
   - `Alt+E`: Explain the error in plain language
   - `Alt+F`: Generate a fix command
   - `Alt+R`: Retry with AI-suggested modifications

**Files to modify**:
- `src/main.rs` - Add post-command error detection
- `src/ai.rs` - Add error analysis prompts
- `src/output.rs` - Add proactive suggestion UI
- `src/config.rs` - Add `active_ai` configuration

**Complexity**: Medium

---

### Phase 2: Next Command Inline Suggestions
**Goal**: Show ghost text suggestions for next command like GitHub Copilot

**Features**:
1. **Inline Ghost Text**
   - Display predicted command in dimmed text
   - Based on: command history, current directory, git status, recent errors
   - Press `Tab` or `→` to accept

2. **Context-Aware Prediction**
   - After `git add .` → suggest `git commit -m ""`
   - After failed build → suggest fix command
   - After `cd project/` → suggest common project commands

3. **Smart Triggering**
   - Only show after 300ms of idle time
   - Don't show while actively typing
   - Dismiss on any keypress

**Files to modify**:
- `src/hinter.rs` - Implement inline suggestions
- `src/ai.rs` - Enhance `CommandPredictor` with more context
- `src/main.rs` - Add ghost text rendering
- `src/config.rs` - Add `next_command` configuration

**Complexity**: Medium

---

### Phase 3: Agent Mode (Pair Programming)
**Goal**: AI plans multi-step tasks, shows plan, executes with approval

**Features**:
1. **Task Planning**
   - User describes goal: `@agent deploy to production`
   - AI generates step-by-step plan with commands
   - Shows plan in formatted list with descriptions

2. **Interactive Execution**
   - User reviews plan: approve all, approve step-by-step, or modify
   - AI executes commands one by one
   - Pauses on errors, asks how to proceed
   - Shows progress indicator

3. **Plan Modification**
   - User can edit individual steps
   - User can skip steps
   - User can add new steps
   - AI can adapt plan based on command output

**New commands**:
- `@agent <task>` or `agent pair <task>` - Enter agent mode with task
- `@plan` - Show current plan
- `@next` - Execute next step
- `@skip` - Skip current step
- `@abort` - Abort agent session

**Files to create**:
- `src/agent.rs` - Agent mode state machine and execution

**Files to modify**:
- `src/main.rs` - Add agent mode handling
- `src/ai.rs` - Add planning prompts
- `src/output.rs` - Add plan display formatting

**Complexity**: High

---

### Phase 4: Dispatch Mode (Autonomous Execution)
**Goal**: Fully autonomous AI that executes tasks without step-by-step approval

**Features**:
1. **Autonomous Execution**
   - `@dispatch <task>` - AI plans and executes autonomously
   - Shows real-time progress of what AI is doing
   - User can interrupt with `Ctrl+C`

2. **Safety Controls**
   - Allow list: commands AI can run without confirmation
   - Deny list: commands AI must never run
   - Max steps limit (default: 20)
   - Timeout per command (default: 60s)
   - Automatic rollback on critical errors (optional)

3. **Permission Levels**
   - `read-only`: Only run read commands (ls, cat, grep, etc.)
   - `safe`: Run safe write commands (git commit, mkdir, touch)
   - `full`: Run any command (with deny list protection)

**Configuration**:
```toml
[agent.dispatch]
enabled = true
permission_level = "safe"
max_steps = 20
timeout_secs = 60
allow_list = ["git *", "cargo *", "npm *"]
deny_list = ["rm -rf /", "sudo rm *", "dd if=*"]
```

**Files to create**:
- `src/dispatch.rs` - Dispatch mode execution engine

**Files to modify**:
- `src/agent.rs` - Integrate dispatch mode
- `src/config.rs` - Add dispatch configuration

**Complexity**: High

---

### Phase 5: Workflow System (Warp Drive equivalent)
**Goal**: Save, parameterize, and share command workflows

**Features**:
1. **Workflow Definition**
   - Save command sequences as named workflows
   - Add parameters with `{{param_name}}` syntax
   - Add descriptions and documentation

2. **Workflow Execution**
   - `@run deploy` - Run a saved workflow
   - Prompt for parameters if not provided
   - AI can suggest parameter values based on context

3. **Workflow Management**
   - `workflow list` - List all workflows
   - `workflow show <name>` - Show workflow details
   - `workflow edit <name>` - Edit workflow
   - `workflow delete <name>` - Delete workflow
   - `workflow export <name>` - Export as YAML

4. **AI-Assisted Creation**
   - `workflow record` - Start recording commands
   - `workflow stop` - Stop recording and save
   - AI automatically names and parameterizes workflows

**Workflow format** (`~/.config/sc/workflows/deploy.yaml`):
```yaml
name: deploy
description: Deploy application to production
parameters:
  - name: version
    description: Version to deploy
    default: latest
  - name: env
    description: Target environment
    options: [staging, production]
steps:
  - command: git checkout {{version}}
    description: Checkout version tag
  - command: cargo build --release
    description: Build release binary
  - command: ./scripts/deploy.sh {{env}}
    description: Deploy to {{env}}
```

**Files to create**:
- `src/workflow.rs` - Workflow system

**Files to modify**:
- `src/main.rs` - Add workflow commands
- `src/completer.rs` - Add workflow name completion

**Complexity**: Medium-High

---

### Phase 6: Session Context & Error Explanation
**Goal**: Better context awareness and error explanation features

**Features**:
1. **Session History Buffer**
   - Keep last N commands and their outputs in memory
   - Include exit codes, timing, working directory
   - Pass to AI for context-aware suggestions

2. **Error Explanation**
   - `explain` or `??` after error - explain last error
   - `explain <text>` - explain any error message
   - Show: what went wrong, why, how to fix

3. **Output Analysis**
   - `analyze` - Analyze last command output
   - Useful for: log files, JSON responses, build outputs
   - AI summarizes key information

**New commands**:
- `explain` / `??` - Explain last error
- `analyze` - Analyze last output
- `context show` - Show current session context
- `context clear` - Clear session context

**Files to create**:
- `src/session.rs` - Session context management

**Files to modify**:
- `src/main.rs` - Integrate session tracking
- `src/ai.rs` - Add context to AI prompts

**Complexity**: Medium

---

### Phase 7: Enhanced UI/UX
**Goal**: Polish the AI interaction experience

**Features**:
1. **Block Organization**
   - Group related commands visually
   - Collapsible output sections
   - Copy block to clipboard

2. **Rich Formatting**
   - Markdown rendering for AI responses
   - Syntax highlighting for code in responses
   - Tables for structured data

3. **Progress Indicators**
   - Spinner during AI thinking
   - Progress bar for long operations
   - ETA for multi-step tasks

4. **Notification System**
   - Desktop notifications for long-running commands
   - Sound alerts (configurable)
   - Badge updates

**Complexity**: Medium

---

## Implementation Priority

| Phase | Feature | Priority | Effort | Impact |
|-------|---------|----------|--------|--------|
| 1 | Active AI (Error Assistance) | 🔴 High | Medium | High |
| 2 | Next Command Suggestions | 🔴 High | Medium | High |
| 3 | Agent Mode (Pair) | 🟡 Medium | High | Very High |
| 6 | Session Context & Error Explanation | 🟡 Medium | Medium | High |
| 4 | Dispatch Mode | 🟢 Low | High | Medium |
| 5 | Workflow System | 🟢 Low | Medium-High | Medium |
| 7 | Enhanced UI/UX | 🟢 Low | Medium | Medium |

---

## Recommended Implementation Order

### Sprint 1 (Phase 1 + 2): Foundation
1. Implement Active AI error detection and suggestions
2. Add inline next command predictions
3. Add configuration options for both features

### Sprint 2 (Phase 6): Context
1. Build session context tracking
2. Implement `explain` and `analyze` commands
3. Enhance AI prompts with session context

### Sprint 3 (Phase 3): Agent Mode
1. Design agent state machine
2. Implement task planning
3. Build interactive execution flow

### Sprint 4 (Phase 4 + 5): Advanced
1. Add dispatch mode with safety controls
2. Build workflow system
3. Polish UI/UX

---

## Technical Considerations

### Architecture Changes
1. **Event System**: Need pub/sub for command completion events
2. **State Machine**: Agent mode requires proper state management
3. **Async Execution**: Dispatch mode needs background execution
4. **Storage**: Workflows need persistent storage

### New Dependencies
- `indicatif` - Progress bars and spinners
- `notify-rust` - Desktop notifications (optional)
- `termimad` - Markdown rendering in terminal

### Configuration Structure
```toml
[ai]
enabled = true
active = "claude"

[ai.active_ai]
enabled = true
show_on_error = true
keybindings = { explain = "alt-e", fix = "alt-f" }

[ai.next_command]
enabled = true
delay_ms = 300
max_suggestions = 3

[ai.agent]
enabled = true
max_steps = 50
require_approval = true

[ai.dispatch]
enabled = false  # Disabled by default for safety
permission_level = "safe"
allow_list = []
deny_list = ["rm -rf *", "sudo rm *"]
```

---

## Success Metrics

1. **Error Recovery Time**: Reduce time to fix errors by 50%
2. **Command Discovery**: Increase usage of new/complex commands
3. **Task Completion**: Enable multi-step tasks without leaving terminal
4. **User Satisfaction**: Positive feedback on AI assistance

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| AI suggestions are wrong | Always require confirmation for execution |
| Dispatch mode runs dangerous commands | Strict deny list, permission levels, disabled by default |
| Performance impact from context tracking | Limit history size, lazy loading |
| Token usage costs | Local caching, smart context pruning |
| User overwhelm | Gradual disclosure, sensible defaults, easy disable |

---

## Questions for User

1. **Priority**: Which features are most important to you?
   - Active AI (proactive error help)
   - Next Command suggestions
   - Agent Mode (multi-step tasks)
   - Workflow system

2. **Safety**: How autonomous should dispatch mode be?
   - Read-only by default
   - Safe commands only
   - Full access with deny list

3. **UI Preference**: How should AI suggestions appear?
   - Inline ghost text (like Copilot)
   - Bottom bar notification
   - Side panel

4. **Scope**: Should we start with Phase 1+2 only, or plan all phases?

---

**WAITING FOR CONFIRMATION**: Please review this plan and let me know:
- Which phases to prioritize?
- Any features to add or remove?
- Any concerns about the approach?

Type `yes` to proceed with implementation, or provide modifications.
