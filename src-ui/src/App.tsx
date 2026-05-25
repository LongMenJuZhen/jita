import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from './store/appStore';
import { useTauriEvents } from './hooks/useTauriEvents';
import { InputPanel } from './components/InputPanel';
import { ScriptPreview } from './components/ScriptPreview';
import { ParamForm } from './components/ParamForm';

function App() {
  const { t } = useTranslation();
  const {
    currentState,
    uvAvailable,
    statusText,
    updateSettings,
    setUvAvailable,
    asrActive,
  } = useAppStore();

  useTauriEvents();

  const openSettings = async () => {
    try {
      await invoke('open_settings_window');
    } catch (e) {
      console.error('Failed to open settings:', e);
    }
  };

  useEffect(() => {
    const initApp = async () => {
      try {
        const settings = await invoke<{
          api_key: string;
          api_base?: string;
          model: string;
        }>('get_settings');

        if (settings) {
          updateSettings(settings);
        }

        const uvStatus = await invoke<boolean>('check_uv');
        setUvAvailable(uvStatus);
      } catch (e) {
        console.error('Init failed:', e);
      }
    };

    initApp();
  }, [updateSettings, setUvAvailable]);

  const renderMainContent = () => {
    switch (currentState) {
      case 'input':
      case 'generating':
        return <InputPanel />;
      case 'reviewing':
        return <ScriptPreview />;
      case 'param_input':
        return <ParamForm />;
      default:
        return null;
    }
  };

  const getStatusIcon = () => {
    if (asrActive) return '🎤';
    if (currentState === 'generating') return '✨';
    if (statusText?.includes('成功') || statusText?.includes('Success')) return '✓';
    if (statusText?.includes('失败') || statusText?.includes('错误') || statusText?.includes('Error') || statusText?.includes('failed')) return '✗';
    return '·';
  };

  return (
    <div className="app">
      {!uvAvailable && (
        <div className="uv-warning">
          {t('status.uvUnavailable')}
        </div>
      )}

      <div className="header">
        <button className="settings-btn" onClick={openSettings}>
          ⚙ {t('settings.title')}
        </button>
      </div>

      <div className="main-content">
        {renderMainContent()}
      </div>

      {statusText && (
        <div className="status-bar">
          <span>{getStatusIcon()}</span> {statusText}
        </div>
      )}
    </div>
  );
}

export { App };