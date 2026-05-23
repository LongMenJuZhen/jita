// 系统托盘模块

use anyhow::Result;
use std::sync::mpsc::{channel, Receiver, Sender};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent, Icon, menu::{Menu, MenuItem, PredefinedMenuItem}};

#[derive(Debug, Clone, PartialEq)]
pub enum TrayEvent {
    LeftClick,
    RightClick,
    DoubleClick,
    MenuClicked { id: String },
    ShowWindow,
    OpenSettings,
    Quit,
}

pub struct TrayManager {
    _tray_icon: TrayIcon,
    event_tx: Sender<TrayEvent>,
    event_rx: Receiver<TrayEvent>,
}

impl TrayManager {
    pub fn new() -> Result<Self> {
        let (event_tx, event_rx) = channel();

        let menu = Menu::new();

        let show_item = MenuItem::with_id("show", "打开主窗口", true, None);
        let settings_item = MenuItem::with_id("settings", "设置", true, None);
        let separator = PredefinedMenuItem::separator();
        let quit_item = MenuItem::with_id("quit", "退出", true, None);

        menu.append(&show_item)?;
        menu.append(&settings_item)?;
        menu.append(&separator)?;
        menu.append(&quit_item)?;

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
                if let Ok(event) = tray_receiver.try_recv() {
                    let ev = match event {
                        TrayIconEvent::Click { button: tray_icon::MouseButton::Left, .. } => {
                            Some(TrayEvent::LeftClick)
                        }
                        TrayIconEvent::Click { button: tray_icon::MouseButton::Right, .. } => {
                            Some(TrayEvent::RightClick)
                        }
                        TrayIconEvent::DoubleClick { .. } => Some(TrayEvent::DoubleClick),
                        _ => None,
                    };
                    if let Some(e) = ev {
                        let _ = tx_clone.send(e);
                    }
                }

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

    pub fn poll_event(&self) -> Option<TrayEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn set_tooltip(&self, tooltip: &str) -> Result<()> {
        self._tray_icon.set_tooltip(Some(tooltip))?;
        Ok(())
    }
}

fn create_placeholder_icon() -> Icon {
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity(width * height * 4);

    for _ in 0..(width * height) {
        rgba.push(0);
        rgba.push(120);
        rgba.push(212);
        rgba.push(255);
    }

    Icon::from_rgba(rgba, width as u32, height as u32)
        .unwrap_or_else(|_| create_minimal_icon())
}

fn create_minimal_icon() -> Icon {
    Icon::from_rgba(vec![0, 120, 212, 255], 1, 1)
        .expect("failed to create minimal icon")
}