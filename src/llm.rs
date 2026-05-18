use crate::script::{ParamDeclaration, Script, ScriptRuntime, ShellTarget};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedScript {
    pub name: String,
    pub description: String,
    pub content: String,
    pub runtime: String,
    pub shell_target: Option<String>,
    pub params: Vec<ParamDeclaration>,
}

pub struct LlmClient {
    api_key: String,
    model: String,
    api_base: String,
    http: reqwest::Client,
}

impl LlmClient {
    pub fn new(api_key: String, model: String, api_base: Option<String>) -> Self {
        Self {
            api_key,
            model,
            api_base: api_base.unwrap_or_else(|| "https://api.anthropic.com".to_string()),
            http: reqwest::Client::new(),
        }
    }

    pub fn build_system_prompt(
        &self,
        uv_tools_summary: &str,
        context_summary: &str,
    ) -> String {
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

## 参数声明 JSON Schema

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

    pub async fn generate_script(
        &self,
        user_input: &str,
        system_prompt: &str,
    ) -> Result<GeneratedScript> {
        let tool_schema = json!({
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
        });

        let request_body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": system_prompt,
            "messages": [
                { "role": "user", "content": user_input }
            ],
            "tools": [
                {
                    "name": "generate_script",
                    "description": "根据用户需求生成可执行脚本及其参数声明",
                    "input_schema": tool_schema
                }
            ],
            "tool_choice": {
                "type": "tool",
                "name": "generate_script"
            }
        });

        let url = format!("{}/v1/messages", self.api_base.trim_end_matches('/'));

        let response = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            anyhow::bail!("LLM API 错误 ({}): {}", status, response_text);
        }

        let parsed: serde_json::Value = serde_json::from_str(&response_text)?;

        // Extract tool_use from response
        let content = parsed
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| anyhow::anyhow!("LLM 响应格式错误: 缺少 content 字段"))?;

        let tool_use = content
            .iter()
            .find(|item| item.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
            .ok_or_else(|| anyhow::anyhow!("LLM 未返回 tool_use，响应: {}", response_text))?;

        let input = tool_use
            .get("input")
            .ok_or_else(|| anyhow::anyhow!("tool_use 缺少 input 字段"))?;

        let generated: GeneratedScript = serde_json::from_value(input.clone())?;
        Ok(generated)
    }
}

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
