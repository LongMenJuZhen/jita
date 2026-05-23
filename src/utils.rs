// 工具函数模块
// 提供数据目录、脚本目录、uv 检测等通用功能
// 这个文件不应该超过300行
use std::path::PathBuf;

/// 获取 Jita 数据目录
/// Windows: %APPDATA%/jita
/// macOS:   ~/Library/Application Support/jita
/// Linux:   ~/.local/share/jita
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("jita")
}

/// 获取脚本存储目录
/// 位于 data_dir()/scripts/
pub fn scripts_dir() -> PathBuf {
    data_dir().join("scripts")
}

/// 获取 ASR 模型目录
/// 位于 data_dir()/models/
pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

/// 确保必要的目录存在
pub fn ensure_dirs() -> anyhow::Result<()> {
    std::fs::create_dir_all(data_dir())?;
    std::fs::create_dir_all(scripts_dir())?;
    std::fs::create_dir_all(models_dir())?;
    Ok(())
}

// uv 可用性状态
#[derive(Debug, Clone, PartialEq)]
pub enum UvStatus {
    Available(String),                                // 可用，值为版本字符串
    Missing,                                          // 未安装
    LowVersion { current: String, required: String }, // 版本过低
}

/// 检查 uv 是否可用及其版本
pub fn check_uv() -> UvStatus {
    match std::process::Command::new("uv").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            // 提取版本号（取最后一个空格分隔的部分）
            let version_num = version
                .split_whitespace()
                .last()
                .unwrap_or("0.0.0")
                .to_string();

            // 检查版本是否满足最低要求（0.4.0）
            if is_version_sufficient(&version_num, "0.4.0") {
                UvStatus::Available(version)
            } else {
                UvStatus::LowVersion {
                    current: version,
                    required: "0.4.0".to_string(),
                }
            }
        }
        _ => UvStatus::Missing,
    }
}

/// 比较版本号是否满足最低要求
/// 例如: is_version_sufficient("0.5.0", "0.4.0") => true
fn is_version_sufficient(current: &str, required: &str) -> bool {
    let parse = |s: &str| {
        s.split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect::<Vec<_>>()
    };

    let c = parse(current);
    let r = parse(required);

    // 逐段比较版本号
    for i in 0..r.len().max(c.len()) {
        let cv = c.get(i).copied().unwrap_or(0);
        let rv = r.get(i).copied().unwrap_or(0);
        if cv > rv {
            return true;
        }
        if cv < rv {
            return false;
        }
    }
    true
}

/// 获取用户首选的编辑器
/// 依次检查 EDITOR 和 VISUAL 环境变量
pub fn check_editor() -> Option<String> {
    std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .ok()
}

// 平台特定路径分隔符
// Windows 使用分号，Unix 使用冒号（与 PATH 环境变量一致）
#[cfg(target_os = "windows")]
pub const PATH_SEP: &str = ";";
#[cfg(not(target_os = "windows"))]
pub const PATH_SEP: &str = ":";
