// 系统托盘模块
// 基于 tray-icon crate 实现跨平台系统托盘图标和右键菜单

use anyhow::Result;
use std::sync::mpsc::{channel, Receiver, Sender};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent, Icon, menu::{Menu, MenuItem, PredefinedMenuItem}};

/// 托盘事件
#[derive(Debug, Clone, PartialEq)]
pub enum TrayEvent {
    /// 左键单击托盘图标
    LeftClick,
    /// 右键单击托盘图标
    RightClick,
    /// 双击托盘图标
    DoubleClick,
    /// 菜单项被点击
    MenuClicked { id: String },
    /// 打开主窗口
    ShowWindow,
    /// 打开设置
    OpenSettings,
    /// 退出应用
    Quit,
}

/// 托盘管理器
/// 管理托盘图标、菜单和事件分发
pub struct TrayManager {
    _tray_icon: TrayIcon,
    event_tx: Sender<TrayEvent>,
    event_rx: Receiver<TrayEvent>,
}

impl TrayManager {
    /// 创建托盘管理器
    /// 托盘图标使用内置的 32x32 像素占位图标（后续可替换为应用图标）
    pub fn new() -> Result<Self> {
        let (event_tx, event_rx) = channel();

        // 创建右键菜单
        let menu = Menu::new();
        let show_item = MenuItem::new("打开主窗口", true, None);
        let settings_item = MenuItem::new("设置", true, None);
        let separator = PredefinedMenuItem::separator();
        let quit_item = MenuItem::new("退出", true, None);

        menu.append(&show_item)?;
        menu.append(&settings_item)?;
        menu.append(&separator)?;
        menu.append(&quit_item)?;

        // 创建托盘图标（使用纯色占位图标）
        let icon = create_placeholder_icon();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Jita - AI 脚本助手")
            .with_icon(icon)
            .build()?;

        let show_id = show_item.id().0.clone();
        let settings_id = settings_item.id().0.clone();
        let quit_id = quit_item.id().0.clone();

        let tx_clone = event_tx.clone();
        std::thread::spawn(move || {
            let tray_receiver = TrayIconEvent::receiver();
            let menu_receiver = tray_icon::menu::MenuEvent::receiver();

            loop {
                // 监听托盘图标事件
                if let Ok(event) = tray_receiver.try_recv() {
                    let ev = match event {
                        TrayIconEvent::Click { button: tray_icon::MouseButton::Left, .. } => {
                            Some(TrayEvent::LeftClick)
                        }
                        TrayIconEvent::Click { button: tray_icon::MouseButton::Right, .. } => {
                            Some(TrayEvent::RightClick)
                        }
                        TrayIconEvent::DoubleClick { .. } => {
                            Some(TrayEvent::DoubleClick)
                        }
                        _ => None,
                    };
                    if let Some(e) = ev {
                        let _ = tx_clone.send(e);
                    }
                }

                // 监听菜单事件
                if let Ok(event) = menu_receiver.try_recv() {
                    let id = event.id.0;
                    let ev = if id == show_id {
                        TrayEvent::ShowWindow
                    } else if id == settings_id {
                        TrayEvent::OpenSettings
                    } else if id == quit_id {
                        TrayEvent::Quit
                    } else {
                        TrayEvent::MenuClicked { id }
                    };
                    let _ = tx_clone.send(ev);
                }

                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        });

        Ok(Self {
            _tray_icon: tray_icon,
            event_tx,
            event_rx,
        })
    }

    /// 检查是否有待处理的托盘事件（非阻塞）
    pub fn poll_event(&self) -> Option<TrayEvent> {
        self.event_rx.try_recv().ok()
    }

    /// 设置托盘提示文字
    pub fn set_tooltip(&self, tooltip: &str) -> Result<()> {
        self._tray_icon.set_tooltip(Some(tooltip))?;
        Ok(())
    }
}

/// 创建占位图标（32x32 蓝色方块）
fn create_placeholder_icon() -> Icon {
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity(width * height * 4);

    // Jita 主题色: #0078d4 (0, 120, 212)
    for _y in 0..height {
        for _x in 0..width {
            rgba.push(0);    // R
            rgba.push(120);  // G
            rgba.push(212);  // B
            rgba.push(255);  // A
        }
    }

    Icon::from_rgba(rgba, width as u32, height as u32)
        .unwrap_or_else(|_| create_minimal_icon())
}

/// 创建最小回退图标（1x1 像素）
fn create_minimal_icon() -> Icon {
    Icon::from_rgba(vec![0, 120, 212, 255], 1, 1)
        .expect("failed to create minimal icon")
}
