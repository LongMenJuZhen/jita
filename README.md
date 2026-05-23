# Jita

一个 AI 驱动的脚本生成与执行工具。通过自然语言描述需求，Jita 利用 LLM 自动生成可执行脚本（Python/Shell），并支持参数化执行。
## 设计哲学
### 交互友好
- 多语言，多平台，多模态
  - 然而，受限于作者目前的设备，我们还是优先的做windows和安卓。
- 准实时，一定要足够快。
- 在技术选择时考虑了嵌入式的可能性，虽然短时间里不太会有裸金属的可能
- 本地偏好，为了更多网络不好的用户，~~也为了更多和你独处的时光~~

### 没有独占生态
- 利用已有的命令行工具，而不是推广自己的插件格式

### 没有第三条
## 功能特性

- **自然语言脚本生成**：用中文描述你的需求，AI 自动生成对应脚本
- **多运行时支持**：Python (PEP 723) 和 Shell 脚本
- **参数化执行**：脚本支持文本、密码、文件选择、目录选择等丰富参数类型
- **实时输出流**：执行过程中实时显示 stdout/stderr
- **历史记录**：所有生成的脚本和执行记录自动保存到本地数据库
- **系统托盘**：后台运行，托盘图标快速访问
- **热键支持**：全局快捷键触发
- **语音输入**：支持 ASR 语音转文字输入（基于 sherpa-onnx）

## 系统要求

- Windows 10/11、macOS 或 Linux
- Rust 工具链（推荐最新稳定版）
- [uv](https://github.com/astral-sh/uv)（用于执行 Python 脚本）
- Anthropic API Key 或兼容的 API（用于 LLM 生成）

## 编译运行

### 1. 克隆项目

```bash
git clone <repository-url>
cd jita
```

### 2. 安装系统依赖

**Windows:**
- 安装 [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio) 或完整的 Visual Studio
- 确保安装 "C++ CMake tools for Windows"

**macOS:**
```bash
xcode-select --install
brew install cmake portaudio
```

**Linux (Ubuntu/Debian):**
```bash
sudo apt install build-essential cmake libasound2-dev portaudio19-dev
```

### 3. 安装 Rust

```bash
# 如果没有安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 确保更新到最新版本（项目使用 Rust 2024 edition）
rustup update
```

### 4. 安装 uv

```bash
# Windows (PowerShell)
powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"

# macOS/Linux
curl -LsSf https://astral.sh/uv/install.sh | sh
```
### 安装mise
### 安装fd
```powershell
winget install sharkdp.fd
```
### 5. 编译运行

```bash
# 编译项目
cargo build --release

# 运行
cargo run --release
```

或者直接使用：

```bash
cargo run --release
```

## 配置

首次运行后，在设置界面中配置：

1. **API Key**: 你的 Anthropic API Key（或兼容的 API Key）
2. **API Base**: API 地址（留空使用默认的 Anthropic API）
3. **Model**: 使用的模型名称（默认 `claude-sonnet-4-20250514`）

API Key 会安全存储在系统密钥链中（Windows Credential Manager / macOS Keychain / Linux libsecret）。

## 使用流程

1. **输入需求**：在输入框中用自然语言描述你想要完成的脚本功能
2. **AI 生成**：Jita 调用 LLM 生成脚本，显示在审阅区域
3. **审阅修改**：查看生成的脚本内容，可选择修改
4. **执行脚本**：点击执行按钮，实时查看输出结果

## 项目结构
- `main.rs`应用主入口
- 
```
jita/
├── src/
│   ├── main.rs          # 应用入口
│   ├── app.rs           # 核心应用逻辑
│   ├── llm.rs           # LLM 客户端
│   ├── script.rs        # 脚本数据结构
│   ├── execution.rs     # 脚本执行器
│   ├── db.rs            # SQLite 数据库
│   ├── asr.rs           # ASR 语音识别
│   ├── hotkey.rs        # 热键管理
│   ├── tray.rs          # 系统托盘
│   ├── state.rs         # 应用状态
│   ├── settings.rs      # 设置管理
│   ├── task_manager.rs  # 任务管理
│   ├── agent.rs         # AI Agent
│   ├── embedding.rs     # 向量嵌入
│   ├── utils.rs         # 工具函数
│   └── ui/              # UI 模块（Slint）
├── ui/                  # Slint UI 源文件（.slint）
├── Cargo.toml           # Rust 项目配置
├── .cargo/         # Rust 项目配置
    ├──


```

## 技术栈

- **UI**: Slint
- **LLM**: rig + Anthropic API
- **Database**: SQLite (rusqlite)
- **Audio**: sherpa-onnx + cpal
- **Runtime**: Tokio (异步)
## 待办（这么多，给我一种做不完的绝望感。）
- 代码结构整理
- agent 实现优化
  - RAG
  - 环境理解

  
- 沙盒机制：AI 安全
- 长期任务：
  - 命令行启动的服务管理，端口服务统一管理
  - agent后台任务
- 本地模型：gemma4看起来很不错，也许后面会用得上。
- 灵感：
  - 随时随地，快捷键草稿纸
  - agent启发
- vi设计
  - 也许会有个二次元形象
- TUI
- openai api
## 许可证

本项目基于 [GPL v3](LICENSE) 许可证开源。
