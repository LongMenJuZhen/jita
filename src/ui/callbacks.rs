// UI 回调绑定模块

use slint::{ComponentHandle, Weak};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

use crate::app::App;
use crate::ui::asr::AsrManager;
use crate::task_manager::execution::OutputLine;
use crate::agent;
use crate::task_manager::script::{ParamDeclaration, Script};
use crate::state::MainWindowState;
use crate::ui::hotkey::HotkeyManager;
use crate::ui::i18n as ui_i18n;

pub fn setup_callbacks(
    window: crate::ui::JitaWindow,
    app: Arc<TokioMutex<App>>,
    runtime: tokio::runtime::Handle,
    asr_manager: Arc<TokioMutex<AsrManager>>,
    hotkey_manager: Option<HotkeyManager>,
) -> crate::ui::JitaWindow {
    let window_weak = window.as_weak();

    // on_submit_input
    {
        let weak = window_weak.clone();
        let app = app.clone();
        let rt = runtime.clone();
        window.on_submit_input(move |text: slint::SharedString| {
            let text = text.to_string();
            let weak = weak.clone();
            let app = app.clone();
            let rt = rt.clone();
            rt.spawn(async move {
                {
                    let weak_gen = weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = weak_gen.upgrade() {
                            w.set_current_state("generating".into());
                            w.set_status_text(ui_i18n::t("ai_generating").into());
                        }
                    });
                }

                let result = {
                    let app_guard = app.lock().await;
                    app_guard.generate_script(&text).await
                };

                match result {
                    Ok(script) => {
                        let name = script.name.clone();
                        let description = script.description.clone();
                        let content = script.content.clone();
                        let params = script.params_schema.clone();
                        let has_params = !params.is_empty();

                        {
                            let app_guard = app.lock().await;
                            app_guard.state.lock().await.window_state =
                                MainWindowState::Reviewing { script };
                        }

                        let weak_review = weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak_review.upgrade() {
                                w.set_script_name(name.into());
                                w.set_script_description(description.into());
                                w.set_script_content(content.into());

                                if has_params {
                                    fill_params(&w, &params);
                                    w.set_current_state("param_input".into());
                                } else {
                                    w.set_current_state("reviewing".into());
                                }
                                w.set_status_text("".into());
                            }
                        });
                    }
                    Err(e) => {
                        let err_msg = ui_i18n::t_args("generation_failed", &[("error", &e.to_string())]);
                        let weak_err = weak.clone();
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
    }

    // on_execute_script
    {
        let weak = window_weak.clone();
        let app = app.clone();
        let rt = runtime.clone();
        window.on_execute_script(move || {
            let weak = weak.clone();
            let app = app.clone();
            let rt = rt.clone();
            rt.spawn(async move {
                let script = {
                    let app_guard = app.lock().await;
                    let state_guard = app_guard.state.lock().await;
                    match &state_guard.window_state {
                        MainWindowState::Reviewing { script } => script.clone(),
                        _ => {
                            let weak_err = weak.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = weak_err.upgrade() {
                                    w.set_status_text(ui_i18n::t("state_error").into());
                                }
                            });
                            return;
                        }
                    }
                };

                let params = collect_params_from_ui(&weak);
                execute_and_stream(weak, app, script, params).await;
            });
        });
    }

    // on_submit_params
    {
        let weak = window_weak.clone();
        let app = app.clone();
        let rt = runtime.clone();
        window.on_submit_params(move || {
            let weak = weak.clone();
            let app = app.clone();
            let rt = rt.clone();
            rt.spawn(async move {
                let script = {
                    let app_guard = app.lock().await;
                    let state_guard = app_guard.state.lock().await;
                    match &state_guard.window_state {
                        MainWindowState::Reviewing { script } => script.clone(),
                        _ => {
                            let weak_err = weak.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = weak_err.upgrade() {
                                    w.set_status_text(ui_i18n::t("state_error").into());
                                }
                            });
                            return;
                        }
                    }
                };

                let params = collect_params_from_ui(&weak);

                let weak_review = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak_review.upgrade() {
                        w.set_current_state("reviewing".into());
                    }
                });

                execute_and_stream(weak, app, script, params).await;
            });
        });
    }

    // on_stop_script
    {
        let weak = window_weak.clone();
        let app = app.clone();
        let rt = runtime.clone();
        window.on_stop_script(move || {
            let weak = weak.clone();
            let app = app.clone();
            let rt = rt.clone();
            rt.spawn(async move {
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
                                    w.set_status_text(ui_i18n::t("task_stopped").into());
                                    w.set_current_state("input".into());
                                    w.set_input_text("".into());
                                }
                            });
                        }
                        Err(e) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = weak.upgrade() {
                                    w.set_status_text(
                                        ui_i18n::t_args("stop_failed", &[("error", &e.to_string())]).into()
                                    );
                                }
                            });
                        }
                    }
                }
            });
        });
    }

    // on_cancel
    {
        let weak = window_weak.clone();
        window.on_cancel(move || {
            let weak = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    w.set_is_executing(false);
                    w.set_running_task_id("".into());
                    w.set_current_state("input".into());
                    w.set_input_text("".into());
                    w.set_status_text(ui_i18n::t("discarded").into());
                }
            });
        });
    }

    // on_cancel_params
    {
        let weak = window_weak.clone();
        window.on_cancel_params(move || {
            let weak = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    w.set_current_state("input".into());
                    w.set_input_text("".into());
                    w.set_status_text(ui_i18n::t("canceled").into());
                }
            });
        });
    }

    // on_toggle_asr
    {
        let weak = window_weak.clone();
        let manager = asr_manager.clone();
        let rt_handle = runtime.clone();
        window.on_toggle_asr(move || {
            let weak = weak.clone();
            let manager = manager.clone();
            let rt = rt_handle.clone();
            rt.spawn(async move {
                let mut asr = manager.lock().await;

                if asr.is_listening() {
                    asr.stop();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = weak.upgrade() {
                            w.set_asr_active(false);
                            w.set_status_text("ASR 已停止".into());
                        }
                    });
                } else {
                    let weak_for_status = weak.clone();
                    let weak_for_text = weak.clone();

                    match asr.start_listening(
                        move |status| {
                            let weak_cb = weak_for_status.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = weak_cb.upgrade() {
                                    w.set_status_text(status.into());
                                }
                            });
                        },
                        move |text| {
                            let weak_cb = weak_for_text.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = weak_cb.upgrade() {
                                    w.set_input_text(text.into());
                                }
                            });
                        },
                    ) {
                        Ok(_) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = weak.upgrade() {
                                    w.set_asr_active(true);
                                }
                            });
                        }
                        Err(e) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = weak.upgrade() {
                                    w.set_asr_active(false);
                                    w.set_status_text(format!("ASR 错误: {}", e).into());
                                }
                            });
                        }
                    }
                }
            });
        });
    }

    // on_open_settings
    {
        let weak = window_weak.clone();
        window.on_open_settings(move || {
            let weak = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    w.set_settings_visible(true);
                }
            });
        });
    }

    // on_close_settings
    {
        let weak = window_weak.clone();
        window.on_close_settings(move || {
            let weak = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    w.set_settings_visible(false);
                }
            });
        });
    }

    // on_save_settings
    {
        let weak = window_weak.clone();
        let app = app.clone();
        let rt = runtime.clone();
        window.on_save_settings(move || {
            let weak = weak.clone();
            let app = app.clone();
            let rt = rt.clone();

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

                let mut app_guard = app.lock().await;
                app_guard.settings.ai.api_key = api_key.clone();
                app_guard.settings.ai.api_base = base_opt.clone();
                app_guard.settings.ai.model = model.clone();

                let _ = app_guard.settings_manager.set("ai_api_key", &api_key);
                let _ = app_guard.settings_manager.set("ai_model", &model);
                if let Some(ref b) = base_opt {
                    let _ = app_guard.settings_manager.set("ai_api_base", b);
                }

                app_guard.agent_client = if api_key.is_empty() {
                    None
                } else {
                    match agent::AgentClient::new(api_key.clone(), Some(model.clone()), base_opt) {
                        Ok(client) => Some(client),
                        Err(e) => {
                            tracing::warn!("Failed to create agent client: {}", e);
                            None
                        }
                    }
                };

                drop(app_guard);

                let weak_close = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = weak_close.upgrade() {
                        w.set_settings_visible(false);
                        w.set_status_text(ui_i18n::t("settings_saved").into());
                    }
                });
            });
        });
    }

    window.on_open_models_folder(move || {
        let model_dir = crate::utils::models_dir();
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer").arg(&model_dir).spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(&model_dir).spawn();
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = std::process::Command::new("xdg-open").arg(&model_dir).spawn();
        }
    });

    if let Some(mut manager) = hotkey_manager {
        if manager.register("ctrl+space").is_ok() {
            manager.start_listener();
        }
    }

    window
}

