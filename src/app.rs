// 应用核心模块
// 持有所有业务模块的引用，协调各子系统的工作

use crate::db::Database;                    // SQLite 数据库
use crate::llm::LlmClient; // LLM 客户端
use crate::script::{ExecutionContext, Script}; // 数据模型
use crate::settings::{AppSettings, SettingsManager}; // 全局设置
use crate::state::AppState;                  // 应用状态
use crate::task_manager::TaskManager;         // 任务管理器
use crate::utils;                           // 工具函数
use anyhow::Result;                          // 错误处理
use std::collections::HashMap;               // 哈希映射
use std::sync::Arc;                         // 原子引用计数
use tokio::sync::Mutex;                     // 异步互斥锁

/// 应用主结构
/// 持有所有业务模块的引用，通过 Arc<Mutex> 支持跨线程共享
pub struct App {
    pub state: Arc<Mutex<AppState>>,              // 应用状态（窗口状态、任务列表等）
    pub db: Arc<tokio::sync::Mutex<Database>>,    // 数据库连接
    pub settings_manager: SettingsManager,          // 设置管理器（keyring 操作）
    pub settings: AppSettings,                    // 当前配置（内存缓存）
    pub task_manager: TaskManager,                // 任务管理器（子进程控制）
    pub llm_client: Option<LlmClient>,            // LLM 客户端（可能未配置）
    pub uv_available: bool,                       // uv 是否可用
}

impl App {
    /// 创建应用实例
    /// 初始化数据库、加载配置、检测外部依赖
    pub fn new() -> Result<Self> {
        // 确保数据目录存在
        utils::ensure_dirs()?;

        // 初始化数据库
        let db = Arc::new(tokio::sync::Mutex::new(Database::new(None)?));

        // 创建设置管理器
        let settings_manager = SettingsManager::new();

        // 从 keyring 加载配置
        let settings = AppSettings::load(&settings_manager).unwrap_or_default();

        // 创建任务管理器
        let task_manager = TaskManager::new();

        // 检测 uv 是否安装
        let uv_status = utils::check_uv();
        let uv_available = matches!(uv_status, utils::UvStatus::Available(_));

        // 如果配置了 API key，创建 LLM 客户端
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

    /// 调用 AI 生成脚本
    /// 1. 收集 uv 工具摘要作为上下文
    /// 2. 构建 system prompt
    /// 3. 调用 LLM API
    /// 4. 解析结构化输出
    /// 5. 存入数据库
    pub async fn generate_script(&self, user_input: &str) -> Result<Script> {
        // 获取 LLM 客户端
        let client = self
            .llm_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;

        // 收集 uv 工具和执行上下文（在锁内完成数据库查询）
        let (uv_summary, context) = {
            let db = self.db.lock().await;
            let uv_tools = db.list_uv_tools()?;
            // 将工具列表格式化为一行一个
            let uv_summary = uv_tools
                .iter()
                .filter_map(|t| t.ai_summary.as_ref().map(|s| format!("{}: {}", t.tool_name, s)))
                .collect::<Vec<_>>()
                .join("\n");
            // 创建执行上下文
            let context = ExecutionContext::new();
            (uv_summary, context)
        };

        // 格式化上下文摘要
        let context_summary = format!(
            "工作目录: {}\n选中文件: {:?}",
            context.cwd.display(),
            context.selected_files
        );

        // 构建提示词并调用 LLM
        let system_prompt = client.build_system_prompt(&uv_summary, &context_summary);
        let generated = client.generate_script(user_input, &system_prompt).await?;

        // 转换为脚本对象
        let script = crate::llm::generated_script_to_script(generated)?;

        // 存入数据库
        {
            let db = self.db.lock().await;
            db.insert_script(&script)?;
        }

        Ok(script)
    }

    /// 执行脚本
    /// 创建任务并启动子进程
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
    /// 将执行结果写入数据库，并更新脚本使用次数
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

    /// 获取脚本上次执行的参数（用于预填）
    pub async fn get_last_params(&self, script_id: &str) -> Result<Option<serde_json::Value>> {
        let db = self.db.lock().await;
        Ok(db
            .get_last_execution(script_id)?
            .map(|r| r.params_used))
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
