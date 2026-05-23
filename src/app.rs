// 应用核心模块
// 持有所有业务模块的引用，协调各子系统的工作

use crate::agent::db::Database;
use crate::agent::{AgentClient, CliTool};
use crate::task_manager::script::{ExecutionContext, Script};
use crate::settings::{AppSettings, SettingsManager};
use crate::state::AppState;
use crate::task_manager::TaskManager;
use crate::utils;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 应用主结构
pub struct App {
    pub state: Arc<Mutex<AppState>>,
    pub db: Arc<Mutex<Database>>,
    pub settings_manager: SettingsManager,
    pub settings: AppSettings,
    pub task_manager: TaskManager,
    pub agent_client: Option<AgentClient>,
    pub uv_available: bool,
}

impl App {
    /// 创建应用实例
    pub fn new() -> Result<Self> {
        utils::ensure_dirs()?;

        let db = Arc::new(Mutex::new(Database::new(None)?));

        let settings_manager = SettingsManager::new();
        let settings = AppSettings::load(&settings_manager).unwrap_or_default();

        let task_manager = TaskManager::new();

        let uv_status = utils::check_uv();
        let uv_available = matches!(uv_status, utils::UvStatus::Available(_));

        let agent_client = if settings.ai.api_key.is_empty() {
            tracing::warn!("API key is empty, skipping AgentClient creation");
            None
        } else {
            tracing::info!("Creating AgentClient with API key length: {}", settings.ai.api_key.len());
            tracing::info!("API key first 8 chars: {}", &settings.ai.api_key[..8.min(settings.ai.api_key.len())]);
            match AgentClient::new(
                settings.ai.api_key.clone(),
                Some(settings.ai.model.clone()),
                settings.ai.api_base.clone(),
            ) {
                Ok(client) => {
                    tracing::info!("AgentClient created successfully");
                    Some(client)
                }
                Err(e) => {
                    tracing::error!("Failed to create AgentClient: {}", e);
                    None
                }
            }
        };

        Ok(Self {
            state: Arc::new(Mutex::new(AppState::default())),
            db: db.clone(),
            settings_manager,
            settings,
            task_manager,
            agent_client,
            uv_available,
        })
    }

