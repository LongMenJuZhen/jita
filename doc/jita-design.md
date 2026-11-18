# Jita 软件设计文档

> **J**ust **I**n **T**ime **A**gent — 由 AI 驱动的本地脚本管理与智能执行平台

---

## 1. 项目概述

### 1.1 定位

Jita 是一个常驻后台的桌面工具，让用户以自然语言描述需求，由 AI 即时生成并执行命令行脚本或 Python 脚本。使用后脚本自动留存，下次遇到相似需求时可模糊匹配复用，高频操作可升级为快捷键。

### 1.2 核心价值循环

```
用户描述需求
    ↓
匹配历史脚本？ ──是──→ 直接复用（填入历史参数）
    ↓ 否
AI 生成新脚本
    ↓
用户审阅 → 填写参数 → 执行
    ↓
自动生成脚本描述，建立向量索引
    ↓
使用频次积累 → 建议绑定快捷键
    ↓（下次）
用户描述相似需求 → 推荐已有脚本
```

---

## 2. 功能规格

### 2.1 核心功能

#### 全局唤醒
- 全局快捷键（默认 `Ctrl+Space`，可自定义）唤出主输入浮窗
- 浮窗出现在当前光标附近，半透明、无边框
- 失焦自动收起

#### 输入方式
- 文字输入（键盘直接输入）
- 语音输入（按住说话 / 点击麦克风图标）
  - 语音模型：本地 Sherpa-onnx（可选常驻内存或按需加载，在设置中配置）
  - 说完后自动转文字填入输入框，用户可在执行前确认

#### 已有脚本匹配（候选项）
- 输入内容实时与脚本描述库做语义匹配
- 输入框下方展示 Top-3 候选卡片，显示脚本名称、描述摘要、上次使用时间
- 用户可选择候选项复用，也可忽略候选项让 AI 生成新脚本

#### 脚本别名（Script Alias）涌现
- 单个脚本被使用超过阈值（默认 5 次）后，Jita 弹出提示：
  > "你经常使用「换密钥」，要为它绑定别名吗？推荐：hmy"
- 推荐别名基于脚本名称/描述的拼音首字母生成
- 用户可接受推荐、自定义、或跳过
- 绑定后，在主输入框输入该字符串可直接定位脚本（仅在 Jita 输入框内有效，不占用系统级快捷键）

### 2.2 脚本生成流程

#### Step 1：需求理解
- 用户的自然语言输入发给 AI
- System prompt 包含：当前已安装 uv tool 的摘要描述（预缓存）

#### Step 2：AI 输出
AI 通过 tool_use（结构化输出）同时返回两个部分：

**脚本本体**（`script` 字段）
- 优先生成 shell 命令序列
- 复杂逻辑生成 Python 脚本（通过 `uv run` 执行，依赖声明在 script 头部 inline）

**参数声明**（`params` 字段）
```json
[
  {
    "name": "API_KEY",
    "widget": { "type": "secret", "label": "API 密钥", "global_key": "OPENAI_API_KEY" },
    "required": true
  },
  {
    "name": "INPUT_FILE",
    "widget": { "type": "file", "label": "输入文件", "filter": ["*.csv", "*.xlsx"] },
    "required": true
  },
  {
    "name": "OUTPUT_DIR",
    "widget": { "type": "directory", "label": "输出目录" },
    "required": false
  }
]
```

**脚本描述**（`description` 字段）
- 一段自然语言描述，用于向量索引和后续匹配

#### Step 3：参数 Widget 类型

| 类型 | 渲染形式 |
|---|---|
| `text` | 普通文本输入框 |
| `secret` | 密码框，支持链接全局设置 |
| `file` | 系统文件选择器，支持后缀过滤 |
| `directory` | 目录选择器 |
| `select` | 下拉选择，AI 提供 options 列表 |
| `number` | 数字输入，支持 min/max |
| `toggle` | 开关，对应布尔参数 |
| `textarea` | 多行文本 |

参数通过**环境变量**传入脚本（`os.environ["PARAM_NAME"]`），secrets 不经过 shell history。

#### Step 4：审阅与执行

