// Jita Tauri 应用入口
// Rust 端只负责：托盘、窗管、全局快捷键、Deno进程管理

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

use jita_lib::deno::{start_deno, DenoProcessState};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    tracing::info!("Jita starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(DenoProcessState::new())
        .setup(|app| {
            // 启动 Deno 后端进程
            let deno_path = std::env::var("DENO_PATH")
                .unwrap_or_else(|_| "deno".to_string());
            let script_path = std::env::current_dir()
                .unwrap_or_default()
                .join("jita-deno/main.ts");

            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = start_deno(
                        app_handle,
                        &deno_path,
                        script_path.to_str().unwrap_or("jita-deno/main.ts"),
                    ).await {
                        tracing::error!("Failed to start Deno process: {}", e);
                    }
                });
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
            jita_lib::deno::deno_invoke,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}