    /// 初始化工具索引（后台执行）
    /// 在应用启动后调用，异步索引本地命令行工具
    pub async fn init_tool_indexing(self: &Arc<Self>) {
        let agent = match &self.agent_client {
            Some(a) => a.clone(),
            None => return,
        };

        let db = self.db.clone();

        tokio::spawn(async move {
            match crate::agent::env_familiar::index_tools(db, &agent).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("Indexed {} new tools", count);
                    }
                }
                Err(e) => {
                    tracing::warn!("Tool indexing failed: {}", e);
                }
            }
        });
    }

    /// 搜索相关工具（RAG）
    /// 根据用户输入搜索相关的命令行工具
    pub async fn search_related_tools(&self, user_input: &str) -> Result<Vec<CliTool>> {
        let db = self.db.lock().await;
        let embeddings = db.get_all_tool_embeddings()?;
        drop(db);

        if embeddings.is_empty() {
            return Ok(Vec::new());
        }

        let results = crate::agent::search_tools(user_input, &embeddings, 5);

        let mut tools = Vec::new();
        for (name, desc, _) in results {
            let db = self.db.lock().await;
            if let Some(cache) = db.get_tool_cache(&name)? {
                tools.push(CliTool {
                    name,
                    description: desc,
                    version: Some(cache.version),
                    help_text: cache.help_text,
                    usage_guide: cache.usage_guide,
                    embedding: cache.embedding,
                });
            }
        }

        Ok(tools)
    }

    /// 获取工具使用指南
    pub async fn get_tool_guides(&self, tool_names: &[String]) -> Result<String> {
        if tool_names.is_empty() {
            return Ok(String::new());
        }

        let db = self.db.lock().await;
        let mut guides = Vec::new();

        for name in tool_names {
            if let Some(cache) = db.get_tool_cache(name)? {
                if let Some(guide) = &cache.usage_guide {
                    if !guide.is_empty() {
                        guides.push(format!("## {}\n{}", name, guide));
                    }
                }
            }
        }

        Ok(guides.join("\n\n"))
    }

    /// 生成脚本
    /// 使用 RAG 搜索相关工具，注入使用指南到 prompt
    pub async fn generate_script(&self, user_input: &str) -> Result<Script> {
        // 详细检查配置
        let api_key = &self.settings.ai.api_key;
        let api_key_env = std::env::var("JITA_API_KEY").ok();

        tracing::info!("Checking API key config:");
        tracing::info!("  - From settings: {}", if api_key.is_empty() { "EMPTY" } else { "SET" });
        tracing::info!("  - From env (JITA_API_KEY): {}", if api_key_env.is_some() { "SET" } else { "NOT SET" });

        // 如果设置了环境变量，优先使用
        let effective_api_key = api_key_env.as_ref().unwrap_or(api_key);

        if effective_api_key.is_empty() {
            return Err(anyhow::anyhow!("没有有效的 API key。请在设置中配置 API key。"));
        }

        // 确保 agent_client 已初始化（如果通过环境变量设置了 key）
        if self.agent_client.is_none() {
            tracing::warn!("AgentClient not initialized but API key is available. Creating client now...");
            // 这里我们不能重新创建 client，因为 App 结构已经固定
            // 返回错误让用户重新启动或检查设置
            return Err(anyhow::anyhow!("Agent 未初始化。请重启应用程序或检查设置中的 API key。"));
        }

        let client = self.agent_client.as_ref().unwrap();

        tracing::debug!("generate_script called with user_input: {}", user_input);

        // RAG: 搜索相关工具
        let related_tools = self.search_related_tools(user_input).await?;
        let tool_names: Vec<String> = related_tools.iter().map(|t| t.name.clone()).collect();
        let tool_guide = self.get_tool_guides(&tool_names).await?;

        // 获取工具摘要
        let db = self.db.lock().await;
        let all_tools = db.list_tool_cache()?;
        drop(db);

        let tools_summary = if all_tools.is_empty() {
            "无可用工具".to_string()
        } else {
            all_tools
                .iter()
                .filter_map(|t| t.ai_summary.as_ref().map(|s| format!("- {}: {}", t.tool_name, s)))
                .collect::<Vec<_>>()
                .join("\n")
        };

        // 构建上下文摘要
        let context = ExecutionContext::new();
        let context_summary = format!(
            "工作目录: {}\n选中文件: {:?}",
            context.cwd.display(),
            context.selected_files
        );

        // 构建 system prompt，注入工具使用指南
        let system_prompt = client.build_system_prompt(&tools_summary, &context_summary, &tool_guide);

        let generated = client.generate_script_with_guides(user_input, &tools_summary, &context_summary, &tool_guide).await?;

        let script = crate::agent::generated_script_to_script(generated)?;

        let db = self.db.lock().await;
        db.insert_script(&script)?;

        Ok(script)
    }

    /// 生成脚本（旧接口，兼容外部调用）
    pub async fn generate_script_legacy(&self, user_input: &str) -> Result<Script> {
        self.generate_script(user_input).await
    }

    /// 执行脚本
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

    /// 记录执行历史
    pub async fn record_execution(
        &self,
        script_id: &str,
        params: serde_json::Value,
        exit_code: Option<i32>,
        stderr_summary: Option<String>,
    ) -> Result<()> {
        let record = crate::task_manager::script::ExecutionRecord {
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

    /// 获取脚本上次执行的参数
    pub async fn get_last_params(&self, script_id: &str) -> Result<Option<serde_json::Value>> {
        let db = self.db.lock().await;
        Ok(db.get_last_execution(script_id)?.map(|r| r.params_used))
    }

    /// 列出所有脚本
    pub async fn list_scripts(&self) -> Result<Vec<Script>> {
        let db = self.db.lock().await;
        db.list_scripts()
    }

    /// 根据别名查找脚本
    pub async fn get_script_by_alias(&self, alias: &str) -> Result<Option<Script>> {
        let db = self.db.lock().await;
        db.get_script_by_alias(alias)
    }
}

