mod agent;
mod app;
mod asr;
mod db;
mod embedding;
mod execution;
mod hotkey;
mod llm;
mod script;
mod settings;
mod state;
mod task_manager;
mod tray;
mod ui;
mod utils;

use app::App;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::sync::Arc;
use tokio::sync::Mutex;

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    let app = runtime
        .block_on(async { App::new().expect("Failed to initialize app") });

    let app = Arc::new(Mutex::new(app));

    // Create window
    let window = ui::JitaWindow::new().expect("创建窗口失败");

    // Load settings into UI
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
            // Switch to generating state
            let weak_gen = weak_clone.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_gen.upgrade() {
                    w.set_current_state("generating".into());
                    w.set_status_text("AI 生成中...".into());
                }
            });

            let result = {
                let app_guard = app.lock().await;
                app_guard.generate_script(&text).await
            };

            match result {
                Ok(script) => {
                    let name = script.name.clone();
                    let description = script.description.clone();
                    let content = script.content.clone();

                    // Store script temporarily for execution
                    {
                        let mut app_guard = app.lock().await;
                        app_guard
                            .state
                            .lock()
                            .await
                            .window_state = state::MainWindowState::Reviewing { script };
                    }

                    let weak_review = weak_clone.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = weak_review.upgrade() {
                            w.set_current_state("reviewing".into());
                            w.set_script_name(name.into());
                            w.set_script_description(description.into());
                            w.set_script_content(content.into());
                            w.set_status_text("".into());
                        }
                    });
                }
                Err(e) => {
                    let err_msg = format!("生成失败: {}", e);
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
            let script = {
                let app_guard = app.lock().await;
                let state_guard = app_guard.state.lock().await;
                match &state_guard.window_state {
                    state::MainWindowState::Reviewing { script } => script.clone(),
                    _ => {
                        let weak_err = weak_for_spawn.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak_err.upgrade() {
                                w.set_status_text("状态错误：不在审阅状态".into());
                            }
                        });
                        return;
                    }
                }
            };

            let params = std::collections::HashMap::new();

            let result = {
                let app_guard = app.lock().await;
                app_guard.execute_script(script.clone(), params).await
            };

            match result {
                Ok(task_handle) => {
                    let task_id = task_handle.task_id.clone();
                    let mut rx = task_handle.rx;

                    let weak_status = weak_for_spawn.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = weak_status.upgrade() {
                            w.set_status_text(format!("任务 {} 开始执行...", task_id).into());
                        }
                    });

                    // Stream output
                    let weak_output = weak_for_spawn.clone();
                    let rt2 = rt.clone();
                    rt2.spawn(async move {
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

                            if output_lines.len() % 5 == 0 {
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

                    // Wait for completion
                    let weak_completion = weak_for_spawn.clone();
                    let rt3 = rt.clone();
                    rt3.spawn(async move {
                        let exit_code = match task_handle.handle.await {
                            Ok(Ok(code)) => Some(code),
                            _ => None,
                        };

                        let msg = if let Some(code) = exit_code {
                            if code == 0 {
                                "执行成功".to_string()
                            } else {
                                format!("执行失败，退出码: {}", code)
                            }
                        } else {
                            "执行异常".to_string()
                        };

                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak_completion.upgrade() {
                                w.set_status_text(msg.into());
                                w.set_current_state("input".into());
                                w.set_input_text("".into());
                            }
                        });
                    });
                }
                Err(e) => {
                    let weak_err = weak_for_spawn.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = weak_err.upgrade() {
                            w.set_status_text(format!("执行失败: {}", e).into());
                        }
                    });
                }
            }
        });
    });

    let window_weak_cancel = window.as_weak();
    window.on_cancel(move || {
        let weak = window_weak_cancel.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade() {
                w.set_current_state("input".into());
                w.set_input_text("".into());
                w.set_status_text("已放弃".into());
            }
        });
    });

    // ASR toggle (placeholder)
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

        // TODO: Start/stop ASR engine when toggled
        if is_active {
            println!("ASR 已启用（待实现）");
        }
    });

    // Settings callbacks
    let window_weak_settings = window.as_weak();
    window.on_open_settings(move || {
        let weak = window_weak_settings.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade() {
                w.set_settings_visible(true);
            }
        });
    });

    let window_weak_close = window.as_weak();
    window.on_close_settings(move || {
        let weak = window_weak_close.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade() {
                w.set_settings_visible(false);
            }
        });
    });

    let window_weak_save = window.as_weak();
    let app_save = app.clone();
    let rt_save = runtime.handle().clone();
    window.on_save_settings(move || {
        let weak = window_weak_save.clone();
        let app = app_save.clone();
        let rt = rt_save.clone();

        // Read values directly from UI (we are on Slint's event loop thread)
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

            // Save to settings
            let mut app_guard = app.lock().await;
            app_guard.settings.ai.api_key = api_key.clone();
            app_guard.settings.ai.api_base = base_opt.clone();
            app_guard.settings.ai.model = model.clone();

            // Persist to keyring
            let _ = app_guard.settings_manager.set("ai_api_key", &api_key);
            let _ = app_guard.settings_manager.set("ai_model", &model);
            if let Some(ref b) = base_opt {
                let _ = app_guard.settings_manager.set("ai_api_base", b);
            }

            // Recreate LLM client
            app_guard.llm_client = if api_key.is_empty() {
                None
            } else {
                Some(llm::LlmClient::new(api_key, model, base_opt))
            };

            drop(app_guard);

            let weak_close = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_close.upgrade() {
                    w.set_settings_visible(false);
                    w.set_status_text("设置已保存".into());
                }
            });
        });
    });

    window.run().expect("运行失败");
}
