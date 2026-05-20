// 数据库模块
// 使用 SQLite 存储脚本、执行记录和 uv 工具缓存

use crate::script::{ExecutionRecord, Script, UvToolCache};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::PathBuf;

/// 解析日期时间字符串，解析失败时返回当前时间
fn parse_datetime(s: &str) -> DateTime<Utc> {
    s.parse().unwrap_or_else(|_| Utc::now())
}

/// 数据库封装
/// 提供脚本、执行记录和 uv 工具缓存的 CRUD 操作
pub struct Database {
    conn: Connection,
}

impl Database {
    /// 创建数据库连接
    /// 自动创建数据目录和数据库文件
    pub fn new(db_path: Option<PathBuf>) -> Result<Self> {
        // 默认路径：~/.local/share/jita/jita.db
        let path = db_path.unwrap_or_else(|| {
            let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
            data_dir.join("jita").join("jita.db")
        });

        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// 初始化数据库表结构
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            -- 脚本表：存储用户生成的所有脚本
            CREATE TABLE IF NOT EXISTS scripts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                content TEXT NOT NULL,
                runtime TEXT NOT NULL,
                shell_target TEXT,
                params_schema TEXT NOT NULL,
                alias TEXT UNIQUE,
                use_count INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                last_used_at TEXT,
                embedding BLOB,
                embedding_model TEXT
            );

            -- 执行记录表：存储每次脚本执行的输入输出
            CREATE TABLE IF NOT EXISTS execution_records (
                id TEXT PRIMARY KEY,
                script_id TEXT NOT NULL REFERENCES scripts(id),
                params_used TEXT NOT NULL,
                exit_code INTEGER,
                stderr_summary TEXT,
                executed_at TEXT NOT NULL
            );

            -- UV 工具缓存表：存储本地 uv tool 安装的工具
            CREATE TABLE IF NOT EXISTS uv_tool_cache (
                tool_name TEXT PRIMARY KEY,
                version TEXT NOT NULL,
                help_text TEXT NOT NULL,
                ai_summary TEXT,
                cached_at TEXT NOT NULL
            );

