// Jita Tauri 应用入口

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tokio::sync::Mutex;

use jita_lib::agent::AgentModule;
use jita_lib::commands::AppState;
use jita_lib::settings::AppSettings;
use jita_lib::task_manager::TaskManager;
use jita_lib::ui::asr::AsrManager;
use jita_lib::utils;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    tracing::info!("Jita starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            let (agent, uv_available) = {
                utils::ensure_dirs().ok();
                let settings = AppSettings::load_from_default().unwrap_or_default();
                let agent = AgentModule::new(settings).unwrap_or_else(|e| {
                    tracing::error!("Failed to create AgentModule: {}", e);
                    panic!("AgentModule creation failed");
                });
                let uv_available = matches!(utils::check_uv(), utils::UvStatus::Available(_));
                (agent, uv_available)
            };

            let task_manager = TaskManager::new();
            let asr_manager = AsrManager::new(utils::models_dir());

            let app_state = AppState {
                agent: Arc::new(Mutex::new(agent)),
                task_manager: Arc::new(Mutex::new(task_manager)),
                asr_manager: Arc::new(Mutex::new(asr_manager)),
                uv_available,
            };

            app.manage(app_state.clone());

            // 后台初始化工具索引
            let agent_arc = app_state.agent.clone();
            rt.spawn(async move {
                let agent = agent_arc.lock().await;
                let agent_for_indexing = Arc::new((&*agent).clone());
                agent_for_indexing.init_tool_indexing().await;
            });

            // 创建系统托盘
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // 处理主窗口关闭事件 - 隐藏而不是退出
            let main_window = app.get_webview_window("main").unwrap();
            let main_window_clone = main_window.clone();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = main_window_clone.hide();
                }
            });

            tracing::info!("App initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            jita_lib::commands::set_language,
            jita_lib::commands::get_language,
            jita_lib::commands::generate_script,
            jita_lib::commands::execute_script,
            jita_lib::commands::stop_script,
            jita_lib::commands::get_settings,
            jita_lib::commands::save_settings,
            jita_lib::commands::open_models_folder,
            jita_lib::commands::check_uv,
            jita_lib::commands::toggle_asr,
            jita_lib::commands::preload_asr,
            jita_lib::commands::open_settings_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
