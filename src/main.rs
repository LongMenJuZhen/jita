// Jita 主入口文件
// 只负责初始化 tokio runtime 和 App，然后交给 UI 子系统

mod agent;
mod app;
mod settings;
mod state;
mod task_manager;
mod ui;
mod utils;

use app::App;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    tracing::info!("Jita starting...");

    // 创建 tokio 异步运行时
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    // 初始化应用（阻塞等待完成）
    let app = runtime.block_on(async { App::new().expect("Failed to initialize app") });

    tracing::info!("App initialized, starting UI...");

    // 交给 UI 子系统
    ui::run(app, runtime);
}