            -- 索引
            CREATE INDEX IF NOT EXISTS idx_scripts_alias ON scripts(alias);
            CREATE INDEX IF NOT EXISTS idx_exec_script_id ON execution_records(script_id);
            "#,
        )?;
        Ok(())
    }

    // =====================
    // 脚本 CRUD
    // =====================

    /// 插入新脚本
    pub fn insert_script(&self, script: &Script) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO scripts (id, name, description, content, runtime, shell_target,
                params_schema, alias, use_count, created_at, last_used_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                &script.id,
                &script.name,
                &script.description,
                &script.content,
                serde_json::to_string(&script.runtime)?,
                script.shell_target.as_ref().map(|s| serde_json::to_string(s).unwrap()),
                serde_json::to_string(&script.params_schema)?,
                &script.alias,
                script.use_count,
                script.created_at.to_rfc3339(),
                script.last_used_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    /// 根据 ID 获取脚本
    pub fn get_script(&self, id: &str) -> Result<Option<Script>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM scripts WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(self.row_to_script(row)?))
        } else {
            Ok(None)
        }
    }

    /// 根据别名获取脚本
    pub fn get_script_by_alias(&self, alias: &str) -> Result<Option<Script>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM scripts WHERE alias = ?1")?;
        let mut rows = stmt.query(params![alias])?;

        if let Some(row) = rows.next()? {
            Ok(Some(self.row_to_script(row)?))
        } else {
            Ok(None)
        }
    }

    /// 列出所有脚本（按上次使用时间排序）
    pub fn list_scripts(&self) -> Result<Vec<Script>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM scripts ORDER BY last_used_at DESC NULLS LAST, created_at DESC")?;
        let rows = stmt.query_map([], |row| self.row_to_script(row))?;
        rows.collect::<Result<_, rusqlite::Error>>()
            .map_err(|e| e.into())
    }

    /// 更新脚本
    pub fn update_script(&self, script: &Script) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE scripts SET
                name = ?2, description = ?3, content = ?4, runtime = ?5,
                shell_target = ?6, params_schema = ?7, alias = ?8,
                use_count = ?9, last_used_at = ?10
            WHERE id = ?1
            "#,
            params![
                &script.id,
                &script.name,
                &script.description,
                &script.content,
                serde_json::to_string(&script.runtime)?,
                script.shell_target.as_ref().map(|s| serde_json::to_string(s).unwrap()),
                serde_json::to_string(&script.params_schema)?,
                &script.alias,
                script.use_count,
                script.last_used_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    /// 删除脚本
    pub fn delete_script(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM scripts WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// 增加使用次数
    pub fn increment_use_count(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE scripts SET use_count = use_count + 1, last_used_at = ?2 WHERE id = ?1",
            params![id, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    // =====================
    // 执行记录
    // =====================

    /// 插入执行记录
    pub fn insert_execution(&self, record: &ExecutionRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO execution_records (id, script_id, params_used, exit_code, stderr_summary, executed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                &record.id,
                &record.script_id,
                record.params_used.to_string(),
                record.exit_code,
                &record.stderr_summary,
                record.executed_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// 获取脚本最近一次执行记录（用于参数预填）
    pub fn get_last_execution(&self, script_id: &str) -> Result<Option<ExecutionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM execution_records WHERE script_id = ?1 ORDER BY executed_at DESC LIMIT 1")?;
        let mut rows = stmt.query(params![script_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(self.row_to_execution_record(row)?))
        } else {
            Ok(None)
        }
    }

    /// 列出脚本所有执行记录
    pub fn list_executions(&self, script_id: &str) -> Result<Vec<ExecutionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM execution_records WHERE script_id = ?1 ORDER BY executed_at DESC")?;
        let rows = stmt.query_map(params![script_id], |row| self.row_to_execution_record(row))?;
        rows.collect::<Result<_, rusqlite::Error>>()
            .map_err(|e| e.into())
    }

    // =====================
    // UV 工具缓存
    // =====================

    /// 插入或更新 uv 工具
    pub fn insert_uv_tool(&self, tool: &UvToolCache) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO uv_tool_cache (tool_name, version, help_text, ai_summary, cached_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(tool_name) DO UPDATE SET
                version = ?2, help_text = ?3, ai_summary = ?4, cached_at = ?5
            "#,
            params![
                &tool.tool_name,
                &tool.version,
                &tool.help_text,
                &tool.ai_summary,
                tool.cached_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// 列出所有 uv 工具
    pub fn list_uv_tools(&self) -> Result<Vec<UvToolCache>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM uv_tool_cache ORDER BY tool_name")?;
        let rows = stmt.query_map([], |row| self.row_to_uv_tool(row))?;
        rows.collect::<Result<_, rusqlite::Error>>()
            .map_err(|e| e.into())
    }

    // =====================
    // 行转换辅助函数
    // =====================

    /// 将数据库行转换为 Script 对象
    fn row_to_script(&self, row: &rusqlite::Row) -> Result<Script, rusqlite::Error> {
        use crate::script::{ParamDeclaration, ScriptRuntime, ShellTarget};

        // 解析 JSON 字段
        let runtime_str: String = row.get("runtime")?;
        let runtime: ScriptRuntime = serde_json::from_str(&runtime_str)
            .unwrap_or(ScriptRuntime::Shell);

        let shell_target: Option<ShellTarget> = row.get::<_, Option<String>>("shell_target")?
            .and_then(|s| serde_json::from_str(&s).ok());

        let params_schema_str: String = row.get("params_schema")?;
        let params_schema: Vec<ParamDeclaration> = serde_json::from_str(&params_schema_str)
            .unwrap_or_default();

        // 解析时间字段
        Ok(Script {
            id: row.get("id")?,
            name: row.get("name")?,
            description: row.get("description")?,
            content: row.get("content")?,
            runtime,
            shell_target,
            params_schema,
            alias: row.get("alias")?,
            use_count: row.get("use_count")?,
            created_at: row.get::<_, String>("created_at")?
                .parse::<DateTime<Utc>>()
                .map_or_else(|_| Utc::now(), |dt| dt),
            last_used_at: row.get::<_, Option<String>>("last_used_at")?
                .and_then(|s| s.parse::<DateTime<Utc>>().ok()),
        })
    }

    /// 将数据库行转换为 ExecutionRecord 对象
    fn row_to_execution_record(
        &self,
        row: &rusqlite::Row,
    ) -> Result<ExecutionRecord, rusqlite::Error> {
        let params_str: String = row.get("params_used")?;
        let params_used: serde_json::Value = serde_json::from_str(&params_str)
            .unwrap_or(serde_json::Value::Null);

        Ok(ExecutionRecord {
            id: row.get("id")?,
            script_id: row.get("script_id")?,
            params_used,
            exit_code: row.get("exit_code")?,
            stderr_summary: row.get("stderr_summary")?,
            executed_at: row.get::<_, String>("executed_at")?
                .parse::<DateTime<Utc>>()
                .map_or_else(|_| Utc::now(), |dt| dt),
        })
    }

    /// 将数据库行转换为 UvToolCache 对象
    fn row_to_uv_tool(&self, row: &rusqlite::Row) -> Result<UvToolCache, rusqlite::Error> {
        Ok(UvToolCache {
            tool_name: row.get("tool_name")?,
            version: row.get("version")?,
            help_text: row.get("help_text")?,
            ai_summary: row.get("ai_summary")?,
            cached_at: row.get::<_, String>("cached_at")?
                .parse::<DateTime<Utc>>()
                .map_or_else(|_| Utc::now(), |dt| dt),
        })
    }
}
