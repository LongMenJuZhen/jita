// Deno 进程管理模块
// 通过 stdin/stdout JSON-RPC 与 Deno 后端通信

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::process::ChildStdin;

/// JSON-RPC 响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
}

/// Deno 发出的事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenoEmit {
    pub method: String,
    pub params: DenoEmitParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenoEmitParams {
    pub event: String,
    pub payload: serde_json::Value,
}

/// Deno 进程状态
pub struct DenoProcessState {
    child: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    pending: Arc<Mutex<HashMap<usize, oneshot::Sender<Result<serde_json::Value>>>>>,
    next_id: Arc<Mutex<usize>>,
}

impl DenoProcessState {
    pub fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(0)),
        }
    }
}

/// 启动 Deno 进程
pub async fn start_deno(
    app: AppHandle,
    deno_path: &str,
    script_path: &str,
) -> Result<()> {
    let state = app.state::<DenoProcessState>();

    // 启动 Deno 进程
    let mut child = Command::new(deno_path)
        .arg("run")
        .arg("-A")
        .arg(script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stdin = child.stdin.take().unwrap();

    // 保存进程句柄
    {
        let mut c = state.child.lock().await;
        *c = Some(child);
    }
    {
        let mut s = state.stdin.lock().await;
        *s = Some(stdin);
    }

    // 启动 stdout 读取任务
    let app_for_events = app.clone();
    let pending = state.pending.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // 尝试解析为事件
            if let Ok(emit) = serde_json::from_str::<DenoEmit>(&line) {
                let _ = app_for_events.emit(&emit.params.event, emit.params.payload);
            } else if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&line) {
                // 处理响应
                if let (serde_json::Value::Number(id_num), Some(result)) = (resp.id.clone(), resp.result) {
                    let id_val = id_num.as_u64().unwrap_or(0) as usize;
                    if let Ok(result_val) = serde_json::from_value(result.clone()) {
                        let mut p = pending.lock().await;
                        if let Some(tx) = p.remove(&id_val) {
                            let _ = tx.send(Ok(result_val));
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

/// 发送请求到 Deno 并等待响应
pub async fn send_request(
    app: &AppHandle,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let state = app.state::<DenoProcessState>();

    let id = {
        let mut next = state.next_id.lock().await;
        let id = *next;
        *next += 1;
        id
    };

    let (tx, rx) = oneshot::channel();
    {
        let mut pending = state.pending.lock().await;
        pending.insert(id, tx);
    }

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    });

    // 发送请求
    {
        let mut stdin = state.stdin.lock().await;
        if let Some(ref mut s) = *stdin {
            use tokio::io::AsyncWriteExt;
            s.write_all(format!("{}\n", request).as_bytes()).await?;
            s.flush().await?;
        }
    }

    let result = rx.await??;
    Ok(result)
}

/// Tauri invoke 命令 - 转发到 Deno
#[tauri::command]
pub async fn deno_invoke(
    app: AppHandle,
    method: String,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    send_request(&app, &method, params)
        .await
        .map_err(|e| e.to_string())
}

/// 停止 Deno 进程
pub async fn stop_deno(app: &AppHandle) -> Result<()> {
    let state = app.state::<DenoProcessState>();

    let mut child = state.child.lock().await;
    if let Some(ref mut c) = *child {
        c.kill().await?;
        *child = None;
    }

    let mut stdin = state.stdin.lock().await;
    *stdin = None;

    Ok(())
}