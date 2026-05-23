// 任务管理器模块
// 管理脚本执行的生命周期：启动、停止、状态追踪

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::process::Child;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use crate::task_manager::execution::{OutputLine, ScriptExecutor};
use crate::task_manager::script::{ExecutionContext, Script};

pub mod script;
pub mod execution;

/// 支持 Clone 以便跨线程共享
impl Clone for TaskManager {
    fn clone(&self) -> Self {
        Self {
            executor: ScriptExecutor::new(),
            max_concurrent: self.max_concurrent,
            task_counter: self.task_counter.clone(),
            running_tasks: self.running_tasks.clone(),
        }
    }
}

/// 任务管理器
pub struct TaskManager {
    executor: ScriptExecutor,
    max_concurrent: usize,         // 最大并发数（当前未强制）
    task_counter: Arc<AtomicU64>,   // 原子计数器，用于生成任务 ID
    running_tasks: Arc<Mutex<HashMap<String, RunningTaskHandle>>>, // 运行中的任务
}

/// 运行中任务句柄
struct RunningTaskHandle {
    child: Arc<Mutex<Option<Child>>>,                    // 子进程
    handle: Arc<Mutex<Option<JoinHandle<Result<i32>>>>>, // 等待退出码的任务
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            executor: ScriptExecutor::new(),
            max_concurrent: 3,
            task_counter: Arc::new(AtomicU64::new(0)),
            running_tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 启动新任务
    /// 1. 生成唯一任务 ID
    /// 2. 执行脚本
    /// 3. 存储进程句柄
    pub async fn spawn_task(
        &self,
        script: Script,
        params: HashMap<String, String>,
        context: ExecutionContext,
    ) -> Result<TaskHandle> {
        // 原子递增生成任务 ID
        let task_id = self.task_counter.fetch_add(1, Ordering::SeqCst);
        let task_id_str = format!("task_{}", task_id);

        // 执行脚本
        let (child, rx) = self.executor.execute(&script, &params, &context).await?;
        let child_id = child.id();
        let child_arc = Arc::new(Mutex::new(Some(child)));
        let handle_arc = Arc::new(Mutex::new(None));

        // 克隆句柄用于等待退出
        let task_id_for_removal = task_id_str.clone();
        let running_tasks = self.running_tasks.clone();
        let child_for_handle = child_arc.clone();

        // 启动等待进程退出的任务
        let handle = tokio::spawn(async move {
            let exit_code = {
                let mut child_guard = child_for_handle.lock().await;
                if let Some(ref mut child) = *child_guard {
                    match child.wait().await {
                        Ok(status) => {
                            *child_guard = None;
                            status.code().unwrap_or(-1)
                        }
                        Err(_) => -1,
                    }
                } else {
                    0
                }
            };

            // 任务完成后从运行列表移除
            let mut tasks = running_tasks.lock().await;
            tasks.remove(&task_id_for_removal);

            Ok(exit_code)
        });

        // 存储 handle
        {
            let mut h = handle_arc.lock().await;
            *h = Some(handle);
        }

        // 添加到运行中任务
        {
            let mut tasks = self.running_tasks.lock().await;
            tasks.insert(task_id_str.clone(), RunningTaskHandle {
                child: child_arc,
                handle: handle_arc.clone(),
            });
        }

        Ok(TaskHandle {
            task_id: task_id_str,
            rx,
            handle: handle_arc,
            child_id,
        })
    }

    /// 停止任务（SIGTERM / TerminateProcess）
    pub async fn stop_task(&self, task_id: &str) -> Result<()> {
        let tasks = self.running_tasks.lock().await;
        if let Some(task) = tasks.get(task_id) {
            let mut child = task.child.lock().await;
            if let Some(ref mut c) = *child {
                c.kill().await?;
                *child = None;
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("任务不存在或已结束"))
        }
    }

    /// 检查是否有运行中的任务
    pub async fn has_running_tasks(&self) -> bool {
        let tasks = self.running_tasks.lock().await;
        !tasks.is_empty()
    }
}

/// 任务句柄（返回给调用者）
pub struct TaskHandle {
    pub task_id: String,
    pub rx: mpsc::UnboundedReceiver<OutputLine>, // 输出流接收端
    pub handle: Arc<Mutex<Option<JoinHandle<Result<i32>>>>>, // 退出码等待句柄
    pub child_id: Option<u32>, // 进程 ID（用于调试）
}