```
展示脚本内容（代码高亮，含 diff 对比如果有修改历史）
    ↓
[执行]  [在编辑器中修改]  [放弃]
         ↓ 选"修改"
    打开 $EDITOR / $VISUAL，阻塞等待进程退出
    重新展示更新后脚本
    ↓
填写参数表单（若有参数声明）
执行上下文预填：selected_files / clipboard → file 参数；全局设置 → secret 参数
    ↓
执行，实时流式展示 stdout / stderr
显示 [停止] 按钮，点击后向子进程发送 SIGTERM（Windows: TerminateProcess）
    ↓ 正常退出（exit code 0）
记录执行历史，更新 use_count，检查是否触发 alias 涌现提示
    ↓ 非零退出码
展示错误面板：完整 stderr + exit code
[让 AI 修复]  [手动编辑]  [放弃]
    ↓ 选"AI 修复"
发送给 AI：原始脚本 + stderr（前 3000 字符）+ exit code
修复约束（写入 system prompt）：
  - 只修改出错的最小必要部分
  - 不得新增网络请求
  - 不得新增高权限系统调用（rm -rf、chmod 等）
  - 参数声明接口必须保持兼容
AI 返回修复后脚本 → 展示 diff → 回到审阅步骤
（修复失败超过 2 次 → 提示用户手动处理，不再自动重试）
```

### 2.3 全局设置

- 命名键值对，存储于系统 keychain（`keyring` crate）
- 在设置页面可视化管理（新增 / 删除 / 查看已设置的 key 名）
- secret 类型参数在填写时提示"保存为全局设置"
- 脚本执行时，若参数有 `global_key`，自动从全局设置预填值

### 2.4 执行历史

- 记录每次执行：脚本 ID、使用的参数值（secret 类型不记录明文）、stdout/stderr 摘要、exit code、时间戳
- 复用脚本时，自动预填上次使用的参数值

### 2.5 系统托盘

常驻托盘图标，右键菜单包含：

- 打开主输入框
- 设置
- 退出

### 2.6 设置页面

| 分类 | 配置项 |
|---|---|
| AI | API Base URL、API Key、模型名称（任意兼容 OpenAI Chat Completions 协议的端点） |
| 语音 | 语音识别开关、Sherpa-onnx 模型选择、是否常驻内存 |
| 快捷键 | 全局唤醒快捷键自定义 |
| 全局设置 | 查看 / 管理共享 API Key 等键值对 |
| 脚本库 | 已生成的所有脚本列表（见 2.7） |
| uv 工具 | 已安装工具可视化管理（见 2.8） |

### 2.7 脚本库管理

- 列表展示所有历史脚本：名称、描述、生成时间、使用次数、绑定的快捷键
- 可手动编辑描述（影响匹配精度）
- 可删除脚本
- 可手动绑定 / 修改快捷键

### 2.8 uv 工具管理

- 列出本地通过 `uv tool install` 安装的所有工具
- 展示 Jita 为每个工具生成的 AI 描述（用于工具选择时的上下文注入）
- 支持一键安装新工具（输入包名）、卸载、升级
- 描述可手动编辑

---

## 3. 系统架构

### 3.1 总体结构

纯 Rust 二进制，无 WebView 依赖。Slint 直接渲染原生 UI，与业务逻辑在同一进程内通过回调/channel 通信。

```
┌──────────────────────────────────────────────────┐
│               Jita（单一 Rust 进程）               │
│                                                   │
│  ┌────────────────────────────────────────────┐  │
│  │              Slint UI 层                   │  │
│  │  浮窗输入 │ 参数表单 │ 输出面板 │ 设置页面  │  │
│  └─────────────────────┬──────────────────────┘  │
│                         │  回调 / tokio channel   │
│  ┌─────────────────────▼──────────────────────┐  │
│  │              业务逻辑层                     │  │
│  │                                            │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐ │  │
│  │  │ LLM 调用  │  │ 进程管理  │  │ 窗口控制  │ │  │
│  │  │  (rig)   │  │ (tokio)  │  │  (slint) │ │  │
│  │  └──────────┘  └──────────┘  └──────────┘ │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐ │  │
│  │  │ 向量检索  │  │ 脚本注册表│  │ 全局设置  │ │  │
│  │  │(fastembed)│  │ (SQLite) │  │ (keyring)│ │  │
│  │  └──────────┘  └──────────┘  └──────────┘ │  │
│  └────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
    │              │                  │
tray-icon    global-hotkey      sherpa-onnx
 系统托盘     全局快捷键          语音识别
    │
uv run script.py
脚本子进程
```

