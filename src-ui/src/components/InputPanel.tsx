import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';

export function InputPanel() {
  const { currentState, setState, setStatus, setScript, asrActive, setAsrActive, inputText, setInputText } = useAppStore();

  const handleSubmit = async () => {
    if (!inputText.trim()) return;

    setState('generating');
    setStatus('正在生成脚本...');

    try {
      const result = await invoke<{ success: boolean; script?: unknown; error?: string }>('generate_script', { text: inputText });

      if (result.success && result.script) {
        setScript(result.script as Parameters<typeof setScript>[0]);
        const script = result.script as { params_schema: unknown[] };
        setState(script.params_schema?.length > 0 ? 'param_input' : 'reviewing');
      } else {
        setStatus(result.error || '生成失败');
        setState('input');
      }
    } catch (e) {
      setStatus(`生成错误: ${e}`);
      setState('input');
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const toggleAsr = async () => {
    try {
      await invoke('toggle_asr');
      setAsrActive(!asrActive);
    } catch (e) {
      console.error('ASR toggle failed:', e);
    }
  };

  return (
    <div className="input-panel">
      <div className="input-row">
        <input
          type="text"
          className="input-field"
          placeholder="描述你的需求，例如：把当前目录所有 jpg 转成 png"
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={currentState === 'generating'}
        />
        <button
          className={`mic-btn ${asrActive ? 'active' : ''}`}
          onClick={toggleAsr}
          disabled={currentState === 'generating'}
        >
          Mic
        </button>
      </div>

      {currentState === 'input' && inputText && (
        <button className="submit-btn" onClick={handleSubmit}>
          生成脚本
        </button>
      )}

      {currentState === 'generating' && (
        <p className="generating-text">AI 生成中...</p>
      )}
    </div>
  );
}