import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';

export function InputPanel() {
  const { t } = useTranslation();
  const {
    currentState,
    setState,
    setStatus,
    setScript,
    asrActive,
    setAsrActive,
    inputText,
    setInputText,
  } = useAppStore();

  const isGenerating = currentState === 'generating';

  const handleSubmit = async () => {
    if (!inputText.trim()) return;

    setState('generating');
    setStatus(t('status.generating'));

    try {
      const result = await invoke<{ success: boolean; script?: unknown; error?: string }>('generate_script', { text: inputText });

      if (result.success && result.script) {
        setScript(result.script as Parameters<typeof setScript>[0]);
        const script = result.script as { params_schema: unknown[] };
        setState(script.params_schema?.length > 0 ? 'param_input' : 'reviewing');
      } else {
        setStatus(result.error || t('common.error'));
        setState('input');
      }
    } catch (e) {
      setStatus(`${t('common.error')}: ${e}`);
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
      if (!asrActive) {
        setStatus(t('status.asrListening'));
      } else {
        setStatus('');
      }
    } catch (e) {
      console.error('ASR toggle failed:', e);
      setStatus(`ASR ${t('common.error')}: ${e}`);
    }
  };

  return (
    <div className="input-panel">
      <div className="input-row">
        <input
          type="text"
          className="input-field"
          placeholder={t('input.placeholder')}
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={isGenerating}
        />
        <button
          className={`mic-btn ${asrActive ? 'active' : ''}`}
          onClick={toggleAsr}
          disabled={isGenerating}
          title={asrActive ? t('status.asrStopped') : '🎤'}
        >
          {asrActive ? '🔴' : '🎤'}
        </button>
      </div>

      {!isGenerating && inputText && (
        <button className="submit-btn" onClick={handleSubmit}>
          ✨ {t('input.submit')}
        </button>
      )}

      {isGenerating && (
        <div className="generating-indicator">
          <span className="spinner"></span>
          <span>{t('input.generating')}</span>
        </div>
      )}
    </div>
  );
}