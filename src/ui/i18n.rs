// 国际化模块
// 简单的 Key-Value 翻译系统

use std::collections::HashMap;
use std::sync::OnceLock;

static MESSAGES: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

const FALLBACK_MESSAGES: &[(&str, &str)] = &[
    ("jita", "Jita"),
    ("ai_generating", "正在生成脚本..."),
    ("generation_failed", "生成失败: {error}"),
    ("task_started", "任务已启动: {task_id}"),
    ("execution_success", "执行成功"),
    ("execution_failed", "执行失败: {error}"),
    ("execution_failed_exit_code", "执行失败，退出码: {code}"),
    ("execution_error", "执行出错"),
    ("state_error", "状态错误"),
    ("task_stopped", "任务已停止"),
    ("stop_failed", "停止失败: {error}"),
    ("discarded", "已放弃"),
    ("canceled", "已取消"),
    ("settings_saved", "设置已保存"),
];

pub fn init() {
    let locale = detect_locale();
    load_messages(locale);
}

fn detect_locale() -> &'static str {
    if let Ok(lang) = std::env::var("JITA_LANG") {
        if !lang.is_empty() {
            return Box::leak(lang.into_boxed_str());
        }
    }

    if let Ok(lang) = std::env::var("LANG") {
        if !lang.is_empty() {
            let lang = lang.split('.').next().unwrap_or(&lang);
            let lang = lang.split('_').next().unwrap_or(lang);
            return Box::leak(lang.to_string().into_boxed_str());
        }
    }

    "en"
}

fn load_messages(_locale: &str) {
    let map = FALLBACK_MESSAGES
        .iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    let _ = MESSAGES.set(map);
}

pub fn t(key: &str) -> String {
    MESSAGES
        .get()
        .and_then(|m| m.get(key))
        .map(|s| s.to_string())
        .unwrap_or_else(|| key.to_string())
}

pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    let template = MESSAGES
        .get()
        .and_then(|m| m.get(key))
        .map(|s| *s)
        .unwrap_or(key);

    let mut result = template.to_string();
    for (k, v) in args {
        result = result.replace(&format!("{{{}}}", k), v);
    }
    result
}