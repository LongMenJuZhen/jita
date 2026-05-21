//! 国际化 (i18n) 模块
//! 使用 fluent-bundle (Mozilla Fluent) 提供翻译支持

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::RwLock;
use unic_langid::LanguageIdentifier;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    ZhCN,
    En,
}

impl Locale {
    fn code(&self) -> u8 {
        match self {
            Locale::ZhCN => 0,
            Locale::En => 1,
        }
    }

    fn from_str(s: &str) -> Self {
        if s.starts_with("zh") {
            Locale::ZhCN
        } else {
            Locale::En
        }
    }
}

static CURRENT_LOCALE: AtomicU8 = AtomicU8::new(0);

#[inline]
fn locale() -> Locale {
    match CURRENT_LOCALE.load(Ordering::Relaxed) {
        0 => Locale::ZhCN,
        _ => Locale::En,
    }
}

/// 初始化语言环境（自动检测或使用指定 locale）
pub fn init() {
    let locale = std::env::var("JITA_LANG")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    set_locale(&locale);
    // 同步到 Slint UI
    let _ = slint::select_bundled_translation(lang_code());
}

/// 设置当前语言
pub fn set_locale(loc: &str) {
    CURRENT_LOCALE.store(Locale::from_str(loc).code(), Ordering::Relaxed);
}

/// 获取当前语言代码（供 Slint UI 使用）
pub fn lang_code() -> &'static str {
    match locale() {
        Locale::ZhCN => "zh",
        Locale::En => "en",
    }
}

fn make_bundle(primary_locale: Locale) -> FluentBundle<FluentResource> {
    let zh_ftl = include_str!("../locales/zh-CN.ftl");
    let en_ftl = include_str!("../locales/en.ftl");

    let mut bundle = FluentBundle::new(vec![
        LanguageIdentifier::from_bytes(b"zh-CN").expect("invalid zh-CN"),
        LanguageIdentifier::from_bytes(b"en").expect("invalid en"),
    ]);

    // FluentBundle 不允许覆盖已存在的消息 ID，
    // 所以只能添加主语言的资源，fallback 只能通过 locale 切换实现
    match primary_locale {
        Locale::ZhCN => {
            let res = FluentResource::try_new(zh_ftl.to_string())
                .expect("Failed to parse zh-CN.ftl");
            bundle.add_resource(res).expect("Failed to add zh-CN resource");
        }
        Locale::En => {
            let res = FluentResource::try_new(en_ftl.to_string())
                .expect("Failed to parse en.ftl");
            bundle.add_resource(res).expect("Failed to add en resource");
        }
    }

    bundle
}

/// 简单翻译（无参数）
pub fn t(key: &str) -> String {
    t_args(key, &[])
}

/// 带参数的翻译
/// args: [("name", "value"), ...]
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    // 使用 RwLock 只在 locale 改变时重建 bundle
    thread_local! {
        static CACHED_LOCALE: AtomicU8 = AtomicU8::new(255); // 255 = 未初始化
        static BUNDLE: RwLock<FluentBundle<FluentResource>> = RwLock::new(make_bundle(Locale::ZhCN));
    }

    CACHED_LOCALE.with(|cached_locale| {
        let current_locale = locale();
        let locale_code = current_locale.code();

        // 检测 locale 是否变化
        if cached_locale.load(Ordering::Relaxed) != locale_code {
            cached_locale.store(locale_code, Ordering::Relaxed);
            let new_bundle = make_bundle(current_locale);
            BUNDLE.with(|bundle| {
                *bundle.write().unwrap() = new_bundle;
            });
        }

        // 使用缓存的 bundle
        BUNDLE.with(|bundle| {
            let bundle = bundle.read().unwrap();
            if let Some(msg) = bundle.get_message(key) {
                if let Some(pattern) = msg.value() {
                    let mut fluent_args = FluentArgs::new();
                    for (k, v) in args {
                        fluent_args.set(*k, FluentValue::String(v.to_string().into()));
                    }
                    let mut errors = vec![];
                    let result = bundle.format_pattern(
                        pattern,
                        if args.is_empty() { None } else { Some(&fluent_args) },
                        &mut errors,
                    );
                    return result.to_string();
                }
            }
            key.to_string()
        })
    })
}