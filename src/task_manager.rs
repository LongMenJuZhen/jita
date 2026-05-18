use crate::execution::{OutputLine, ScriptExecutor};
use crate::script::{ExecutionContext, ExecutionRecord, Script};
use crate::state::{TaskInfo, TaskState};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

pub struct TaskManager {
    executor: ScriptExecutor,
    max_concurrent: usize,
    task_counter: Arc<Mutex<u64>>,
}

pub struct TaskHandle {
    pub task_id: String,
    pub rx: mpsc::UnboundedReceiver<OutputLine>,
    pub handle: JoinHandle<Result<i32>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            executor: ScriptExecutor::new(),
            max_concurrent: 3,
            task_counter: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn spawn_task(
        &self,
        script: Script,
        params: HashMap<String, String>,
        context: ExecutionContext,
    ) -> Result<TaskHandle> {
        let counter = self.task_counter.clone();
        let mut c = counter.lock().await;
        *c += 1;
        let task_id = format!("task_{}", *c);
        drop(c);

        let (child, rx) = self.executor.execute(&script, &params, &context).await?;

        let task_id_clone = task_id.clone();
        let handle = tokio::spawn(async move {
            let output = child.wait_with_output().await?;
            let exit_code = output.status.code().unwrap_or(-1);
            Ok(exit_code)
        });

        Ok(TaskHandle {
            task_id: task_id_clone,
            rx,
            handle,
        })
    }

    pub fn create_task_info(&self,
        task_id: String,
        script: &Script,
    ) -> TaskInfo {
        TaskInfo {
            task_id,
            script_id: script.id.clone(),
            script_name: script.name.clone(),
            state: TaskState::Running {
                start_time: std::time::Instant::now(),
            },
            stdout_lines: Vec::new(),
        }
    }
}
