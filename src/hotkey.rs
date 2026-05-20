// 全局快捷键管理模块
// 提供跨平台全局热键注册与监听功能
// 用户可通过快捷键在任何应用中唤醒 Jita 浮窗

use std::sync::mpsc;

/// 快捷键管理器
/// 用于注册和监听全局热键事件
pub struct HotkeyManager {
    event_rx: mpsc::Receiver<()>, // 接收快捷键触发事件
}

impl HotkeyManager {
    /// 创建新的快捷键管理器
    pub fn new() -> std::io::Result<Self> {
        let (_, event_rx) = mpsc::channel();
        // 注意：完整的全局热键支持需要平台特定实现
        // 当前为占位符，可后续增强
        Ok(Self { event_rx })
    }

    /// 检查是否有待处理的快捷键事件
    pub fn is_pressed(&self) -> bool {
        // 尝试非阻塞接收事件
        self.event_rx.try_recv().is_ok()
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            let (_, event_rx) = mpsc::channel();
            Self { event_rx }
        })
    }
}