### 3.2 窗口结构

| 窗口 | 特性 |
|---|---|
| 主输入浮窗 | 无边框、半透明、跟随光标、置顶、失焦隐藏，由 Slint Window 属性控制 |
| 脚本审阅面板 | 从浮窗展开（同一 Window 内切换组件）|
| 设置页面 | 独立 Window，从托盘菜单打开 |

---

## 4. 数据模型

### 4.1 Script（脚本）

```sql
id              TEXT     PRIMARY KEY
name            TEXT                  -- 用户可编辑的名称
description     TEXT                  -- AI 生成，用于向量索引
content         TEXT                  -- 脚本全文
runtime         TEXT                  -- "python_pep723" | "shell"
shell_target    TEXT     NULLABLE     -- shell 时指定：bash/pwsh，null 则按平台默认
params_schema   TEXT                  -- JSON，ParamDeclaration 数组
alias           TEXT     NULLABLE     -- 绑定的脚本别名，如 "hmy"（非系统快捷键）
use_count       INTEGER  DEFAULT 0
created_at      DATETIME
last_used_at    DATETIME NULLABLE
embedding       BLOB     NULLABLE     -- 预计算的向量，避免每次重算
embedding_model TEXT     NULLABLE     -- 生成向量时使用的模型标识，用于失效检测
```

**ScriptRuntime 说明**：

```rust
enum ScriptRuntime {
    PythonPep723,  // uv run script.py，依赖声明在头部
    Shell,         // 经由 shell 执行的命令序列
}

enum ShellTarget {
    Bash,   // Unix 默认
    Pwsh,   // Windows 默认（PowerShell 7+）
    Sh,     // 最小兼容
}
```

不同 runtime 影响：参数注入方式、编辑器高亮、执行器选择、AI 生成规范。

### 4.2 ExecutionRecord（执行记录）

```
id              TEXT  PRIMARY KEY
script_id       TEXT  REFERENCES Script(id)
params_used     TEXT               -- JSON，非 secret 参数的实际值
exit_code       INTEGER
stderr_summary  TEXT               -- 前 2000 字符
executed_at     DATETIME
```

### 4.3 GlobalSetting（全局设置）

```
key             TEXT  PRIMARY KEY  -- 如 "OPENAI_API_KEY"
description     TEXT               -- 用户可读说明
-- 值存在系统 keychain，不在数据库明文存储
```

### 4.4 UvToolCache（uv 工具缓存）

```
tool_name       TEXT  PRIMARY KEY
version         TEXT
help_text       TEXT               -- --help 输出全文
ai_summary      TEXT               -- AI 生成的单行描述
cached_at       DATETIME
```

### 4.5 ExecutionContext（执行上下文，运行时结构）

脚本执行依赖的环境信息，由 Jita 自动收集并传递给 AI 或脚本：

```rust
struct ExecutionContext {
    cwd: PathBuf,                    // 工作目录，默认 $HOME，可在参数中覆盖
    selected_files: Vec<PathBuf>,    // 用户选中/拖入的文件
    clipboard_path: Option<PathBuf>, // 剪贴板中的文件路径（如果有）
    env_vars: HashMap<String, String>, // 用户填写的参数转换为环境变量
}
```

**上下文注入流程**：

1. **文件上下文自动注入**：
   - 用户拖拽文件到浮窗 → `selected_files`
   - 剪贴板包含文件路径 → `clipboard_path`
   - 这些路径自动作为 AI prompt 的一部分："用户选中了 3 个 .png 文件"

2. **参数表单预填**：
   - 脚本声明 `file` 类型参数
   - Jita 检测到 `selected_files` 有值 → 自动预填第一个文件路径
   - 用户可覆盖或选择其他文件

3. **工作目录控制**：
   - 默认在用户 HOME 目录执行
   - 参数中可添加 `directory` 类型的 `WORK_DIR`，脚本执行时 `cd` 过去

**示例**：

用户拖入 `report.csv` 后输入"转成 Excel"：

