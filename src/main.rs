// Jita 主入口文件
// 只负责初始化 tokio runtime 和 App，然后交给 UI 子系统

mod agent;
mod app;
mod llm;
mod settings;
mod state;
mod task_manager;
mod ui;
mod utils;

use app::App;

fn main() {
    // 创建 tokio 异步运行时
    let runtime = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime");

    // 初始化应用（阻塞等待完成）
    let app = runtime
        .block_on(async { App::new().expect("Failed to initialize app") });

    // 交给 UI 子系统
    ui::run(app, runtime);
}