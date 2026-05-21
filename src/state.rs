// 状态机模块
// 定义主窗口状态和任务状态

use crate::script::Script;
use std::collections::HashMap;

/// 主窗口状态枚举
/// 描述用户在主界面中的交互阶段
#[derive(Debug, Clone, PartialEq)]
pub enum MainWindowState {
    Idle,     // 空闲（窗口隐藏）

    /// 用户在输入框输入文字
    Input {
        text: String,
    },

    /// 正在进行语义匹配搜索
    Matching,

    /// 别名命中，直接定位到脚本
    AliasHit {
        script: Script,
    },

    /// AI 正在生成脚本
    Generating,

    /// 脚本审阅状态，等待用户确认
    Reviewing {
        script: Script,
    },

    /// 参数填写状态
    ParamInput {
        script: Script,
        params: HashMap<String, String>,
    },
}

impl Default for MainWindowState {
    fn default() -> Self {
        MainWindowState::Idle
    }
}

/// 任务执行状态枚举
/// 描述单个脚本任务的执行阶段
#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    /// 等待中（超过最大并发数时）
    Queued {
        position: usize,
    },

    /// 正在运行
    Running {
        start_time: std::time::Instant,
    },

    /// 执行成功
    Success {
        exit_code: i32,
        end_time: std::time::Instant,
    },

    /// 执行失败
    Failed {
        exit_code: i32,
        stderr: String,
    },

    /// AI 修复中
    Repairing {
        attempt: u8,
    },

    /// 用户取消
    Cancelled,
}

/// 任务信息
/// UI 中显示的任务卡片数据
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub task_id: String,         // 任务 ID
    pub script_id: String,        // 关联的脚本 ID
    pub script_name: String,      // 脚本名称（用于显示）
    pub state: TaskState,         // 当前状态
    pub stdout_lines: Vec<String>, // 输出的行（用于滚动显示）
}

/// 应用状态
/// 持有窗口状态和任务列表
#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub window_state: MainWindowState,          // 主窗口状态
    pub tasks: Vec<TaskInfo>,                  // 任务列表
    pub status_message: String,                 // 状态消息（显示在 UI 底部）
    pub candidate_scripts: Vec<Script>,         // 匹配的候选脚本
}