```
AI 收到的 prompt：
  用户需求：转成 Excel
  上下文：选中文件 /Users/xxx/Downloads/report.csv
  
AI 生成的参数声明：
  [
    { "name": "INPUT_CSV", "widget": { "type": "file", ... }, "default": "/Users/xxx/Downloads/report.csv" }
  ]
```

### 4.6 ParamDeclaration（参数声明,运行时结构）

```rust
enum WidgetType {
    Text { placeholder: Option<String> },
    Secret { global_key: Option<String> },
    File { 
        filter: Vec<String>,           // 文件后缀过滤，如 ["*.csv", "*.xlsx"]
        multiple: bool,                // 是否允许多选，默认 false
    },
    Directory,
    Select { options: Vec<String> },
    Number { min: Option<f64>, max: Option<f64> },
    Toggle,
    Textarea,
}

struct ParamDeclaration {
    name: String,               // 对应环境变量名，大写下划线
    label: String,              // 界面显示名称
    widget: WidgetType,
    required: bool,
    description: Option<String>,
    default: Option<String>,    // 默认值，可从 ExecutionContext 推断
}
```

**file widget 特殊行为**：
- 自动检测 `ExecutionContext.selected_files` 并预填
- 支持拖拽文件到输入框
- 支持从剪贴板粘贴文件路径
- `multiple: true` 时，环境变量值为路径列表（用 `:` 分隔，Windows 用 `;`）

**default 值推断规则**：
```rust
if widget == File && context.selected_files.len() > 0 {
    default = Some(context.selected_files[0].to_string())
} else if widget == Directory && context.cwd != HOME {
    default = Some(context.cwd.to_string())
}
```

---

## 5. 关键模块设计

### 5.1 LLM 交互

**多 Provider 支持**：

Jita 通过 OpenAI Chat Completions 协议与 LLM 通信，用户只需配置三个字段：

| 字段 | 示例（Anthropic） | 示例（其他） |
|---|---|---|
| API Base URL | `https://api.anthropic.com/v1` | `https://api.deepseek.com/v1` |
| API Key | `sk-ant-...` | `sk-...` |
| 模型名称 | `claude-sonnet-4-20250514` | `deepseek-chat` |

设置页面提供常见厂商的预设模板（一键填入 Base URL），用户只需补充 API Key 和选择模型。

tool_use / function calling 是 OpenAI 协议的标准功能，主流兼容厂商均支持。对于不支持 tool_use 的端点，降级为 JSON mode（在 system prompt 中要求严格 JSON 输出，手动解析）。

使用 `rig` crate 的 OpenAI-compatible provider，切换 provider 只需更换 client 配置，业务逻辑无需修改。

**System prompt 结构**：

```
[角色定义]
你是 Jita 的脚本生成器。根据用户需求生成可执行脚本。

[脚本规范]
Python 脚本必须使用 PEP 723 格式声明依赖：
  # /// script
  # dependencies = ["rich"]
  # ///
参数通过环境变量读取：os.environ["PARAM_NAME"]

Shell 脚本参数读取：
  Unix(bash): $PARAM_NAME
  Windows(pwsh): $env:PARAM_NAME

[ParamDeclaration JSON Schema]
{schema}  ← 由 schemars 自动生成，注入此处

[可用工具列表]
{tool_summaries}  ← 见下方注入策略
```

**工具上下文两级注入策略**：

```
第一级（始终注入，轻量）：
  所有已安装 uv 工具的 ai_summary
  格式："ruff: Python 代码检查和格式化工具，支持 --fix 自动修复"
  约 20 token/条，100 个工具 = 约 2000 token

第二级（按需注入，精准）：
  仅在以下情况注入完整 help_text：
  - AI 在第一轮已选定具体工具（通过工具名匹配）
  - 脚本复用场景，已知绑定工具
  使用两步调用：第一步选工具，第二步带完整文档生成
```

**平台 Shell 规范**：

| 平台 | 默认 Shell | 环境变量语法 | 多文件路径分隔 |
|---|---|---|---|
| macOS / Linux | `bash` | `$NAME` | `:` |
| Windows | `pwsh`（PowerShell 7+） | `$env:NAME` | `;` |

AI 生成 shell 脚本时，system prompt 注入当前平台类型，确保语法正确。Python 脚本通过 `os.environ` 读取，无平台差异。

### 5.2 语义匹配

