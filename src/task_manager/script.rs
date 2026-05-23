// 数据模型模块
// 定义脚本、执行记录、参数声明等核心数据结构

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 脚本运行时类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScriptRuntime {
    PythonPep723, // Python 脚本，使用 PEP 723 格式声明依赖
    Shell,       // Shell 脚本（bash/pwsh/sh）
}

/// Shell 目标解释器
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShellTarget {
    Bash, // Unix 默认 shell
    Pwsh, // Windows PowerShell 7+
    Sh,   // 最小兼容 shell
}

impl Default for ShellTarget {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        return ShellTarget::Pwsh;
        #[cfg(not(target_os = "windows"))]
        return ShellTarget::Bash;
    }
}

/// 脚本实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Script {
    pub id: String,                    // 唯一标识符
    pub name: String,                   // 脚本名称
    pub description: String,            // 描述（用于语义搜索）
    pub content: String,                 // 脚本内容
    pub runtime: ScriptRuntime,          // 运行时类型
    pub shell_target: Option<ShellTarget>, // Shell 目标（仅 shell 脚本）
    pub params_schema: Vec<ParamDeclaration>, // 参数声明列表
    pub alias: Option<String>,          // 用户绑定的别名
    pub use_count: i64,                // 使用次数
    pub created_at: chrono::DateTime<chrono::Utc>, // 创建时间
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>, // 上次使用时间
}

impl Script {
    /// 创建新脚本
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

/// 执行记录
/// 记录每次脚本执行的输入输出摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: String,                      // 唯一标识符
    pub script_id: String,               // 关联的脚本 ID
    pub params_used: serde_json::Value,  // 使用的参数（JSON）
    pub exit_code: Option<i32>,          // 退出码
    pub stderr_summary: Option<String>,   // stderr 前 2000 字符摘要
    pub executed_at: chrono::DateTime<chrono::Utc>, // 执行时间
}

/// UV 工具缓存
/// 存储本地 uv tool install 安装的工具信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UvToolCache {
    pub tool_name: String,   // 工具名称
    pub version: String,    // 版本号
    pub help_text: String,   // --help 输出
    pub ai_summary: Option<String>, // AI 生成的摘要
    pub cached_at: chrono::DateTime<chrono::Utc>, // 缓存时间
}

/// 全局设置（keychain 键值对）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSetting {
    pub key: String,         // 键名
    pub description: String,  // 说明
}

/// UI 控件类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WidgetType {
    Text { placeholder: Option<String> },           // 普通文本输入
    Secret { global_key: Option<String> },          // 密码框（可关联全局设置）
    File { filter: Vec<String>, multiple: bool },   // 文件选择器
    Directory,                                      // 目录选择器
    Select { options: Vec<String> },               // 下拉选择
    Number { min: Option<f64>, max: Option<f64> }, // 数字输入
    Toggle,                                        // 开关
    Textarea,                                       // 多行文本
}

/// 参数声明
/// AI 生成的参数元信息，用于渲染表单
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParamDeclaration {
    pub name: String,          // 环境变量名（大写下划线）
    pub label: String,         // 界面显示标签
    pub widget: WidgetType,     // 控件类型
    pub required: bool,        // 是否必填
    pub description: Option<String>, // 说明文本
    pub default: Option<String>,   // 默认值
}

/// 执行上下文
/// 脚本执行时的环境信息（工作目录、选中的文件等）
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub cwd: PathBuf,                              // 工作目录
    pub selected_files: Vec<PathBuf>,              // 选中的文件列表
    pub clipboard_path: Option<PathBuf>,            // 剪贴板中的文件路径
    pub env_vars: std::collections::HashMap<String, String>, // 环境变量
}

impl ExecutionContext {
    /// 创建新的执行上下文
    pub fn new() -> Self {
        Self {
            cwd: dirs::home_dir().unwrap_or_else(|| PathBuf::from("../..")),
            selected_files: Vec::new(),
            clipboard_path: None,
            env_vars: std::collections::HashMap::new(),
        }
    }

    /// 根据控件类型推断默认值
    /// 例如：File 控件自动填入选中的第一个文件
    pub fn infer_default(&self, widget: &WidgetType) -> Option<String> {
        match widget {
            // 如果有选中的文件，File 控件默认填第一个
            WidgetType::File { .. } if !self.selected_files.is_empty() => {
                self.selected_files[0].to_str().map(|s| s.to_string())
            }
            // 如果工作目录不是 home，Directory 控件填当前目录
            WidgetType::Directory if self.cwd != dirs::home_dir().unwrap_or_default() => {
                self.cwd.to_str().map(|s| s.to_string())
            }
            _ => None,
        }
    }
}
