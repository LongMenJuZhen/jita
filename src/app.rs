use crate::db::Database;
use crate::llm::{GeneratedScript, LlmClient};
use crate::script::{ExecutionContext, Script};
use crate::settings::{AppSettings, SettingsManager};
use crate::state::AppState;
use crate::task_manager::TaskManager;
use crate::utils;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct App {
    pub state: Arc<Mutex<AppState>>,
    pub db: Arc<tokio::sync::Mutex<Database>>,
    pub settings_manager: SettingsManager,
    pub settings: AppSettings,
    pub task_manager: TaskManager,
    pub llm_client: Option<LlmClient>,
    pub uv_available: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        utils::ensure_dirs()?;
        let db = Arc::new(tokio::sync::Mutex::new(Database::new(None)?));
        let settings_manager = SettingsManager::new();
        let settings = AppSettings::load(&settings_manager).unwrap_or_default();
        let task_manager = TaskManager::new();

        let uv_status = utils::check_uv();
        let uv_available = matches!(uv_status, utils::UvStatus::Available(_));

        let llm_client = if settings.ai.api_key.is_empty() {
            None
        } else {
            Some(LlmClient::new(
                settings.ai.api_key.clone(),
                settings.ai.model.clone(),
                settings.ai.api_base.clone(),
            ))
        };

        Ok(Self {
            state: Arc::new(Mutex::new(AppState::default())),
            db,
            settings_manager,
            settings,
            task_manager,
            llm_client,
            uv_available,
        })
    }

    pub async fn generate_script(&self, user_input: &str) -> Result<Script> {
        let client = self
            .llm_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;

        let uv_summary = {
            let db = self.db.lock().await;
            let uv_tools = db.list_uv_tools()?;
            uv_tools
                .iter()
                .filter_map(|t| t.ai_summary.as_ref().map(|s| format!("{}: {}", t.tool_name, s)))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let context = ExecutionContext::new();
        let context_summary = format!(
            "工作目录: {}\n选中文件: {:?}",
            context.cwd.display(),
            context.selected_files
        );

        let system_prompt = client.build_system_prompt(&uv_summary, &context_summary);
        let generated = client.generate_script(user_input, &system_prompt).await?;
        let script = crate::llm::generated_script_to_script(generated)?;

        {
            let db = self.db.lock().await;
            db.insert_script(&script)?;
        }

        Ok(script)
    }

    pub async fn execute_script(
        &self,
        script: Script,
        params: HashMap<String, String>,
    ) -> Result<crate::task_manager::TaskHandle> {
        let context = ExecutionContext::new();
        self.task_manager
            .spawn_task(script.clone(), params, context)
            .await
    }

    pub async fn record_execution(
        &self,
        script_id: &str,
        params: serde_json::Value,
        exit_code: Option<i32>,
        stderr_summary: Option<String>,
    ) -> Result<()> {
        let record = crate::script::ExecutionRecord {
            id: uuid::Uuid::new_v4().to_string(),
            script_id: script_id.to_string(),
            params_used: params,
            exit_code,
            stderr_summary,
            executed_at: chrono::Utc::now(),
        };
        let db = self.db.lock().await;
        db.insert_execution(&record)?;
        db.increment_use_count(script_id)?;
        Ok(())
    }

    pub async fn get_last_params(&self, script_id: &str) -> Result<Option<serde_json::Value>> {
        let db = self.db.lock().await;
        Ok(db
            .get_last_execution(script_id)?
            .map(|r| r.params_used))
    }

    pub async fn list_scripts(&self) -> Result<Vec<Script>> {
        let db = self.db.lock().await;
        db.list_scripts()
    }

    pub async fn get_script_by_alias(&self, alias: &str) -> Result<Option<Script>> {
        let db = self.db.lock().await;
        db.get_script_by_alias(alias)
    }
}