- 使用本地嵌入模型（`fastembed` crate，基于 ONNX Runtime，无需 Python）
- 脚本注册/更新时异步计算 embedding，存入 `Script.embedding`
- 查询时计算输入 embedding，做 cosine 相似度排序，取 Top-3
- 相似度低于阈值时不展示候选项，避免噪声

### 5.3 进程管理

脚本通过 `uv run` 执行，依赖以 PEP 723 格式声明在脚本头部，uv 全局缓存包，无需为每个脚本维护独立 venv：

```python
# /// script
# requires-python = ">=3.11"
# dependencies = ["rich", "httpx"]
# ///
import os, rich
# 脚本正文...
```

执行时直接：

```
uv run script.py
```

uv 检测到头部声明，自动在全局缓存中解析依赖并运行，首次执行后包已缓存，后续启动极快。

```
tokio::process::Command::new("uv")
  .args(["run", &script_path])
  .envs(&param_env_vars)        // 参数以环境变量注入
  .stdout(Stdio::piped())
  .stderr(Stdio::piped())

stdout/stderr → 逐行 → tokio channel → Slint UI 实时追加
exit code → 非零 → 触发错误修复流程
```

### 5.4 工具缓存刷新策略

- 首次启动时扫描 `uv tool list`，为所有工具抓取 `--help` 并生成 AI 摘要
- 后台定时检查（每日一次）`uv tool list` 版本变更，有变更则更新对应缓存
- 用户在 uv 工具管理页面可手动触发刷新

### 5.5 快捷键涌现

```
每次执行后：
  Script.use_count += 1
  if use_count == THRESHOLD（默认 5）and shortcut == null:
    生成推荐快捷键（脚本名/描述的拼音首字母）
    推送托盘通知："是否为「xxx」绑定快捷键？"
    用户确认 → 写入 Script.shortcut
    注册到输入框匹配逻辑（alias 机制，非全局快捷键）

输入框接收输入时：
  优先检查是否精确匹配某个 Script.shortcut
  匹配到 → 直接跳到参数填写步骤，跳过语义搜索
```

**术语说明**：
- **全局快捷键（Global Hotkey）**：系统级按键组合（如 `Ctrl+Space`），用于唤醒 Jita 主窗口，由 `global-hotkey` crate 监听
- **脚本别名（Script Alias）**：输入框内的文本快捷方式（如 `hmy`），相当于 shell alias，仅在 Jita 输入框内有效，不占用系统快捷键

### 5.6 并发任务模型

Jita 支持多任务并行执行，用户可在一个脚本运行时触发新脚本。

**执行队列设计**：

```rust
struct TaskManager {
    running_tasks: HashMap<TaskId, RunningTask>,  // 当前运行的任务
    max_concurrent: usize,                         // 最大并发数（默认 3）
}

struct RunningTask {
    script_id: String,
    process: Child,              // tokio::process::Child
    stdout_rx: Receiver<String>, // 输出流 channel
    started_at: DateTime,
}
```

**UI 交互**：

主输入框展开后变为**任务面板**，显示：
- 当前运行中的任务列表（脚本名、运行时长、实时输出预览）
- 每个任务右侧有 `[停止]` 按钮
- 点击任务卡片展开完整输出

**执行策略**：
- 达到 `max_concurrent` 时，新任务进入队列等待
- 用户可手动调整优先级（拖拽排序）
- 长时间运行任务（>30s）自动最小化到托盘，完成后通知

**状态同步**：
```
UI 层（Slint）
    ↓ tokio::sync::mpsc
业务层（Rust）
    ↓ tokio::process
子进程（uv run）
```

每个任务独立 channel，stdout/stderr 实时推送至对应 UI 卡片。

### 5.7 外部依赖检测

Jita 启动时检测必要的外部依赖，缺失时展示引导而非崩溃。

**uv 检测**：

```rust
fn check_uv() -> UvStatus {
    match Command::new("uv").arg("--version").output() {
        Ok(out) if out.status.success() => UvStatus::Available(parse_version(&out.stdout)),
        _ => UvStatus::Missing,
    }
}
```

检测到 `uv` 缺失时，在主窗口展示提示横幅：

```
⚠️  未检测到 uv，脚本执行功能不可用。
    uv 是 Jita 的运行依赖，请通过系统包管理器或官网安装。
    [打开下载页面 →]   （跳转至 https://docs.astral.sh/uv/getting-started/installation/）
```

