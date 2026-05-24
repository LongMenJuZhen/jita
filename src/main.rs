// Jita Tauri 应用入口

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

use jita_lib::agent::AgentModule;
use jita_lib::commands::AppState;
use jita_lib::settings::AppSettings;
use jita_lib::task_manager::TaskManager;
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

            let app_state = AppState {
                agent: Arc::new(Mutex::new(agent)),
                task_manager: Arc::new(Mutex::new(task_manager)),
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

            tracing::info!("App initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            jita_lib::commands::generate_script,
            jita_lib::commands::execute_script,
            jita_lib::commands::stop_script,
            jita_lib::commands::get_settings,
            jita_lib::commands::save_settings,
            jita_lib::commands::open_models_folder,
            jita_lib::commands::check_uv,
            jita_lib::commands::toggle_asr,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}