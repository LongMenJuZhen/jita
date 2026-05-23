// 全局快捷键管理模块
// 基于 global-hotkey crate 实现跨平台全局热键注册与监听

use anyhow::Result;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use std::sync::mpsc::{channel, Receiver, Sender};

/// 快捷键事件
#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Triggered { id: u32 },
}

/// 快捷键管理器
/// 封装 global-hotkey 的注册和事件监听
pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    registered: Vec<HotKey>,
    event_tx: Sender<HotkeyEvent>,
    event_rx: Receiver<HotkeyEvent>,
}

impl HotkeyManager {
    /// 创建新的快捷键管理器
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        let (event_tx, event_rx) = channel();
        Ok(Self {
            manager,
            registered: Vec::new(),
            event_tx,
            event_rx,
        })
    }

    /// 注册一个全局快捷键
    /// 例如: register("ctrl+space") 注册 Ctrl+Space
    pub fn register(&mut self, shortcut: &str) -> Result<u32> {
        let hotkey = self.parse_shortcut(shortcut)?;
        self.manager.register(hotkey)?;
        let id = hotkey.id();
        self.registered.push(hotkey);
        Ok(id)
    }

    /// 解析快捷键字符串为 HotKey
    /// 支持格式: "ctrl+space", "alt+f1", "shift+meta+d", 等
    fn parse_shortcut(&self, shortcut: &str) -> Result<HotKey> {
        let lowered = shortcut.to_lowercase();
        let parts: Vec<&str> = lowered.split('+').collect();
        let mut modifiers = Modifiers::empty();
        let mut key_code = None;

        for part in parts {
            let part = part.trim();
            match part {
                "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
                "alt" | "option" => modifiers |= Modifiers::ALT,
                "shift" => modifiers |= Modifiers::SHIFT,
                "meta" | "cmd" | "command" | "win" | "super" => modifiers |= Modifiers::META,
                key => {
                    key_code = Some(self.parse_key(key)?);
                }
            }
        }

        let code = key_code.ok_or_else(|| anyhow::anyhow!("快捷键缺少按键: {}", shortcut))?;
        let hotkey = if modifiers.is_empty() {
            HotKey::new(None, code)
        } else {
            HotKey::new(Some(modifiers), code)
        };

        Ok(hotkey)
    }

    /// 解析单个按键名为 Code
    fn parse_key(&self, key: &str) -> Result<Code> {
        match key {
            "space" => Ok(Code::Space),
            "enter" | "return" => Ok(Code::Enter),
            "escape" | "esc" => Ok(Code::Escape),
            "tab" => Ok(Code::Tab),
            "backspace" => Ok(Code::Backspace),
            "delete" | "del" => Ok(Code::Delete),
            "up" => Ok(Code::ArrowUp),
            "down" => Ok(Code::ArrowDown),
            "left" => Ok(Code::ArrowLeft),
            "right" => Ok(Code::ArrowRight),
            "home" => Ok(Code::Home),
            "end" => Ok(Code::End),
            "pageup" => Ok(Code::PageUp),
            "pagedown" => Ok(Code::PageDown),
            k if k.starts_with('f') && k.len() > 1 => {
                let num: u8 = k[1..].parse()?;
                match num {
                    1 => Ok(Code::F1),
                    2 => Ok(Code::F2),
                    3 => Ok(Code::F3),
                    4 => Ok(Code::F4),
                    5 => Ok(Code::F5),
                    6 => Ok(Code::F6),
                    7 => Ok(Code::F7),
                    8 => Ok(Code::F8),
                    9 => Ok(Code::F9),
                    10 => Ok(Code::F10),
                    11 => Ok(Code::F11),
                    12 => Ok(Code::F12),
                    _ => anyhow::bail!("不支持的 F 键: {}", k),
                }
            }
            k if k.len() == 1 && k.chars().next().unwrap().is_ascii_alphabetic() => {
                let c = k.chars().next().unwrap().to_ascii_uppercase();
                match c {
                    'A' => Ok(Code::KeyA),
                    'B' => Ok(Code::KeyB),
                    'C' => Ok(Code::KeyC),
                    'D' => Ok(Code::KeyD),
                    'E' => Ok(Code::KeyE),
                    'F' => Ok(Code::KeyF),
                    'G' => Ok(Code::KeyG),
                    'H' => Ok(Code::KeyH),
                    'I' => Ok(Code::KeyI),
                    'J' => Ok(Code::KeyJ),
                    'K' => Ok(Code::KeyK),
                    'L' => Ok(Code::KeyL),
                    'M' => Ok(Code::KeyM),
                    'N' => Ok(Code::KeyN),
                    'O' => Ok(Code::KeyO),
                    'P' => Ok(Code::KeyP),
                    'Q' => Ok(Code::KeyQ),
                    'R' => Ok(Code::KeyR),
                    'S' => Ok(Code::KeyS),
                    'T' => Ok(Code::KeyT),
                    'U' => Ok(Code::KeyU),
                    'V' => Ok(Code::KeyV),
                    'W' => Ok(Code::KeyW),
                    'X' => Ok(Code::KeyX),
                    'Y' => Ok(Code::KeyY),
                    'Z' => Ok(Code::KeyZ),
                    _ => anyhow::bail!("不支持的字母键: {}", k),
                }
            }
            k if k.len() == 1 && k.chars().next().unwrap().is_ascii_digit() => {
                let c = k.chars().next().unwrap();
                match c {
                    '0' => Ok(Code::Digit0),
                    '1' => Ok(Code::Digit1),
                    '2' => Ok(Code::Digit2),
                    '3' => Ok(Code::Digit3),
                    '4' => Ok(Code::Digit4),
                    '5' => Ok(Code::Digit5),
                    '6' => Ok(Code::Digit6),
                    '7' => Ok(Code::Digit7),
                    '8' => Ok(Code::Digit8),
                    '9' => Ok(Code::Digit9),
                    _ => anyhow::bail!("不支持的数字键: {}", k),
                }
            }
            _ => anyhow::bail!("不支持的按键: {}", key),
        }
    }

    /// 启动事件监听循环（阻塞）
    /// 在独立线程中运行，将事件通过 channel 发送
    pub fn start_listener(&self) {
        let tx = self.event_tx.clone();
        std::thread::spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            loop {
                if let Ok(event) = receiver.recv() {
                    if event.state == HotKeyState::Pressed {
                        let _ = tx.send(HotkeyEvent::Triggered { id: event.id });
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
    }

    /// 检查是否有待处理的快捷键事件（非阻塞）
    pub fn poll_event(&self) -> Option<HotkeyEvent> {
        self.event_rx.try_recv().ok()
    }

    /// 注销所有已注册的快捷键
    pub fn unregister_all(&mut self) -> Result<()> {
        if !self.registered.is_empty() {
            self.manager.unregister_all(&self.registered)?;
            self.registered.clear();
        }
        Ok(())
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            let (event_tx, event_rx) = channel();
            Self {
                manager: GlobalHotKeyManager::new().expect("failed to create hotkey manager"),
                registered: Vec::new(),
                event_tx,
                event_rx,
            }
        })
    }
}