安装后无需重启，用户可点击"重新检测"按钮刷新状态。

**uv 版本要求**：最低支持版本 `0.4.0`（PEP 723 inline script metadata 支持稳定版本）。检测到低版本时提示升级，但不阻止非 Python 脚本功能的使用。

**其他依赖**：

| 依赖 | 用途 | 缺失行为 |
|---|---|---|
| `uv` | Python 脚本执行 | 提示下载，禁用相关功能 |
| `$EDITOR` / `$VISUAL` | 脚本审阅时打开编辑器 | 降级为内置只读代码展示，提示用户配置环境变量 |
| Sherpa-onnx 模型文件 | 语音识别 | 语音按钮置灰，提示在设置页面下载模型 |

### 5.8 国际化 (i18n)

Jita 采用**双轨翻译系统**，分别处理 Rust 代码和 Slint UI 的文本国际化。

#### 总体架构

```
┌──────────────────────────────────────────────────────────┐
│                     i18n 系统                             │
│                                                          │
│  Rust 代码                    Slint UI                    │
│  ┌──────────────┐            ┌──────────────────┐        │
│  │ fluent-bundle │            │ bundled .po 文件 │        │
│  │ 运行时加载    │            │ 编译期嵌入       │        │
│  └──────┬───────┘            └────────┬─────────┘        │
│         │                              │                 │
│    i18n::t("key")              @tr("key")               │
│         │                              │                 │
│  locales/*.ftl              lang/<lang>/LC_MESSAGES/*.po │
└─────────┴──────────────────────────────┴─────────────────┘
```

#### Slint UI 翻译（编译期）

- 翻译源文件：`lang/<locale>/LC_MESSAGES/*.po`（GNU gettext 格式）
- `build.rs` 通过 `with_bundled_translations("lang")` 在编译时将 `.po` 文件嵌入二进制
- Slint 代码中使用 `@tr("msgid")` 标记待翻译文本
- 运行时通过 `slint::select_bundled_translation("zh")` 切换语言，无需外部文件

**示例**（`ui/main.slint`）：
```slint
text: @tr("设置");
```

对应 `lang/en/LC_MESSAGES/jita.po`：
```po
msgid "设置"
msgstr "Settings"
```

#### Rust 代码翻译（运行时）

- 使用 Mozilla Fluent 格式（`.ftl` 文件）
- `fluent-bundle` crate 提供运行时解析和参数插值
- 支持复数、变量插值等复杂语法
- Bundle 通过 `thread_local` 缓存，仅在 locale 变化时重建

**API**：

```rust
// 简单翻译
i18n::t("ai_generating")  // -> "AI 生成中..."

// 带参数翻译
i18n::t_args("generation_failed", &[("error", &e.to_string())])
// -> "生成失败: 连接超时"
```

**翻译文件**（`locales/zh-CN.ftl`）：
```ftl
ai_generating = AI 生成中...
generation_failed = 生成失败: { $error }
task_started = 任务 { $task_id } 开始执行...
```

**实现要点**（`src/i18n.rs`）：

```rust
pub fn init() {
    // 1. 检测 locale：JITA_LANG > LANG > 系统默认
    let locale = std::env::var("JITA_LANG")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    set_locale(&locale);

    // 2. 同步到 Slint UI
    let _ = slint::select_bundled_translation(lang_code());
}

pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    // thread_local 缓存 bundle，locale 变化时自动重建
    thread_local! {
        static CACHED_LOCALE: AtomicU8 = AtomicU8::new(255);
        static BUNDLE: RwLock<FluentBundle<FluentResource>> = ...;
    }
    // ... 检测变化 → 重建 bundle → 翻译
}
```

#### Locale 检测优先级

1. `JITA_LANG` 环境变量（最高优先级，方便用户覆盖）
2. `LANG` 环境变量（标准 Unix locale）
3. 系统默认（fallback 到中文）

运行时切换：`i18n::set_locale("en")` + `slint::select_bundled_translation("en")`

#### 新增语言流程