fn fill_params(window: &crate::ui::JitaWindow, params: &[ParamDeclaration]) {
    for (i, p) in params.iter().take(6).enumerate() {
        let field = crate::ui::ParamField {
            name: p.name.clone().into(),
            label: p.label.clone().into(),
            value: p.default.clone().unwrap_or_default().into(),
            required: p.required,
            visible: true,
        };
        match i {
            0 => window.set_param0(field),
            1 => window.set_param1(field),
            2 => window.set_param2(field),
            3 => window.set_param3(field),
            4 => window.set_param4(field),
            5 => window.set_param5(field),
            _ => {}
        }
    }
    for i in params.len()..6 {
        clear_param(window, i);
    }
}

fn clear_param(window: &crate::ui::JitaWindow, i: usize) {
    let field = crate::ui::ParamField {
        name: "".into(),
        label: "".into(),
        value: "".into(),
        required: false,
        visible: false,
    };
    match i {
        0 => window.set_param0(field),
        1 => window.set_param1(field),
        2 => window.set_param2(field),
        3 => window.set_param3(field),
        4 => window.set_param4(field),
        5 => window.set_param5(field),
        _ => {}
    }
}

fn collect_params_from_ui(weak: &Weak<crate::ui::JitaWindow>) -> HashMap<String, String> {
    let mut params = HashMap::new();

    if let Some(w) = weak.upgrade() {
        let fields = [
            w.get_param0(),
            w.get_param1(),
            w.get_param2(),
            w.get_param3(),
            w.get_param4(),
            w.get_param5(),
        ];
        for p in fields {
            if p.visible && !p.name.is_empty() {
                params.insert(p.name.to_string(), p.value.to_string());
            }
        }
    }

    params
}

