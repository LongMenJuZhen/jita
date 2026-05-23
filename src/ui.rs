// UI 子系统
// 统一管理 i18n、ASR、Hotkey、Tray、Slint 窗口和所有回调绑定

pub mod callbacks;
pub mod hotkey;
pub mod i18n;
pub mod tray;
mod asr;
mod generative;

use slint::ComponentHandle;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex as TokioMutex;

use crate::app::App;
use crate::settings::AppSettings;
use crate::ui::asr::AsrManager;
use crate::utils;

#[derive(Clone)]
struct AppStateForSettings {
    uv_available: bool,
    settings: AppSettings,
}

/// UI 子系统入口函数
pub fn run(app: App, runtime: Runtime) {
    i18n::init();

    let window = JitaWindow::new().expect("创建窗口失败");

    // 初始化设置
    window.set_uv_available(app.uv_available);
    window.set_current_state("input".into());
    window.set_settings_api_key(app.settings.ai.api_key.clone().into());
    window.set_settings_api_base(
        app.settings.ai.api_base.clone().unwrap_or_default().into(),
    );
    window.set_settings_model(app.settings.ai.model.clone().into());

    let model_dir = utils::models_dir();
    let asr_manager = Arc::new(TokioMutex::new(AsrManager::new(model_dir.clone())));

    let asr_preload = asr_manager.clone();
    let rt_preload = runtime.handle().clone();
    let window_preload = window.as_weak();
    let model_dir_clone = model_dir.clone();
    rt_preload.spawn(async move {
        let weak_status = window_preload.clone();

        let status_cb = move |status: String| {
            let weak_cb = weak_status.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_cb.upgrade() {
                    w.set_status_text(status.into());
                }
            });
        };

        if let Err(e) = asr::ensure_models(&model_dir_clone, &status_cb).await {
            let weak_err = window_preload.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_err.upgrade() {
                    w.set_status_text(format!("模型准备失败: {}", e).into());
                }
            });
            return;
        }

        if let Ok(_) = asr_preload.lock().await.preload(status_cb) {
            let weak_ok = window_preload.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak_ok.upgrade() {
                    w.set_status_text("ASR 模型已就绪".into());
                }
            });
        }
    });

    let tray = match tray::TrayManager::new() {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("托盘初始化失败: {}", e);
            None
        }
    };
    let _ = tray;

    let hotkey_manager = hotkey::HotkeyManager::new().ok();

    let app_wrapped = Arc::new(TokioMutex::new(app));
    let runtime_handle = runtime.handle().clone();
    let window = callbacks::setup_callbacks(
        window,
        app_wrapped,
        runtime_handle,
        asr_manager,
        hotkey_manager,
    );

    window.run().expect("运行失败");
}

slint::include_modules!();