1. **Rust 侧**：新建 `locales/<lang>.ftl`，在 `make_bundle()` 中注册
2. **Slint 侧**：新建 `lang/<locale>/LC_MESSAGES/jita.po`
3. **注册 locale**：在 `Locale` enum 和 `lang_code()` 中增加映射
4. **重建**：`cargo build` 自动将新的 `.po` 文件编译进二进制

---

## 6. 技术选型

| 层 | 选型 | 理由 |
|---|---|---|
| UI 框架 | Slint | 纯 Rust，无 WebView，原生渲染，声明式 UI，单一二进制 |
| LLM 调用 | `rig` crate | Rust 原生 agent 框架，tool_use 一等公民 |
| 嵌入模型 | `fastembed` crate | ONNX Runtime，纯 Rust，无 Python 依赖 |
| 数据库 | SQLite via `rusqlite` | 本地零配置，够用 |
| 密钥存储 | `keyring` crate | 跨平台系统 keychain 封装 |
| 语音识别 | `sherpa-onnx` | ONNX 推理，支持多模型（含流式），比 whisper-rs 更灵活 |
| JSON Schema | `schemars` crate | 从 Rust 类型自动生成 schema |
| 全局快捷键 | `global-hotkey` crate | 独立 crate，无需 Tauri |
| 窗口定位 | Slint Window API | 光标坐标读取后手动定位 |
| 系统托盘 | `tray-icon` crate | 跨平台，与 `global-hotkey` 同生态 |
| 脚本执行 | `uv run` + PEP 723 | 内联依赖声明，全局包缓存，无需 per-script venv |
| 国际化（Rust）| `fluent-bundle` crate | Mozilla Fluent 格式，支持复数/参数插值 |
| 国际化（Slint）| Slint `with_bundled_translations` | 编译期捆绑 `.po` 文件，运行时零开销 |

---

## 7. 安全模型

- **执行前必须审阅**：所有新生成脚本在执行前展示完整内容，用户显式确认
- **复用脚本可跳过审阅**：已执行过的脚本默认跳过审阅，但参数填写前仍可展开查看
- **Secret 不落盘**：secret 类型参数通过环境变量传入进程，不写入执行记录，全局设置存 keychain
- **编辑器修改**：用户可在执行前通过 `$EDITOR` 修改脚本，修改后重新展示
- **沙箱边界**：Jita 本身不做沙箱，脚本在用户权限下运行，审阅机制是唯一防线

---

## 8. 状态机

Jita 有两个独立的状态层：主窗口状态（用户交互流）和任务状态（每个执行任务独立）。

### 8.1 主窗口状态

```
                    ┌─────────────────────────────────────────┐
                    │                                         │
                    ▼                                         │
              ┌──────────┐   全局快捷键 / 托盘点击             │
  ────────►   │   Idle   │ ──────────────────────────────►    │
              └──────────┘                                    │
                                                             │
              ┌──────────┐   用户输入中                       │
              │  Input   │ ◄──────────────────────────────    │
              └────┬─────┘                                    │
                   │                                          │
          ┌────────┴─────────┐                               │
          │ 有输入内容时      │                               │
          ▼                  ▼                               │
    ┌──────────┐      ┌───────────┐                          │
    │ Matching │      │ (alias    │                          │
    │(语义检索) │      │  命中)    │                          │
    └────┬─────┘      └─────┬─────┘                         │
         │ 无候选 / 用户     │                               │
         │ 选择"生成新脚本"  │                               │
         ▼                  │                               │
    ┌────────────┐          │                               │
    │ Generating │          │                               │
    │(LLM 生成中)│          │                               │
    └─────┬──────┘          │                               │
          │                 │                               │
          ▼                 ▼                               │
    ┌──────────────────────────┐                            │
    │         Reviewing        │                            │
    │  (展示脚本，等待用户确认) │                            │
    └────────────┬─────────────┘                            │
                 │ 确认执行                                  │
                 ▼                                          │
    ┌──────────────────────────┐                            │
    │        ParamInput        │                            │
    │  (填写参数表单，可能为空) │                            │
    └────────────┬─────────────┘                            │
                 │ 提交参数                                  │
                 ▼                                          │
    任务移交 TaskManager        ──────────────────────────►  │
    主窗口回到 Input 状态（可继续发起新任务）                  │
                                                            │
              ESC / 失焦 ───────────────────────────────────┘
              (任何状态下均可退回 Idle)
```