async fn execute_and_stream(
    weak: Weak<crate::ui::JitaWindow>,
    app: Arc<TokioMutex<App>>,
    script: Script,
    params: HashMap<String, String>,
) {
    let result = {
        let app_guard = app.lock().await;
        app_guard.execute_script(script.clone(), params.clone()).await
    };

    match result {
        Ok(task_handle) => {
            let task_id = task_handle.task_id.clone();
            let mut rx = task_handle.rx;

            let weak_status = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_status.upgrade() {
                    w.set_is_executing(true);
                    w.set_running_task_id(task_id.clone().into());
                    w.set_status_text(
                        ui_i18n::t_args("task_started", &[("task_id", &task_id)]).into()
                    );
                    w.set_script_name(script.name.clone().into());
                }
            });

            let weak_output = weak.clone();
            tokio::spawn(async move {
                const MAX_OUTPUT_LINES: usize = 1000;
                let mut output_lines = Vec::new();

                while let Some(line) = rx.recv().await {
                    match line {
                        OutputLine::Stdout(line) => {
                            output_lines.push(format!("[stdout] {}", line));
                        }
                        OutputLine::Stderr(line) => {
                            output_lines.push(format!("[stderr] {}", line));
                        }
                    }

                    if output_lines.len() > MAX_OUTPUT_LINES {
                        output_lines.drain(0..output_lines.len() - MAX_OUTPUT_LINES);
                    }

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

            let weak_completion = weak.clone();
            let script_id = script.id.clone();
            let params_json = serde_json::to_value(&params).unwrap_or_default();
            let handle_arc = task_handle.handle.clone();
            tokio::spawn(async move {
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

                let (msg, record_stderr) = if let Some(code) = exit_code {
                    if code == 0 {
                        (ui_i18n::t("execution_success"), None)
                    } else {
                        (
                            ui_i18n::t_args("execution_failed_exit_code", &[("code", &code.to_string())]),
                            Some(format!("exit code: {}", code))
                        )
                    }
                } else {
                    (ui_i18n::t("execution_error"), Some("execution error".to_string()))
                };

                {
                    let app_guard = app.lock().await;
                    let _ = app_guard.record_execution(&script_id, params_json, exit_code, record_stderr).await;
                }

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
                    w.set_status_text(
                        ui_i18n::t_args("execution_failed", &[("error", &e.to_string())]).into()
                    );
                }
            });
        }
    }
}