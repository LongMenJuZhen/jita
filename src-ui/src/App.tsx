import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from './store/appStore';
import { useTauriEvents } from './hooks/useTauriEvents';
import { InputPanel } from './components/InputPanel';
import { ScriptPreview } from './components/ScriptPreview';
import { ParamForm } from './components/ParamForm';
import { Settings } from './components/Settings';

function App() {
  const {
    currentState,
    isSettingsVisible,
    uvAvailable,
    statusText,
    openSettings,
    updateSettings,
    setUvAvailable,
    asrActive,
  } = useAppStore();

  useTauriEvents();

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
    if (statusText?.includes('成功')) return '✓';
    if (statusText?.includes('失败') || statusText?.includes('错误')) return '✗';
    return '·';
  };

  return (
    <div className="app">
      {!uvAvailable && (
        <div className="uv-warning">
          未检测到 uv，脚本执行功能不可用。请安装 uv。
        </div>
      )}

      <div className="header">
        <button className="settings-btn" onClick={openSettings}>
          ⚙ 设置
        </button>
      </div>

      <div className="main-content">
        {isSettingsVisible ? <Settings /> : renderMainContent()}
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