### 8.2 任务状态（每个任务独立）

```
  ┌─────────┐
  │ Queued  │  等待执行槽位（超出 max_concurrent 时）
  └────┬────┘
       │ 有空槽
       ▼
  ┌─────────┐
  │ Running │ ◄────────────────────────────────────┐
  └────┬────┘                                      │
       │                                           │
  ┌────┴─────────────────────┐                    │
  │                          │                    │
  ▼ exit 0                   ▼ exit ≠ 0            │
┌─────────┐             ┌─────────┐               │
│ Success │             │ Failed  │               │
└─────────┘             └────┬────┘               │
                             │                    │
                   ┌─────────┴──────────┐         │
                   │ 用户选"AI 修复"     │         │
                   ▼                    │         │
              ┌──────────┐              │         │
              │ Repairing│              │         │
              └────┬─────┘              │         │
                   │ 修复完成            │         │
                   ▼                    │         │
              ┌──────────┐              │         │
              │ Reviewing│              │         │
              │(修复审阅) │              │         │
              └────┬─────┘              │         │
                   │ 确认               │         │
                   └────────────────────┘         │
                      再次执行 ────────────────────┘

  任何状态 + 用户点击[停止] → Cancelled
  Repairing 失败超 2 次 → Failed（不再自动重试）
```

### 8.3 状态与 UI 组件对应

| 主窗口状态 | 显示内容 |
|---|---|
| `Idle` | 窗口隐藏 |
| `Input` | 输入框 + 候选卡片列表 + 任务面板（若有运行中任务） |
| `Matching` | 输入框 + 加载指示器 |
| `Generating` | 输入框（禁用）+ "AI 生成中..." 动画 |
| `Reviewing` | 脚本代码展示区 + [执行/编辑/放弃] |
| `ParamInput` | 参数表单 + [提交/取消] |

| 任务状态 | 任务卡片样式 |
|---|---|
| `Queued` | 灰色，显示队列位置 |
| `Running` | 蓝色边框，实时滚动输出，[停止] 按钮 |
| `Success` | 绿色，输出可展开查看 |
| `Failed` | 红色，stderr 摘要，[AI 修复/手动编辑/放弃] |
| `Repairing` | 黄色，"AI 修复中..." |
| `Cancelled` | 灰色，"已停止" |

---

## 9. 开发路线图

### Phase 1：核心可用（MVP）

- [ ] Slint 项目初始化，`global-hotkey` + `tray-icon` 集成
- [ ] 基础浮窗（无边框、半透明、光标定位）
- [ ] 文字输入 → AI 生成脚本 → 展示 → 执行 → 流式输出
- [ ] 参数声明 → 基础 widget 渲染（text / secret / file / directory）
- [ ] 脚本存储（SQLite）+ 执行历史
- [ ] 执行取消（kill process）

### Phase 2：智能复用

- [ ] fastembed 向量索引
- [ ] 候选项匹配 + 输入框实时推荐
- [ ] 执行历史参数预填
- [ ] 全局设置（keychain）

### Phase 3：体验打磨

- [ ] 语音输入（sherpa-onnx）
- [ ] 错误修复循环（AI fix）
- [ ] 快捷键涌现机制（alias）
- [ ] 设置页面完整实现
- [ ] 并发任务面板 UI

### Phase 4：工具管理

- [ ] uv 工具可视化安装 / 卸载 / 升级
- [ ] uv tool --help 缓存 + AI 摘要生成
- [ ] 脚本库管理页面（编辑描述、删除、手动绑定快捷键）

---

## 10. 待决策项

| 问题 | 备选方案 | 现状 |
|---|---|---|
| 嵌入模型大小 vs 精度 | all-MiniLM-L6-v2（小快）vs nomic-embed-text（更准）| 待测试效果后决定 |
| Sherpa-onnx 模型选型 | Whisper tiny/base vs SenseVoice（中文更强）| 待评估中文识别效果 |
| 多语言脚本支持 | 现仅 Python + shell，是否支持 Node.js 等 | 暂不支持 |
| 文件管理器右键集成 | 跨平台实现复杂 | 暂搁置，后期作为可选功能 |
| 脚本执行沙箱 | 裸执行（当前）vs 容器隔离 | 暂不做，uv run 的 venv 隔离已够用 |
