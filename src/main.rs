// Jita 主入口文件
// 负责初始化应用、创建窗口、绑定 Slint 回调

mod agent;        // Agent 模块（预留）
mod app;          // 应用核心逻辑
mod asr;         // 语音识别引擎
mod db;           // SQLite 数据库
mod embedding;    // 向量索引（Phase 2）
mod execution;    // 脚本执行器
mod hotkey;       // 全局快捷键（预留）
mod llm;          // LLM 客户端
mod script;       // 数据模型
mod settings;      // 全局设置
mod state;         // 状态机
mod task_manager;  // 任务管理器
mod tray;          // 系统托盘（预留）
mod i18n;          // 国际化
mod ui;            // Slint UI
mod utils;          // 工具函数

use app::App;                      // 应用核心
use slint::{ComponentHandle, Weak}; // Slint 窗口组件
use std::cell::RefCell;             // 内部可变性（用于 ASR 状态）
use std::sync::Arc;                 // 原子引用计数
use tokio::sync::Mutex;             // 异步互斥锁

fn main() {
    // 初始化 i18n（自动检测 locale 并初始化 Slint 捆绑翻译）
    i18n::init();

    // 创建 tokio 异步运行时
    let runtime = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime");

    // 初始化应用（阻塞等待完成）
    let app = runtime
        .block_on(async { App::new().expect("Failed to initialize app") });

    // 用 Arc<Mutex> 包装以便跨线程共享
    let app = Arc::new(Mutex::new(app));

    // 创建 Slint 窗口
    let window = ui::JitaWindow::new().expect("创建窗口失败");

    // 将设置从应用加载到 UI（窗口创建后立即同步一次）
    {
        let app_guard = runtime.block_on(async { app.lock().await });
        window.set_uv_available(app_guard.uv_available);
        window.set_current_state("input".into());
        window.set_settings_api_key(app_guard.settings.ai.api_key.clone().into());
        window.set_settings_api_base(
            app_guard.settings.ai.api_base.clone().unwrap_or_default().into(),
        );
        window.set_settings_model(app_guard.settings.ai.model.clone().into());
    }

    // ============================================================
    // 回调：用户提交输入
    // ============================================================
    let window_weak_submit = window.as_weak();
    let app_submit = app.clone();
    let rt_submit = runtime.handle().clone();

    window.on_submit_input(move |text: slint::SharedString| {
        let text = text.to_string();
        let weak = window_weak_submit.clone();
        let app = app_submit.clone();
        let rt = rt_submit.clone();

        let weak_clone = weak.clone();
        rt.spawn(async move {
            // 切换到生成状态
            let weak_gen = weak_clone.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_gen.upgrade() {
                    w.set_current_state("generating".into());
                    w.set_status_text(i18n::t("ai_generating").into());
                }
            });

            // 调用 AI 生成脚本
            let result = {
                let app_guard = app.lock().await;
                app_guard.generate_script(&text).await
            };

            match result {
                Ok(script) => {
                    // 提取脚本信息用于 UI 更新
                    let name = script.name.clone();
                    let description = script.description.clone();
                    let content = script.content.clone();
                    let params = script.params_schema.clone();
                    let has_params = !params.is_empty();

                    // 将脚本存入应用状态（供执行时使用）
                    {
                        let mut app_guard = app.lock().await;
                        app_guard.state.lock().await.window_state =
                            state::MainWindowState::Reviewing { script };
                    }

                    // 更新 UI
                    let weak_review = weak_clone.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = weak_review.upgrade() {
                            // 始终显示脚本信息
                            w.set_script_name(name.into());
                            w.set_script_description(description.into());
                            w.set_script_content(content.into());

                            if has_params {
                                // 填充参数表单（最多 6 个）
                                let param_setters: Vec<_> = params.iter().take(6).enumerate().map(|(i, p)| {
                                    (
                                        i,
                                        ui::ParamField {
                                            name: p.name.clone().into(),
                                            label: p.label.clone().into(),
                                            value: p.default.clone().unwrap_or_default().into(),
                                            required: p.required,
                                            visible: true,
                                        },
                                    )
                                }).collect();

                                for (i, field) in param_setters {
                                    match i {
                                        0 => w.set_param0(field),
                                        1 => w.set_param1(field),
                                        2 => w.set_param2(field),
                                        3 => w.set_param3(field),
                                        4 => w.set_param4(field),
                                        5 => w.set_param5(field),
                                        _ => {}
                                    }
                                }
                                // 隐藏未使用的参数字段
                                for i in params.len()..6 {
                                    match i {
                                        0 => w.set_param0(ui::ParamField {
                                            name: "".into(), label: "".into(), value: "".into(),
                                            required: false, visible: false }),
                                        1 => w.set_param1(ui::ParamField {
                                            name: "".into(), label: "".into(), value: "".into(),
                                            required: false, visible: false }),
                                        2 => w.set_param2(ui::ParamField {
                                            name: "".into(), label: "".into(), value: "".into(),
                                            required: false, visible: false }),
                                        3 => w.set_param3(ui::ParamField {
                                            name: "".into(), label: "".into(), value: "".into(),
                                            required: false, visible: false }),
                                        4 => w.set_param4(ui::ParamField {
                                            name: "".into(), label: "".into(), value: "".into(),
                                            required: false, visible: false }),
                                        5 => w.set_param5(ui::ParamField {
                                            name: "".into(), label: "".into(), value: "".into(),
                                            required: false, visible: false }),
                                        _ => {}
                                    }
                                }
                                w.set_current_state("param_input".into());
                            } else {
                                w.set_current_state("reviewing".into());
                            }
                            w.set_status_text("".into());
                        }
                    });
                }
                Err(e) => {
                    // 生成失败，显示错误信息
                    let err_msg = i18n::t_args("generation_failed", &[("error", &e.to_string())]);
                    let weak_err = weak_clone.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = weak_err.upgrade() {
                            w.set_current_state("input".into());
                            w.set_status_text(err_msg.into());
                        }
                    });
                }
            }
        });
    });

    // ============================================================
    // 回调：直接执行脚本（从审阅面板触发，跳过参数表单）
    // ============================================================
    let window_weak_exec = window.as_weak();
    let app_exec = app.clone();
    let rt_exec = runtime.handle().clone();

    window.on_execute_script(move || {
        let weak = window_weak_exec.clone();
        let app = app_exec.clone();
        let rt = rt_exec.clone();

        let weak_for_spawn = weak.clone();
        let rt_for_spawn = rt.clone();
        rt_for_spawn.spawn(async move {
            // 从应用状态获取当前审阅中的脚本
            let script = {
                let app_guard = app.lock().await;
                let state_guard = app_guard.state.lock().await;
                match &state_guard.window_state {
                    state::MainWindowState::Reviewing { script } => script.clone(),
                    _ => {
                        // 状态不对，返回错误
                        let weak_err = weak_for_spawn.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak_err.upgrade() {
                                w.set_status_text(i18n::t("state_error").into());
                            }
                        });
                        return;
                    }
                }
            };

            // 收集参数（无参数脚本使用空 HashMap）
            let params = collect_params_from_ui(&weak_for_spawn);

            // 执行并流式输出
            execute_and_stream(weak_for_spawn, app.clone(), rt.clone(), script, params).await;
        });
    });

    // ============================================================
    // 回调：提交参数并执行（从参数表单触发）
    // ============================================================
    let window_weak_params = window.as_weak();
    let app_params = app.clone();
    let rt_params = runtime.handle().clone();

    window.on_submit_params(move || {
        let weak = window_weak_params.clone();
        let app = app_params.clone();
        let rt = rt_params.clone();

        let weak_for_spawn = weak.clone();
        let rt_for_spawn = rt.clone();
        rt_for_spawn.spawn(async move {
            let script = {
                let app_guard = app.lock().await;
                let state_guard = app_guard.state.lock().await;
                match &state_guard.window_state {
                    state::MainWindowState::Reviewing { script } => script.clone(),
                    _ => {
                        let weak_err = weak_for_spawn.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak_err.upgrade() {
                                w.set_status_text(i18n::t("state_error").into());
                            }
                        });
                        return;
                    }
                }
            };

            // 收集用户填写的参数
            let params = collect_params_from_ui(&weak_for_spawn);

            // 切换到审阅状态以显示输出
            let weak_review = weak_for_spawn.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_review.upgrade() {
                    w.set_current_state("reviewing".into());
                }
            });

            execute_and_stream(weak_for_spawn, app.clone(), rt.clone(), script, params).await;
        });
    });

    // ============================================================
    // 回调：停止正在执行的任务
    // ============================================================
    let window_weak_stop = window.as_weak();
    let app_stop = app.clone();
    let rt_stop = runtime.handle().clone();

    window.on_stop_script(move || {
        let weak = window_weak_stop.clone();
        let app = app_stop.clone();
        let rt = rt_stop.clone();

        rt.spawn(async move {
            // 获取当前运行中的任务 ID
            let task_id = {
                if let Some(w) = weak.upgrade() {
                    w.get_running_task_id().to_string()
                } else {
                    String::new()
                }
            };

            if !task_id.is_empty() {
                let task_manager = {
                    let app_guard = app.lock().await;
                    app_guard.task_manager.clone()
                };

                match task_manager.stop_task(&task_id).await {
                    Ok(_) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak.upgrade() {
                                w.set_is_executing(false);
                                w.set_running_task_id("".into());
                                w.set_status_text(i18n::t("task_stopped").into());
                                w.set_current_state("input".into());
                                w.set_input_text("".into());
                            }
                        });
                    }
                    Err(e) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak.upgrade() {
                                w.set_status_text(i18n::t_args("stop_failed", &[("error", &e.to_string())]).into());
                            }
                        });
                    }
                }
            }
        });
    });

    // ============================================================
    // 回调：取消（放弃审阅/参数）
    // ============================================================
    let window_weak_cancel = window.as_weak();
    window.on_cancel(move || {
        let weak = window_weak_cancel.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade() {
                w.set_is_executing(false);
                w.set_running_task_id("".into());
                w.set_current_state("input".into());
                w.set_input_text("".into());
                w.set_status_text(i18n::t("discarded").into());
            }
        });
    });

    // ============================================================
    // 回调：取消参数输入
    // ============================================================
    let window_weak_cancel_params = window.as_weak();
    window.on_cancel_params(move || {
        let weak = window_weak_cancel_params.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade() {
                w.set_current_state("input".into());
                w.set_input_text("".into());
                w.set_status_text(i18n::t("canceled").into());
            }
        });
    });

    // ============================================================
    // 回调：切换 ASR 语音输入（预留）
    // ============================================================
    let window_weak_asr = window.as_weak();
    let asr_active = RefCell::new(false);
    window.on_toggle_asr(move || {
        let mut active = asr_active.borrow_mut();
        *active = !*active;
        let weak = window_weak_asr.clone();
        let is_active = *active;
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade() {
                w.set_asr_active(is_active);
            }
        });
        if is_active {
            println!("ASR 已启用（待实现）");
        }
    });

    // ============================================================
    // 回调：打开设置面板
    // ============================================================
    let window_weak_settings = window.as_weak();
    window.on_open_settings(move || {
        let weak = window_weak_settings.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade() {
                w.set_settings_visible(true);
            }
        });
    });

    // ============================================================
    // 回调：关闭设置面板
    // ============================================================
    let window_weak_close = window.as_weak();
    window.on_close_settings(move || {
        let weak = window_weak_close.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade() {
                w.set_settings_visible(false);
            }
        });
    });

    // ============================================================
    // 回调：保存设置
    // ============================================================
    let window_weak_save = window.as_weak();
    let app_save = app.clone();
    let rt_save = runtime.handle().clone();
    window.on_save_settings(move || {
        let weak = window_weak_save.clone();
        let app = app_save.clone();
        let rt = rt_save.clone();

        // 从 UI 读取设置值（回调在 Slint 线程执行，可以直接读）
        let (api_key, api_base, model) = if let Some(w) = weak.upgrade() {
            (
                w.get_settings_api_key().to_string(),
                w.get_settings_api_base().to_string(),
                w.get_settings_model().to_string(),
            )
        } else {
            return;
        };

        rt.spawn(async move {
            let base_opt = if api_base.is_empty() { None } else { Some(api_base) };

            // 更新内存中的设置
            let mut app_guard = app.lock().await;
            app_guard.settings.ai.api_key = api_key.clone();
            app_guard.settings.ai.api_base = base_opt.clone();
            app_guard.settings.ai.model = model.clone();

            // 持久化到 keyring
            let _ = app_guard.settings_manager.set("ai_api_key", &api_key);
            let _ = app_guard.settings_manager.set("ai_model", &model);
            if let Some(ref b) = base_opt {
                let _ = app_guard.settings_manager.set("ai_api_base", b);
            }

            // 重新创建 LLM 客户端（使新配置生效）
            app_guard.llm_client = if api_key.is_empty() {
                None
            } else {
                Some(llm::LlmClient::new(api_key, model, base_opt))
            };

            drop(app_guard);

            // 关闭设置面板并提示保存成功
            let weak_close = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_close.upgrade() {
                    w.set_settings_visible(false);
                    w.set_status_text(i18n::t("settings_saved").into());
                }
            });
        });
    });

    // 启动 Slint 事件循环（阻塞）
    window.run().expect("运行失败");
}

