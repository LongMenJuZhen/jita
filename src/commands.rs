// Tauri 命令定义

use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use crate::agent::AgentModule;
use crate::settings::AppSettings;
use crate::task_manager::script::{ParamDeclaration, Script, ScriptRuntime};
use crate::task_manager::execution::OutputLine as TaskOutputLine;
use crate::task_manager::TaskManager;
use serde::{Deserialize, Serialize};

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<Mutex<AgentModule>>,
    pub task_manager: Arc<Mutex<TaskManager>>,
    pub uv_available: bool,
}

/// 脚本参数声明
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamDeclarationDto {
    pub name: String,
    pub label: String,
    pub widget_type: String,
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<String>,
}

impl ParamDeclarationDto {
    pub fn from_param(p: &ParamDeclaration) -> Self {
        let widget_type = match &p.widget {
            crate::task_manager::script::WidgetType::Text { .. } => "text",
            crate::task_manager::script::WidgetType::Secret { .. } => "secret",
            crate::task_manager::script::WidgetType::File { .. } => "file",
            crate::task_manager::script::WidgetType::Directory => "directory",
            crate::task_manager::script::WidgetType::Select { .. } => "select",
            crate::task_manager::script::WidgetType::Number { .. } => "number",
            crate::task_manager::script::WidgetType::Toggle => "toggle",
            crate::task_manager::script::WidgetType::Textarea => "textarea",
        }
        .to_string();

        Self {
            name: p.name.clone(),
            label: p.label.clone(),
            widget_type,
            required: p.required,
            description: p.description.clone(),
            default: p.default.clone(),
        }
    }
}

/// 脚本 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub runtime: String,
    pub shell_target: Option<String>,
    pub params_schema: Vec<ParamDeclarationDto>,
    pub alias: Option<String>,
    pub use_count: i64,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

impl ScriptDto {
    pub fn from_script(s: &Script) -> Self {
        Self {
            id: s.id.clone(),
            name: s.name.clone(),
            description: s.description.clone(),
            content: s.content.clone(),
            runtime: match s.runtime {
                ScriptRuntime::PythonPep723 => "python_pep723".to_string(),
                ScriptRuntime::Shell => "shell".to_string(),
            },
            shell_target: s.shell_target.as_ref().map(|t| match t {
                crate::task_manager::script::ShellTarget::Bash => "bash".to_string(),
                crate::task_manager::script::ShellTarget::Pwsh => "pwsh".to_string(),
                crate::task_manager::script::ShellTarget::Sh => "sh".to_string(),
            }),
            params_schema: s.params_schema.iter().map(ParamDeclarationDto::from_param).collect(),
            alias: s.alias.clone(),
            use_count: s.use_count,
            created_at: s.created_at.to_rfc3339(),
            last_used_at: s.last_used_at.map(|t| t.to_rfc3339()),
        }
    }
}

/// 生成结果
#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateResult {
    pub success: bool,
    pub script: Option<ScriptDto>,
    pub error: Option<String>,
}

/// 执行结果
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub success: bool,
    pub task_id: Option<String>,
    pub error: Option<String>,
}

/// 输出行
#[derive(Debug, Clone, Serialize)]
pub struct OutputLine {
    pub line_type: String,
    pub content: String,
}

/// 任务完成事件
#[derive(Debug, Clone, Serialize)]
pub struct TaskComplete {
    pub task_id: String,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

/// 设置 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub api_key: String,
    pub api_base: Option<String>,
    pub model: String,
    pub hotkey: String,
    pub asr_enabled: bool,
    pub asr_model_path: Option<String>,
}

impl From<&AppSettings> for SettingsDto {
    fn from(s: &AppSettings) -> Self {
        Self {
            api_key: s.ai.api_key.clone(),
            api_base: s.ai.api_base.clone(),
            model: s.ai.model.clone(),
            hotkey: s.hotkey.clone(),
            asr_enabled: s.asr_enabled,
            asr_model_path: s.asr_model_path.clone(),
        }
    }
}

