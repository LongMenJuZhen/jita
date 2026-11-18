import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Highlight, themes } from 'prism-react-renderer';
import { useAppStore } from '../store/appStore';
import { Terminal } from './Terminal';

export function ScriptPreview() {
  const { t } = useTranslation();
  const [currentTaskId, setCurrentTaskId] = useState<string | null>(null);
  const {
    script,
    scriptContent,
    isExecuting,
    runningTaskId,
    executionOutputs,
    setState,
    setExecuting,
    setTaskId,
    setStatus,
    addExecutionOutput,
    appendOutputLine,
    setOutputExitCode,
  } = useAppStore();

  if (!script) return null;

  const handleExecute = async () => {
    setExecuting(true);
    setStatus(t('status.executing'));

    try {
      const result = await invoke<{ success: boolean; task_id?: string; error?: string }>('execute_script', {
        script,
        params: {},
      });

      if (result.success && result.task_id) {
        setTaskId(result.task_id);
        setCurrentTaskId(result.task_id);
        addExecutionOutput(result.task_id);
      } else {
        setStatus(result.error || t('status.failed'));
        setExecuting(false);
      }
    } catch (e) {
      setStatus(`${t('status.error')}: ${e}`);
      setExecuting(false);
    }
  };

  const handleStop = async () => {
    if (runningTaskId) {
      try {
        await invoke('stop_script', { taskId: runningTaskId });
      } catch (e) {
        console.error('Stop failed:', e);
      }
    }
  };

  const handleCancel = () => {
    setState('input');
  };

  const getOutputForCurrentTask = () => {
    if (!currentTaskId) return { lines: [], exitCode: null as number | null };
    const output = executionOutputs.find(o => o.taskId === currentTaskId);
    return output || { lines: [], exitCode: null };
  };

  return (
    <div className="script-preview">
      <h2 className="script-name">{script.name}</h2>
      <p className="script-description">{script.description}</p>

      <Highlight
        theme={themes.nightOwl}
        code={scriptContent}
        language="python"
      >
        {({ style, tokens, getLineProps }) => (
          <pre className="code-block" style={style}>
            {tokens.map((line, i) => (
              <div key={i} {...getLineProps({ line })}>
                {line.map((token, key) => (
                  <span key={key}>{token.content}</span>
                ))}
              </div>
            ))}
          </pre>
        )}
      </Highlight>

      <div className="action-buttons">
        {!isExecuting ? (
          <button className="action-btn execute" onClick={handleExecute}>
            ▶ {t('preview.execute')}
          </button>
        ) : (
          <button className="action-btn stop" onClick={handleStop}>
            ⏹ {t('preview.stop')}
          </button>
        )}
        <button className="action-btn cancel" onClick={handleCancel}>
          {isExecuting ? t('preview.close') : t('preview.discard')}
        </button>
      </div>

      {isExecuting && (
        <div className="execution-output">
          <Terminal {...getOutputForCurrentTask()} />
        </div>
      )}
    </div>
  );
}