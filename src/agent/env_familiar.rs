// 环境感知模块
// 检测本地包管理器，索引已安装的命令行工具
//
// 检测流程:
// 1. 检测白名单中的包管理器是否存在（uv, pnpm 等）
// 2. 获取已安装的工具列表
// 3. 获取工具的版本和帮助信息
// 4. 与数据库缓存对比，只处理新工具

use crate::agent::db::Database;
use anyhow::Result;
use std::process::Command;
use tokio::sync::Mutex;
use std::sync::Arc;

/// 白名单中的包管理器
const TOOL_WHITELIST: &[&str] = &["uv", "pnpm"];

/// 包管理器检测结果
#[derive(Debug, Clone)]
pub struct PackageManager {
    pub name: String,
    pub version: String,
    pub available: bool,
}

/// 已安装的工具信息
#[derive(Debug, Clone)]
pub struct InstalledTool {
    pub name: String,
    pub package_manager: String,
    pub version: Option<String>,
    pub help_text: String,
}

/// 检测系统中安装的包管理器
pub fn detect_package_managers() -> Vec<PackageManager> {
    let mut managers = Vec::new();

    for name in TOOL_WHITELIST {
        if let Some(version) = check_command_exists(name) {
            managers.push(PackageManager {
                name: name.to_string(),
                version,
                available: true,
            });
        } else {
            managers.push(PackageManager {
                name: name.to_string(),
                version: String::new(),
                available: false,
            });
        }
    }

    managers
}

/// 检测命令是否存在，返回版本号（如果能获取）
fn check_command_exists(cmd: &str) -> Option<String> {
    // 尝试 --version
    let output = Command::new(cmd)
        .arg("--version")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Some(version);
        }
    }

    // 尝试 -v
    let output = Command::new(cmd)
        .arg("-v")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Some(version);
        }
    }

    None
}

/// 检测 uv 工具列表
/// 返回已安装的 uv tool 列表
pub fn get_uv_tools() -> Result<Vec<InstalledTool>> {
    let output = Command::new("uv")
        .args(["tool", "list"])
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut tools = Vec::new();

    // 解析 uv tool list 输出
    // 格式类似:
    // black v24.8.0 [executable: black]
    // ruff v0.6.9 [executable: ruff]
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 跳过 "No tools installed" 等提示
        if line.starts_with("No ") || line.starts_with("help") || line.starts_with("Usage") {
            continue;
        }

        // 解析 "name vversion [executable: x]"
        if let Some((name, rest)) = line.split_once(' ') {
            let name = name.to_string();

            // 提取版本
            let version = rest
                .trim_start_matches('v')
                .split_whitespace()
                .next()
                .map(|s| s.to_string());

            // 获取帮助信息
            let help_text = get_tool_help(&name).unwrap_or_default();

            tools.push(InstalledTool {
                name,
                package_manager: "uv".to_string(),
                version,
                help_text,
            });
        }
    }

    Ok(tools)
}

/// 获取工具的帮助信息
fn get_tool_help(tool_name: &str) -> Result<String> {
    // 优先尝试 --help
    let output = Command::new(tool_name)
        .arg("--help")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    // 尝试 -h
    let output = Command::new(tool_name)
        .arg("-h")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    Ok(String::new())
}

/// 同步检测 pnpm 工具列表（返回基本信息，不包含详细 help）
pub fn get_pnpm_tools() -> Result<Vec<InstalledTool>> {
    let output = Command::new("pnpm")
        .args(["list", "-g", "--depth=0"])
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut tools = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();

        // 跳过表头和空行
        if line.is_empty() || line.starts_with("Legend:") || line.contains("dependencies") {
            continue;
        }

        // 解析 "package@version" 格式
        if let Some((name, version)) = line.rsplit_once('@') {
            let name = name.trim();
            if !name.is_empty() && !name.starts_with('-') {
                let help_text = get_tool_help(name).unwrap_or_default();
                tools.push(InstalledTool {
                    name: name.to_string(),
                    package_manager: "pnpm".to_string(),
                    version: Some(version.trim().to_string()),
                    help_text,
                });
            }
        }
    }

    Ok(tools)
}

/// 获取所有已安装的工具
pub fn get_all_tools() -> Result<Vec<InstalledTool>> {
    let mut all_tools = Vec::new();

    // 获取 uv 工具
    if let Ok(uv_tools) = get_uv_tools() {
        all_tools.extend(uv_tools);
    }

    // 获取 pnpm 全局包
    if let Ok(pnpm_tools) = get_pnpm_tools() {
        all_tools.extend(pnpm_tools);
    }

    Ok(all_tools)
}

/// 索引工具（异步）
/// 检测新安装的工具，获取帮助信息，生成嵌入
/// 此函数应在后台任务中调用
pub async fn index_tools(
    db: Arc<Mutex<Database>>,
    agent: &crate::agent::AgentClient,
) -> Result<usize> {
    let managers = detect_package_managers();
    let mut new_tools_count = 0;

    // 收集所有新工具
    let mut tools_to_index: Vec<InstalledTool> = Vec::new();

    // 检查 uv
    if managers.iter().any(|m| m.name == "uv" && m.available) {
        if let Ok(tools) = get_uv_tools() {
            let db_guard = db.lock().await;
            for tool in tools {
                if db_guard.get_tool_cache(&tool.name)?.is_none() {
                    tools_to_index.push(tool);
                }
            }
        }
    }

    // 检查 pnpm
    if managers.iter().any(|m| m.name == "pnpm" && m.available) {
        if let Ok(tools) = get_pnpm_tools() {
            let db_guard = db.lock().await;
            for tool in tools {
                if db_guard.get_tool_cache(&tool.name)?.is_none() {
                    tools_to_index.push(tool);
                }
            }
        }
    }

    // 处理新工具
    for tool in tools_to_index {
        // 生成 AI 摘要
        let summary = agent
            .summarize_tool(&tool.name, &tool.help_text)
            .await
            .unwrap_or_default();

        // 生成使用指南
        let guide = agent
            .generate_usage_guide(&tool.name, &tool.help_text)
            .await
            .unwrap_or_default();

        // 计算嵌入
        let description = format!("{}: {}", tool.name, summary);
        let embedding = crate::agent::embedding::embed_text(&description).ok();

        // 存入数据库
        let db_guard = db.lock().await;
        db_guard.insert_tool_cache(
            &tool.name,
            &tool.version.unwrap_or_default(),
            &tool.help_text,
            &summary,
            &guide,
            embedding.as_deref(),
        )?;
        new_tools_count += 1;

        // 小延迟避免 API 限流
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    Ok(new_tools_count)
}

/// 获取工具摘要列表
/// 格式化为字符串，供 LLM 使用
pub fn format_tools_summary(tools: &[(String, String)]) -> String {
    if tools.is_empty() {
        return "无可用工具".to_string();
    }

    tools
        .iter()
        .map(|(name, summary)| format!("- {}: {}", name, summary))
        .collect::<Vec<_>>()
        .join("\n")
}