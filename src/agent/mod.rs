// Agent 模块
// 封装 LLM 交互、RAG 搜索、数据库访问

mod client_impl;

pub use client_impl::AgentClient;
pub use client_impl::CliTool;
pub use client_impl::RepairRequest;
pub use client_impl::generated_script_to_script;
pub use client_impl::search_tools;

pub mod db;
pub mod embedding;
pub mod env_familiar;

use crate::settings::AppSettings;
use crate::task_manager::script::{ExecutionContext, Script};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Agent 模块
/// 持有 LLM 客户端和数据库，提供脚本生成和管理功能
pub struct AgentModule {
    pub client: Option<AgentClient>,
    pub db: Arc<Mutex<db::Database>>,
    pub settings: AppSettings,
}

impl AgentModule {
    /// 创建 Agent 模块
    pub fn new(settings: AppSettings) -> Result<Self> {
        let db = Arc::new(Mutex::new(db::Database::new(None)?));

        let client = if settings.ai.api_key.is_empty() {
            tracing::warn!("API key is empty, skipping AgentClient creation");
            None
        } else {
            match AgentClient::new(
                settings.ai.api_key.clone(),
                Some(settings.ai.model.clone()),
                settings.ai.api_base.clone(),
            ) {
                Ok(c) => {
                    tracing::info!("AgentClient created successfully");
                    Some(c)
                }
                Err(e) => {
                    tracing::error!("Failed to create AgentClient: {}", e);
                    None
                }
            }
        };

        Ok(Self {
            client,
            db,
            settings,
        })
    }

    /// 初始化工具索引（后台执行）
    pub async fn init_tool_indexing(self: &Arc<Self>) {
        let agent = match &self.client {
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
    pub async fn search_related_tools(&self, user_input: &str) -> Result<Vec<CliTool>> {
        let db = self.db.lock().await;
        let embeddings = db.get_all_tool_embeddings()?;
        drop(db);

        if embeddings.is_empty() {
            return Ok(Vec::new());
        }

        let results = search_tools(user_input, &embeddings, 5);

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
    pub async fn generate_script(&self, user_input: &str) -> Result<Script> {
        let api_key = &self.settings.ai.api_key;
        let api_key_env = std::env::var("JITA_API_KEY").ok();

        let effective_api_key = api_key_env.as_ref().unwrap_or(api_key);

        if effective_api_key.is_empty() {
            return Err(anyhow::anyhow!("没有有效的 API key。请在设置中配置 API key。"));
        }

        if self.client.is_none() {
            return Err(anyhow::anyhow!("Agent 未初始化。请重启应用程序或检查设置中的 API key。"));
        }

        let client = self.client.as_ref().unwrap();

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

        // 生成脚本
        let generated = client
            .generate_script_with_guides(user_input, &tools_summary, &context_summary, &tool_guide)
            .await?;

        let script = generated_script_to_script(generated)?;

        let db = self.db.lock().await;
        db.insert_script(&script)?;

        Ok(script)
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

    /// 修复失败的脚本
    pub async fn repair_script(&self, request: crate::agent::RepairRequest) -> Result<Script> {
        let client = self.client.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Agent 未初始化")
        })?;

        let generated = client.repair_script(request).await?;
        crate::agent::generated_script_to_script(generated)
    }
}

impl Clone for AgentModule {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            db: self.db.clone(),
            settings: self.settings.clone(),
        }
    }
}