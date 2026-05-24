import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';

export function ScriptPreview() {
  const { script, scriptContent, isExecuting, runningTaskId, setState, setExecuting, setTaskId, setStatus } = useAppStore();

  if (!script) return null;

  const handleExecute = async () => {
    setExecuting(true);
    setStatus('正在执行...');

    try {
      const result = await invoke<{ success: boolean; task_id?: string; error?: string }>('execute_script', {
        script,
        params: {},
      });

      if (result.success && result.task_id) {
        setTaskId(result.task_id);
      } else {
        setStatus(result.error || '执行失败');
        setExecuting(false);
      }
    } catch (e) {
      setStatus(`执行错误: ${e}`);
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

  return (
    <div className="script-preview">
      <h2 className="script-name">{script.name}</h2>
      <p className="script-description">{script.description}</p>

      <div className="code-block">
        <pre>{scriptContent}</pre>
      </div>

      <div className="action-buttons">
        {!isExecuting ? (
          <button className="action-btn execute" onClick={handleExecute}>
            执行
          </button>
        ) : (
          <button className="action-btn stop" onClick={handleStop}>
            停止
          </button>
        )}
        <button className="action-btn cancel" onClick={handleCancel}>
          {isExecuting ? '关闭' : '放弃'}
        </button>
      </div>
    </div>
  );
}