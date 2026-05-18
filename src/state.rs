use crate::script::{ExecutionRecord, ParamDeclaration, Script};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum MainWindowState {
    Idle,
    Input {
        text: String,
    },
    Matching,
    AliasHit {
        script: Script,
    },
    Generating,
    Reviewing {
        script: Script,
    },
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

#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    Queued { position: usize },
    Running {
        start_time: std::time::Instant,
    },
    Success {
        exit_code: i32,
        end_time: std::time::Instant,
    },
    Failed {
        exit_code: i32,
        stderr: String,
    },
    Repairing {
        attempt: u8,
    },
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub task_id: String,
    pub script_id: String,
    pub script_name: String,
    pub state: TaskState,
    pub stdout_lines: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub window_state: MainWindowState,
    pub tasks: Vec<TaskInfo>,
    pub status_message: String,
    pub candidate_scripts: Vec<Script>,
}
