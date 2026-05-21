// AI Agent 模块
// 基于 rig crate 实现与 LLM 的交互
// 负责构建 prompt、调用 LLM API、解析结构化响应
// 主要用途: 错误修复循环（多轮对话）和高级 Agent 功能

use crate::script::{ParamDeclaration, Script, ScriptRuntime, ShellTarget};
use anyhow::Result;
use rig::prelude::*;
use rig::{
    completion::Prompt,
    providers::anthropic::{self, completion::CLAUDE_SONNET_4_6},
};
use serde::{Deserialize, Serialize};

/// 修复请求
/// 用于 AI 修复失败脚本时的输入
#[derive(Debug, Clone)]
pub struct RepairRequest {
    pub original_script: String,
    pub stderr: String,
    pub exit_code: i32,
    pub attempt: u8,
}

/// AI Agent 客户端
/// 基于 rig crate 的 Anthropic provider
pub struct AgentClient {
    client: anthropic::Client,
    model: String,
}

impl AgentClient {
    /// 创建 Agent 客户端
    pub fn new(api_key: String, model: Option<String>) -> Result<Self> {
        let client = anthropic::Client::builder().api_key(&api_key).build()?;
        let model = model.unwrap_or_else(|| CLAUDE_SONNET_4_6.to_string());

        Ok(Self { client, model })
    }

    /// 生成脚本（基于 rig 的实现）
    /// 与 llm.rs 的 LlmClient 提供相同接口
    pub async fn generate_script(
        &self,
        user_input: &str,
        uv_tools_summary: &str,
        context_summary: &str,
    ) -> Result<GeneratedScript> {
        let system_prompt = build_system_prompt(uv_tools_summary, context_summary);

        let agent = self
            .client
            .agent(&self.model)
            .preamble(&system_prompt)
            .temperature(0.2)
            .max_tokens(4096)
            .build();

        let tool_description = format!(
            r#"你需要调用 generate_script 工具来生成脚本。

用户输入: {}

请使用 generate_script 工具，参数如下:
- name: 脚本名称
- description: 脚本用途描述
- content: 完整脚本内容
- runtime: "python_pep723" 或 "shell"
- shell_target: shell 时的目标解释器 ("bash", "pwsh", "sh" 或 null)
- params: 参数声明数组

工具 schema:
{}"#,
            user_input,
            serde_json::to_string_pretty(&*crate::llm::TOOL_SCHEMA)?
        );

        let response = agent.prompt(&tool_description).await?;

        let generated: GeneratedScript = extract_script_from_response(&response)?;
        Ok(generated)
    }

    /// 修复失败的脚本
    /// 发送原始脚本 + stderr + exit_code 给 AI，获取修复后的脚本
    pub async fn repair_script(
        &self,
        request: RepairRequest,
    ) -> Result<GeneratedScript> {
        let stderr_truncated = if request.stderr.len() > 3000 {
            &request.stderr[..3000]
        } else {
            &request.stderr
        };

        let repair_prompt = format!(
            r#"你正在修复一个执行失败的脚本。请只修改出错的最小必要部分。

## 约束
- 不得新增网络请求
- 不得新增高权限系统调用（rm -rf、chmod 等）
- 参数声明接口必须保持兼容
- 修复失败超过 2 次则不再自动重试（当前尝试: {}/2）

## 原始脚本
```
{}
```

## 错误输出 (stderr)
```
{}
```

## 退出码
{}

请调用 generate_script 工具输出修复后的脚本。"#,
            request.attempt,
            request.original_script,
            stderr_truncated,
            request.exit_code
        );

        let agent = self
            .client
            .agent(&self.model)
            .preamble("你是一个脚本修复专家。你的任务是修复执行失败的脚本。")
            .temperature(0.1)
            .max_tokens(4096)
            .build();

        let response = agent.prompt(&repair_prompt).await?;
        extract_script_from_response(&response)
    }

    /// 生成 uv 工具的 AI 摘要
    /// 输入工具的 --help 输出，返回一行描述
    pub async fn summarize_uv_tool(
        &self,
        tool_name: &str,
        help_text: &str,
    ) -> Result<String> {
        let prompt = format!(
            r#"请为以下命令行工具生成一行中文描述（不超过 50 字）。

工具名称: {}

Help 输出:
{}

请只输出描述文字，不要添加引号或其他格式。"#,
            tool_name,
            if help_text.len() > 2000 {
                &help_text[..2000]
            } else {
                help_text
            }
        );

        let agent = self
            .client
            .agent(&self.model)
            .preamble("你是一个命令行工具专家。")
            .temperature(0.3)
            .max_tokens(100)
            .build();

        let response = agent.prompt(&prompt).await?;
        Ok(response.trim().to_string())
    }

