use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("jita")
}

pub fn scripts_dir() -> PathBuf {
    data_dir().join("scripts")
}

pub fn ensure_dirs() -> anyhow::Result<()> {
    std::fs::create_dir_all(data_dir())?;
    std::fs::create_dir_all(scripts_dir())?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub enum UvStatus {
    Available(String),
    Missing,
    LowVersion { current: String, required: String },
}

pub fn check_uv() -> UvStatus {
    match std::process::Command::new("uv").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout)
                .trim()
                .to_string();
            // Simple version check: extract numeric part
            let version_num = version
                .split_whitespace()
                .last()
                .unwrap_or("0.0.0")
                .to_string();

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

fn is_version_sufficient(current: &str, required: &str) -> bool {
    let parse = |s: &str| {
        s.split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect::<Vec<_>>()
    };

    let c = parse(current);
    let r = parse(required);

    for i in 0..r.len().max(c.len()) {
        let cv = c.get(i).copied().unwrap_or(0);
        let rv = r.get(i).copied().unwrap_or(0);
        if cv > rv { return true; }
        if cv < rv { return false; }
    }
    true
}

pub fn check_editor() -> Option<String> {
    std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .ok()
}

#[cfg(target_os = "windows")]
pub const PATH_SEP: &str = ";";
#[cfg(not(target_os = "windows"))]
pub const PATH_SEP: &str = ":";
