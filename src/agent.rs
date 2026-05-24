// AI Agent 模块
// 基于 rig crate 实现与 LLM 的交互
// 负责构建 prompt、调用 LLM API、解析结构化响应
// RAG: 命令行工具索引和语义搜索

use crate::task_manager::script::{ParamDeclaration, Script, ScriptRuntime, ShellTarget};
use anyhow::Result;
use once_cell::sync::Lazy;
use rig::prelude::*;
use rig::{
    completion::Prompt,
    providers::anthropic::{self, completion::CLAUDE_SONNET_4_6},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub mod db;
pub mod embedding;
pub mod env_familiar;

use embedding::embed_text;

// =====================
// 工具 Schema 定义
// =====================

/// 工具的 JSON Schema
pub static TOOL_SCHEMA: Lazy<serde_json::Value> = Lazy::new(|| {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "description": "脚本名称" },
            "description": { "type": "string", "description": "脚本用途描述，用于后续语义匹配" },
            "content": { "type": "string", "description": "完整脚本内容" },
            "runtime": { "type": "string", "enum": ["python_pep723", "shell"], "description": "脚本运行方式" },
            "shell_target": { "type": ["string", "null"], "enum": ["bash", "pwsh", "sh", null], "description": "shell 时的目标解释器" },
            "params": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "label": { "type": "string" },
                        "widget": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": ["text", "secret", "file", "directory", "select", "number", "toggle", "textarea"] }
                            },
                            "required": ["type"]
                        },
                        "required": { "type": "boolean" },
                        "description": { "type": ["string", "null"] },
                        "default": { "type": ["string", "null"] }
                    },
                    "required": ["name", "label", "widget", "required"]
                }
            }
        },
        "required": ["name", "description", "content", "runtime", "params"]
    })
});

// =====================
// 数据结构
// =====================

/// AI 返回的脚本结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedScript {
    pub name: String,
    pub description: String,
    pub content: String,
    pub runtime: String,
    pub shell_target: Option<String>,
    pub params: Vec<ParamDeclaration>,
}

/// 修复请求
#[derive(Debug, Clone)]
pub struct RepairRequest {
    pub original_script: String,
    pub stderr: String,
    pub exit_code: i32,
    pub attempt: u8,
}

/// 命令行工具信息（用于 RAG）
#[derive(Debug, Clone)]
pub struct CliTool {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub help_text: String,
    pub usage_guide: Option<String>,
    pub embedding: Option<Vec<f32>>,
}

// =====================
// Agent 客户端
// =====================

/// AI Agent 客户端
#[derive(Clone)]
pub struct AgentClient {
    client: anthropic::Client,
    model: String,
}

impl AgentClient {
    /// 创建 Agent 客户端
    pub fn new(api_key: String, model: Option<String>, api_base: Option<String>) -> Result<Self> {
        let mut builder = anthropic::Client::builder().api_key(&api_key);
        if let Some(base) = api_base {
            builder = builder.base_url(&base);
        }
        let client = builder.build()?;
        let model = model.unwrap_or_else(|| CLAUDE_SONNET_4_6.to_string());

        Ok(Self { client, model })
    }

    /// 构建 System Prompt（包含工具使用指南）
    pub fn build_system_prompt(
        &self,
        tools_summary: &str,
        context_summary: &str,
        tool_guides: &str,
    ) -> String {
        let platform = if cfg!(target_os = "windows") {
            "Windows"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else {
            "Linux"
        };

        let guide_section = if tool_guides.is_empty() {
            String::new()
        } else {
            format!("\n\n## 相关工具使用指南\n\n{}", tool_guides)
        };

        format!(
            r#"你是 Jita 的脚本生成器。根据用户需求生成可执行脚本。

当前平台: {platform}

## 脚本规范

**优先使用 PowerShell**（Windows）或 Bash（Unix）。只有当任务需要复杂的数据处理时才考虑 Python。

### PowerShell 脚本示例
```powershell
param(
    [string]$INPUT_PATH,
    [int]$LIMIT = 10
)

$content = Get-Content -Path $env:INPUT_PATH
$content | Select-Object -First $env:LIMIT
```

### Python 脚本（仅必要时使用）
```python
# /// script
# requires-python = ">=3.11"
# dependencies = ["rich"]
# ///
import os
# 脚本正文...
```
参数通过环境变量读取：os.environ["PARAM_NAME"]

## 参数声明

每个参数必须包含:
- name: 环境变量名（大写下划线）
- label: 界面显示名称
- widget.type: text | secret | file | directory | select | number | toggle | textarea
- required: boolean
- description: 可选说明
- default: 可选默认值

## 可用工具

{tools_summary}
{guide_section}

## 执行上下文

{context_summary}

## 输出格式

请使用 generate_script 工具输出结果。"#,
        )
    }

    /// 生成脚本
    pub async fn generate_script(
        &self,
        user_input: &str,
        tools_summary: &str,
        context_summary: &str,
    ) -> Result<GeneratedScript> {
        self.generate_script_with_guides(user_input, tools_summary, context_summary, "").await
    }

    /// 生成脚本（包含工具使用指南）
    pub async fn generate_script_with_guides(
        &self,
        user_input: &str,
        tools_summary: &str,
        context_summary: &str,
        tool_guides: &str,
    ) -> Result<GeneratedScript> {
        let system_prompt = self.build_system_prompt(tools_summary, context_summary, tool_guides);

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
            serde_json::to_string_pretty(&*TOOL_SCHEMA)?
        );

        let response = agent.prompt(&tool_description).await?;
        extract_script_from_response(&response)
    }

