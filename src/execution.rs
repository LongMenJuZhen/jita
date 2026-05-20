// 脚本执行器模块
// 使用 uv run 执行 Python 脚本，或 shell 执行命令

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// 输出行类型
#[derive(Debug)]
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}

/// 脚本执行器
pub struct ScriptExecutor;

impl ScriptExecutor {
    pub fn new() -> Self {
        Self
    }

    /// 执行脚本
    /// 返回子进程和输出流接收端
    pub async fn execute(
        &self,
        script: &crate::script::Script,
        params: &HashMap<String, String>,
        context: &crate::script::ExecutionContext,
    ) -> Result<(Child, mpsc::UnboundedReceiver<OutputLine>)> {
        // 将脚本写入磁盘
        let script_path = self.write_script_to_disk(script).await?;

        // 构建环境变量
        let env_vars = self.build_env_vars(script, params, context);

        // 根据运行时类型选择命令
        let (cmd, args) = match script.runtime {
            crate::script::ScriptRuntime::PythonPep723 => {
                // uv run script.py
                ("uv", vec!["run".to_string(), script_path.to_string_lossy().to_string()])
            }
            crate::script::ScriptRuntime::Shell => {
                // 根据目标 shell 选择命令
                let shell = script.shell_target.clone().unwrap_or_default();
                match shell {
                    crate::script::ShellTarget::Bash => ("bash", vec![script_path.to_string_lossy().to_string()]),
                    crate::script::ShellTarget::Pwsh => ("pwsh", vec!["-File".to_string(), script_path.to_string_lossy().to_string()]),
                    crate::script::ShellTarget::Sh => ("sh", vec![script_path.to_string_lossy().to_string()]),
                }
            }
        };

        // 启动进程
        let mut command = Command::new(cmd);
        command
            .args(&args)
            .envs(&env_vars)
            .current_dir(&context.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command.spawn()?;

        // 获取输出流
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        // 创建 channel
        let (tx, rx) = mpsc::unbounded_channel::<OutputLine>();

        // 异步读取 stdout
        let tx_stdout = tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stdout.send(OutputLine::Stdout(line));
            }
        });

        // 异步读取 stderr
        let tx_stderr = tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stderr.send(OutputLine::Stderr(line));
            }
        });

        Ok((child, rx))
    }

    /// 将脚本写入磁盘
    /// 脚本存储在 ~/.local/share/jita/scripts/ 目录
    async fn write_script_to_disk(&self, script: &crate::script::Script) -> Result<PathBuf> {
        use crate::utils::scripts_dir;
        use std::fs::Permissions;
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let dir = scripts_dir();
        tokio::fs::create_dir_all(&dir).await?;

        // 根据运行时确定文件扩展名
        let ext = match script.runtime {
            crate::script::ScriptRuntime::PythonPep723 => "py",
            crate::script::ScriptRuntime::Shell => {
                match script.shell_target {
                    Some(crate::script::ShellTarget::Pwsh) => "ps1",
                    _ => "sh",
                }
            }
        };

        let path = dir.join(format!("{}.{}", script.id, ext));
        tokio::fs::write(&path, &script.content).await?;

        // Unix 上设置可执行权限
        #[cfg(unix)]
        {
            let perms = Permissions::from_mode(0o755);
            tokio::fs::set_permissions(&path, perms).await?;
        }

        Ok(path)
    }

    /// 构建环境变量
    /// 将参数转换为环境变量注入脚本
    fn build_env_vars(
        &self,
        script: &crate::script::Script,
        params: &HashMap<String, String>,
        context: &crate::script::ExecutionContext,
    ) -> HashMap<String, String> {
        use crate::utils::PATH_SEP;
        use crate::script::WidgetType;

        let mut env = context.env_vars.clone();

        // 注入每个参数为环境变量
        for decl in &script.params_schema {
            if let Some(value) = params.get(&decl.name) {
                // 多文件参数用平台特定分隔符连接
                let env_value = match &decl.widget {
                    WidgetType::File { multiple: true, .. } => {
                        value.split(',').collect::<Vec<_>>().join(PATH_SEP)
                    }
                    _ => value.clone(),
                };
                env.insert(decl.name.clone(), env_value);
            }
        }

        env
    }
}
