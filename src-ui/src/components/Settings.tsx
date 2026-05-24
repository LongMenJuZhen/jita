import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';
import type { AppSettings } from '../types';

export function Settings() {
  const { settings, updateSettings, closeSettings } = useAppStore();
  const [localSettings, setLocalSettings] = useState<AppSettings>(settings);

  useEffect(() => {
    setLocalSettings(settings);
  }, [settings]);

  const handleChange = (key: keyof AppSettings, value: string | boolean) => {
    setLocalSettings((prev) => ({ ...prev, [key]: value }));
  };

  const handleSave = async () => {
    try {
      await invoke('save_settings', { settings: localSettings });
      updateSettings(localSettings);
      closeSettings();
    } catch (e) {
      console.error('Save failed:', e);
    }
  };

  const openModelsFolder = async () => {
    try {
      await invoke('open_models_folder');
    } catch (e) {
      console.error('Open folder failed:', e);
    }
  };

  return (
    <div className="settings-panel">
      <h2>设置</h2>

      <div className="settings-row">
        <label>API Key</label>
        <input
          type="password"
          value={localSettings.api_key}
          onChange={(e) => handleChange('api_key', e.target.value)}
        />
      </div>

      <div className="settings-row">
        <label>API Base URL</label>
        <input
          type="text"
          value={localSettings.api_base || ''}
          onChange={(e) => handleChange('api_base', e.target.value)}
          placeholder="可选"
        />
      </div>

      <div className="settings-row">
        <label>模型</label>
        <input
          type="text"
          value={localSettings.model}
          onChange={(e) => handleChange('model', e.target.value)}
        />
      </div>

      <div className="settings-row">
        <label>ASR 模型路径</label>
        <div style={{ display: 'flex', gap: 8 }}>
          <input
            type="text"
            value={localSettings.asr_model_path || ''}
            onChange={(e) => handleChange('asr_model_path', e.target.value)}
            readOnly
            placeholder="点击下方按钮打开"
          />
          <button className="settings-btn" onClick={openModelsFolder}>
            打开
          </button>
        </div>
      </div>

      <div className="settings-actions">
        <button className="save" onClick={handleSave}>
          保存
        </button>
        <button className="cancel" onClick={closeSettings}>
          取消
        </button>
      </div>
    </div>
  );
}