#[tauri::command]
pub async fn generate_script(
    state: State<'_, AppState>,
    text: String,
) -> Result<GenerateResult, String> {
    let agent = state.agent.lock().await;

    match agent.generate_script(&text).await {
        Ok(script) => Ok(GenerateResult {
            success: true,
            script: Some(ScriptDto::from_script(&script)),
            error: None,
        }),
        Err(e) => Ok(GenerateResult {
            success: false,
            script: None,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
pub async fn execute_script(
    app: AppHandle,
    state: State<'_, AppState>,
    script: ScriptDto,
    params: HashMap<String, String>,
) -> Result<ExecuteResult, String> {
    let runtime = match script.runtime.as_str() {
        "python_pep723" => ScriptRuntime::PythonPep723,
        _ => ScriptRuntime::Shell,
    };

    let shell_target = script.shell_target.as_ref().map(|t| match t.as_str() {
        "bash" => crate::task_manager::script::ShellTarget::Bash,
        "pwsh" => crate::task_manager::script::ShellTarget::Pwsh,
        _ => crate::task_manager::script::ShellTarget::Sh,
    });

    let rust_script = Script {
        id: script.id,
        name: script.name,
        description: script.description,
        content: script.content,
        runtime,
        shell_target,
        params_schema: Vec::new(),
        alias: script.alias,
        use_count: script.use_count,
        created_at: chrono::Utc::now(),
        last_used_at: None,
    };

    let task_manager = state.task_manager.clone();
    let context = crate::task_manager::script::ExecutionContext::new();

    let spawn_result = task_manager.lock().await.spawn_task(rust_script, params, context).await;

    match spawn_result {
        Ok(handle) => {
            let task_id = handle.task_id.clone();
            let task_id_for_event = task_id.clone();
            let mut rx = handle.rx;
            let handle_arc = handle.handle.clone();
            let app_for_events = app.clone();

            tokio::spawn(async move {
                while let Some(line) = rx.recv().await {
                    let output = OutputLine {
                        line_type: match line {
                            TaskOutputLine::Stdout(_) => "stdout",
                            TaskOutputLine::Stderr(_) => "stderr",
                        }
                        .to_string(),
                        content: match line {
                            TaskOutputLine::Stdout(s) => s,
                            TaskOutputLine::Stderr(s) => s,
                        },
                    };
                    let _ = app_for_events.emit("script_output", output);
                }

                let exit_code = {
                    let mut h = handle_arc.lock().await;
                    if let Some(handle) = h.take() {
                        match handle.await {
                            Ok(Ok(code)) => Some(code),
                            _ => None,
                        }
                    } else {
                        None
                    }
                };

                let _ = app_for_events.emit(
                    "task_complete",
                    TaskComplete {
                        task_id: task_id_for_event,
                        exit_code,
                        error: None,
                    },
                );
            });

            Ok(ExecuteResult {
                success: true,
                task_id: Some(task_id),
                error: None,
            })
        }
        Err(e) => Ok(ExecuteResult {
            success: false,
            task_id: None,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
pub async fn stop_script(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
    let task_manager = state.task_manager.lock().await;
    task_manager.stop_task(&task_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsDto, String> {
    let agent = state.agent.lock().await;
    Ok(SettingsDto::from(&agent.settings))
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: SettingsDto,
) -> Result<(), String> {
    let mut agent = state.agent.lock().await;

    agent.settings.ai.api_key = settings.api_key;
    agent.settings.ai.api_base = settings.api_base;
    agent.settings.ai.model = settings.model;

    agent.settings.save().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn open_models_folder() -> Result<(), String> {
    let model_dir = crate::utils::models_dir();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&model_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&model_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&model_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn check_uv() -> bool {
    matches!(crate::utils::check_uv(), crate::utils::UvStatus::Available(_))
}

#[cfg(feature = "asr")]
#[tauri::command]
pub async fn toggle_asr(
    _app: AppHandle,
    _state: State<'_, AppState>,
) -> Result<bool, String> {
    // TODO: 实现 ASR 功能
    Err("ASR 功能暂未实现".to_string())
}

#[cfg(not(feature = "asr"))]
#[tauri::command]
pub async fn toggle_asr(_app: AppHandle, _state: State<'_, AppState>) -> Result<bool, String> {
    Err("ASR 功能未启用，请使用 --features asr 重新编译".to_string())
}