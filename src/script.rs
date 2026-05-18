use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScriptRuntime {
    PythonPep723,
    Shell,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShellTarget {
    Bash,
    Pwsh,
    Sh,
}

impl Default for ShellTarget {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        return ShellTarget::Pwsh;
        #[cfg(not(target_os = "windows"))]
        return ShellTarget::Bash;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Script {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub runtime: ScriptRuntime,
    pub shell_target: Option<ShellTarget>,
    pub params_schema: Vec<ParamDeclaration>,
    pub alias: Option<String>,
    pub use_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Script {
    pub fn new(name: String, description: String, content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            content,
            runtime: ScriptRuntime::Shell,
            shell_target: None,
            params_schema: Vec::new(),
            alias: None,
            use_count: 0,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: String,
    pub script_id: String,
    pub params_used: serde_json::Value,
    pub exit_code: Option<i32>,
    pub stderr_summary: Option<String>,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UvToolCache {
    pub tool_name: String,
    pub version: String,
    pub help_text: String,
    pub ai_summary: Option<String>,
    pub cached_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSetting {
    pub key: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WidgetType {
    Text { placeholder: Option<String> },
    Secret { global_key: Option<String> },
    File { filter: Vec<String>, multiple: bool },
    Directory,
    Select { options: Vec<String> },
    Number { min: Option<f64>, max: Option<f64> },
    Toggle,
    Textarea,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParamDeclaration {
    pub name: String,
    pub label: String,
    pub widget: WidgetType,
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub cwd: PathBuf,
    pub selected_files: Vec<PathBuf>,
    pub clipboard_path: Option<PathBuf>,
    pub env_vars: std::collections::HashMap<String, String>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self {
            cwd: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
            selected_files: Vec::new(),
            clipboard_path: None,
            env_vars: std::collections::HashMap::new(),
        }
    }

    pub fn infer_default(&self, widget: &WidgetType) -> Option<String> {
        match widget {
            WidgetType::File { .. } if !self.selected_files.is_empty() => {
                self.selected_files[0].to_str().map(|s| s.to_string())
            }
            WidgetType::Directory if self.cwd != dirs::home_dir().unwrap_or_default() => {
                self.cwd.to_str().map(|s| s.to_string())
            }
            _ => None,
        }
    }
}
