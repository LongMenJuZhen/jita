import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';
import type { AppSettings } from '../types';

export function Settings() {
  const { t, i18n } = useTranslation();
  const { settings, updateSettings, closeSettings } = useAppStore();
  const [localSettings, setLocalSettings] = useState<AppSettings>(settings);
  const [selectedLang, setSelectedLang] = useState(i18n.language);

  useEffect(() => {
    setLocalSettings(settings);
  }, [settings]);

  const handleChange = (key: keyof AppSettings, value: string | boolean) => {
    setLocalSettings((prev) => ({ ...prev, [key]: value }));
  };

  const handleLanguageChange = (lang: string) => {
    setSelectedLang(lang);
    i18n.changeLanguage(lang);
    localStorage.setItem('i18nextLng', lang);
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

  const languages = [
    { code: 'zh', name: t('languages.zh') },
    { code: 'en', name: t('languages.en') },
    { code: 'ja', name: t('languages.ja') },
  ];

  return (
    <div className="settings-panel">
      <h2>{t('settings.title')}</h2>

      <div className="settings-row">
        <label>{t('settings.language')}</label>
        <div className="language-selector">
          {languages.map((lang) => (
            <button
              key={lang.code}
              className={`lang-btn ${selectedLang === lang.code ? 'active' : ''}`}
              onClick={() => handleLanguageChange(lang.code)}
            >
              {lang.name}
            </button>
          ))}
        </div>
      </div>

      <div className="settings-row">
        <label>{t('settings.apiKey')}</label>
        <input
          type="password"
          value={localSettings.api_key}
          onChange={(e) => handleChange('api_key', e.target.value)}
        />
      </div>

      <div className="settings-row">
        <label>{t('settings.apiBase')}</label>
        <input
          type="text"
          value={localSettings.api_base || ''}
          onChange={(e) => handleChange('api_base', e.target.value)}
          placeholder={t('settings.apiBasePlaceholder')}
        />
      </div>

      <div className="settings-row">
        <label>{t('settings.model')}</label>
        <input
          type="text"
          value={localSettings.model}
          onChange={(e) => handleChange('model', e.target.value)}
        />
      </div>

      <div className="settings-row">
        <label>{t('settings.asrModel')}</label>
        <div className="input-with-btn">
          <input
            type="text"
            value={localSettings.asr_model_path || ''}
            onChange={(e) => handleChange('asr_model_path', e.target.value)}
            readOnly
            placeholder={t('settings.openFolder')}
          />
          <button className="settings-btn" onClick={openModelsFolder}>
            📁 {t('settings.openFolder')}
          </button>
        </div>
      </div>

      <div className="settings-actions">
        <button className="save" onClick={handleSave}>
          💾 {t('settings.save')}
        </button>
        <button className="cancel" onClick={closeSettings}>
          {t('settings.cancel')}
        </button>
      </div>
    </div>
  );
}