    /// 生成脚本别名推荐
    /// 基于脚本名称/描述生成拼音首字母缩写
    pub async fn suggest_alias(
        &self,
        script_name: &str,
        script_description: &str,
    ) -> Result<String> {
        let prompt = format!(
            r#"为以下脚本推荐一个简短的别名（2-4 个字母）。

脚本名称: {}
描述: {}

请只输出别名本身，不要添加解释或引号。"#,
            script_name, script_description
        );

        let agent = self
            .client
            .agent(&self.model)
            .preamble("你是一个命名专家。")
            .temperature(0.5)
            .max_tokens(10)
            .build();

        let response = agent.prompt(&prompt).await?;
        let alias = response.trim().to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");
        Ok(alias)
    }
}

/// AI 返回的脚本结构（与 llm.rs 中定义相同）
#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedScript {
    pub name: String,
    pub description: String,
    pub content: String,
    pub runtime: String,
    pub shell_target: Option<String>,
    pub params: Vec<ParamDeclaration>,
}

/// 构建 System Prompt
fn build_system_prompt(uv_tools_summary: &str, context_summary: &str) -> String {
    let platform = if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    };

    let shell_syntax = if cfg!(target_os = "windows") {
        "PowerShell 7+: $env:PARAM_NAME"
    } else {
        "Bash: $PARAM_NAME"
    };

    format!(
        r#"你是 Jita 的脚本生成器。根据用户需求生成可执行脚本。

当前平台: {platform}

## 脚本规范

Python 脚本必须使用 PEP 723 格式声明依赖：
```python
# /// script
# requires-python = ">=3.11"
# dependencies = ["rich"]
# ///
import os
# 脚本正文...
```
参数通过环境变量读取：os.environ["PARAM_NAME"]

Shell 脚本参数读取: {shell_syntax}

## 参数声明

每个参数必须包含:
- name: 环境变量名（大写下划线）
- label: 界面显示名称
- widget.type: text | secret | file | directory | select | number | toggle | textarea
- required: boolean
- description: 可选说明
- default: 可选默认值

## 可用工具

{uv_tools_summary}

## 执行上下文

{context_summary}

## 输出格式

请使用 generate_script 工具输出结果。"#,
    )
}

/// 从 LLM 响应中提取脚本 JSON
/// rig 返回纯文本，需要从中提取 JSON 块
fn extract_script_from_response(response: &str) -> Result<GeneratedScript> {
    // 尝试直接解析整个响应为 JSON
    if let Ok(script) = serde_json::from_str::<GeneratedScript>(response) {
        return Ok(script);
    }

    // 尝试从 markdown 代码块中提取 JSON
    if let Some(start) = response.find("```json") {
        let after_start = &response[start + 7..];
        if let Some(end) = after_start.find("```") {
            let json_str = after_start[..end].trim();
            if let Ok(script) = serde_json::from_str::<GeneratedScript>(json_str) {
                return Ok(script);
            }
        }
    }

    // 尝试从任意代码块中提取
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            let json_str = &response[start..=end];
            if let Ok(script) = serde_json::from_str::<GeneratedScript>(json_str) {
                return Ok(script);
            }
        }
    }

    anyhow::bail!("无法从响应中解析脚本: {}", response)
}

/// 将 GeneratedScript 转换为 Script 模型（复用 llm.rs 的逻辑）
pub fn generated_script_to_script(generated: GeneratedScript) -> Result<Script> {
    let runtime = match generated.runtime.as_str() {
        "python_pep723" => ScriptRuntime::PythonPep723,
        "shell" => ScriptRuntime::Shell,
        _ => ScriptRuntime::Shell,
    };

    let shell_target = generated.shell_target.and_then(|s| match s.as_str() {
        "bash" => Some(ShellTarget::Bash),
        "pwsh" => Some(ShellTarget::Pwsh),
        "sh" => Some(ShellTarget::Sh),
        _ => None,
    });

    let mut script = Script::new(generated.name, generated.description, generated.content);
    script.runtime = runtime;
    script.shell_target = shell_target;
    script.params_schema = generated.params;

    Ok(script)
}
