use crate::script::{ExecutionContext, ParamDeclaration, Script, ScriptRuntime, ShellTarget};
use crate::utils;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

pub struct ScriptExecutor;

pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}

impl ScriptExecutor {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        script: &Script,
        params: &HashMap<String, String>,
        context: &ExecutionContext,
    ) -> Result<(Child, mpsc::UnboundedReceiver<OutputLine>)> {
        let script_path = self.write_script_to_disk(script).await?;
        let env_vars = self.build_env_vars(&script.params_schema, params, context);

        let (cmd, args) = match script.runtime {
            ScriptRuntime::PythonPep723 => {
                ("uv", vec!["run".to_string(), script_path.to_string_lossy().to_string()])
            }
            ScriptRuntime::Shell => {
                let shell = script.shell_target.clone().unwrap_or_default();
                match shell {
                    ShellTarget::Bash => ("bash", vec![script_path.to_string_lossy().to_string()]),
                    ShellTarget::Pwsh => ("pwsh", vec!["-File".to_string(), script_path.to_string_lossy().to_string()]),
                    ShellTarget::Sh => ("sh", vec![script_path.to_string_lossy().to_string()]),
                }
            }
        };

        let mut command = Command::new(cmd);
        command
            .args(&args)
            .envs(&env_vars)
            .current_dir(&context.cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        let mut child = command.spawn()?;

        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        let (tx, rx) = mpsc::unbounded_channel::<OutputLine>();

        let tx_stdout = tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stdout.send(OutputLine::Stdout(line));
            }
        });

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

    async fn write_script_to_disk(&self, script: &Script) -> Result<PathBuf> {
        let scripts_dir = utils::scripts_dir();
        tokio::fs::create_dir_all(&scripts_dir).await?;

        let ext = match script.runtime {
            ScriptRuntime::PythonPep723 => "py",
            ScriptRuntime::Shell => match script.shell_target {
                Some(ShellTarget::Pwsh) => "ps1",
                _ => "sh",
            },
        };

        let path = scripts_dir.join(format!("{}.{}", script.id, ext));
        tokio::fs::write(&path, &script.content).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&path).await?.permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&path, perms).await?;
        }

        Ok(path)
    }

    fn build_env_vars(
        &self,
        params_schema: &[ParamDeclaration],
        params: &HashMap<String, String>,
        context: &ExecutionContext,
    ) -> HashMap<String, String> {
        let mut env = context.env_vars.clone();

        for decl in params_schema {
            if let Some(value) = params.get(&decl.name) {
                let env_value = match &decl.widget {
                    crate::script::WidgetType::File { multiple: true, .. } => {
                        // Multiple files: join with platform separator
                        value.split(',').collect::<Vec<_>>().join(utils::PATH_SEP)
                    }
                    _ => value.clone(),
                };
                env.insert(decl.name.clone(), env_value);
            }
        }

        env
    }
}