/// 从 Slint UI 收集参数表单的值
/// 遍历 6 个参数字段，将可见且非空的参数收集到 HashMap
fn collect_params_from_ui(
    weak: &Weak<ui::JitaWindow>,
) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();

    if let Some(w) = weak.upgrade() {
        // 依次检查 6 个参数字段
        let p0 = w.get_param0();
        if p0.visible && !p0.name.is_empty() {
            params.insert(p0.name.to_string(), p0.value.to_string());
        }
        let p1 = w.get_param1();
        if p1.visible && !p1.name.is_empty() {
            params.insert(p1.name.to_string(), p1.value.to_string());
        }
        let p2 = w.get_param2();
        if p2.visible && !p2.name.is_empty() {
            params.insert(p2.name.to_string(), p2.value.to_string());
        }
        let p3 = w.get_param3();
        if p3.visible && !p3.name.is_empty() {
            params.insert(p3.name.to_string(), p3.value.to_string());
        }
        let p4 = w.get_param4();
        if p4.visible && !p4.name.is_empty() {
            params.insert(p4.name.to_string(), p4.value.to_string());
        }
        let p5 = w.get_param5();
        if p5.visible && !p5.name.is_empty() {
            params.insert(p5.name.to_string(), p5.value.to_string());
        }
    }

    params
}