    /// 修复失败的脚本
    pub async fn repair_script(&self, request: RepairRequest) -> Result<GeneratedScript> {
        let stderr_truncated = if request.stderr.len() > 3000 {
            &request.stderr[..3000]
        } else {
            &request.stderr
        };

        let repair_prompt = format!(
            r#"你正在修复一个执行失败的脚本。请只修改出错的最小必要部分。

## 约束
- 不得新增网络请求
- 不得新增高权限系统调用
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
            request.attempt, request.original_script, stderr_truncated, request.exit_code
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

    /// 生成命令行工具的 AI 摘要
    pub async fn summarize_tool(&self, tool_name: &str, help_text: &str) -> Result<String> {
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

    /// 生成工具的使用指南
    pub async fn generate_usage_guide(
        &self,
        tool_name: &str,
        help_text: &str,
    ) -> Result<String> {
        let prompt = format!(
            r#"请为以下命令行工具生成一段使用指南，帮助 AI 在生成脚本时正确调用该工具。

工具名称: {}

Help 输出:
{}

请用中文描述:
1. 该工具的主要功能（1-2 句）
2. 常用参数和用法示例（2-3 个例子）
3. 在脚本中调用的方式

输出格式示例:
功能: xxx
常用参数:
  - xxx: xxx
调用示例: xxx"#,
            tool_name,
            if help_text.len() > 4000 {
                &help_text[..4000]
            } else {
                help_text
            }
        );

        let agent = self
            .client
            .agent(&self.model)
            .preamble("你是一个命令行工具专家和脚本编写助手。")
            .temperature(0.3)
            .max_tokens(500)
            .build();

        let response = agent.prompt(&prompt).await?;
        Ok(response.trim().to_string())
    }

    /// 生成脚本别名推荐
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
        let alias = response
            .trim()
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "");
        Ok(alias)
    }
}

/// 从 LLM 响应中提取脚本 JSON
fn extract_script_from_response(response: &str) -> Result<GeneratedScript> {
    if let Ok(script) = serde_json::from_str::<GeneratedScript>(response) {
        return Ok(script);
    }

    if let Some(start) = response.find("```json") {
        let after_start = &response[start + 7..];
        if let Some(end) = after_start.find("```") {
            let json_str = after_start[..end].trim();
            if let Ok(script) = serde_json::from_str::<GeneratedScript>(json_str) {
                return Ok(script);
            }
        }
    }

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

/// 将 GeneratedScript 转换为 Script 模型
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

// =====================
// RAG 工具搜索
// =====================

/// 在工具嵌入中搜索相关工具
pub fn search_tools(
    query: &str,
    tools: &[(String, String, Vec<f32>)],
    top_k: usize,
) -> Vec<(String, String, f32)> {
    let query_emb = match embed_text(query) {
        Ok(emb) => emb,
        Err(_) => return Vec::new(),
    };

    let mut results: Vec<(String, String, f32)> = tools
        .iter()
        .filter_map(|(name, desc, emb)| {
            let sim = embedding::cosine_similarity(&query_emb, emb);
            if sim >= embedding::similarity_threshold() {
                Some((name.clone(), desc.clone(), sim))
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    results.truncate(top_k);

    results
}