/// 执行脚本并流式更新 UI
/// 1. 调用 TaskManager 启动子进程
/// 2. 异步读取 stdout/stderr 并实时推送 UI
/// 3. 等待执行完成，记录历史，重置 UI
async fn execute_and_stream(
    weak: Weak<ui::JitaWindow>,
    app: Arc<Mutex<App>>,
    rt: tokio::runtime::Handle,
    script: script::Script,
    params: std::collections::HashMap<String, String>,
) {
    // 启动任务
    let result = {
        let app_guard = app.lock().await;
        app_guard.execute_script(script.clone(), params.clone()).await
    };

    match result {
        Ok(task_handle) => {
            let task_id = task_handle.task_id.clone();
            let mut rx = task_handle.rx;
            let task_manager = app.lock().await.task_manager.clone();

            // 更新 UI 为执行状态
            let weak_status = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_status.upgrade() {
                    w.set_is_executing(true);
                    w.set_running_task_id(task_id.clone().into());
                    w.set_status_text(i18n::t_args("task_started", &[("task_id", &task_id)]).into());
                    w.set_script_name(script.name.clone().into());
                }
            });

            // 并发读取输出流
            let weak_output = weak.clone();
            let rt2 = rt.clone();
            rt2.spawn(async move {
                const MAX_OUTPUT_LINES: usize = 1000; // 限制内存：最多保留 1000 行
                let mut output_lines = Vec::new();

                while let Some(line) = rx.recv().await {
                    match line {
                        execution::OutputLine::Stdout(line) => {
                            output_lines.push(format!("[stdout] {}", line));
                        }
                        execution::OutputLine::Stderr(line) => {
                            output_lines.push(format!("[stderr] {}", line));
                        }
                    }

                    // 超出上限时丢弃最早的行
                    if output_lines.len() > MAX_OUTPUT_LINES {
                        output_lines.drain(0..output_lines.len() - MAX_OUTPUT_LINES);
                    }

                    // 每 3 行更新一次 UI（避免过于频繁）
                    if output_lines.len() % 3 == 0 {
                        let text = output_lines.join("\n");
                        let weak_update = weak_output.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak_update.upgrade() {
                                w.set_script_content(text.into());
                            }
                        });
                    }
                }
            });

            // 等待任务完成并记录历史
            let weak_completion = weak.clone();
            let script_id = script.id.clone();
            let params_json = serde_json::to_value(&params).unwrap_or_default();
            let handle_arc = task_handle.handle.clone();
            let rt3 = rt.clone();
            rt3.spawn(async move {
                // 等待进程退出
                let exit_code = {
                    let mut handle_guard = handle_arc.lock().await;
                    if let Some(handle) = handle_guard.take() {
                        match handle.await {
                            Ok(Ok(code)) => Some(code),
                            _ => None,
                        }
                    } else {
                        None
                    }
                };

                // 生成消息和错误摘要
                let (msg, record_stderr) = if let Some(code) = exit_code {
                    if code == 0 {
                        (i18n::t("execution_success"), None)
                    } else {
                        (i18n::t_args("execution_failed_exit_code", &[("code", &code.to_string())]), Some(format!("exit code: {}", code)))
                    }
                } else {
                    (i18n::t("execution_error"), Some("execution error".to_string()))
                };

                // 记录到数据库
                {
                    let app_guard = app.lock().await;
                    let _ = app_guard.record_execution(
                        &script_id,
                        params_json,
                        exit_code,
                        record_stderr,
                    ).await;
                }

                // 重置 UI
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak_completion.upgrade() {
                        w.set_is_executing(false);
                        w.set_running_task_id("".into());
                        w.set_status_text(msg.into());
                        w.set_current_state("input".into());
                        w.set_input_text("".into());
                    }
                });
            });
        }
        Err(e) => {
            let weak_err = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_err.upgrade() {
                    w.set_is_executing(false);
                    w.set_status_text(i18n::t_args("execution_failed", &[("error", &e.to_string())]).into());
                }
            });
        }
    }
}
