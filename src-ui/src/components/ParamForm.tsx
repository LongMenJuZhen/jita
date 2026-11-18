import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';

export function ParamForm() {
  const { t } = useTranslation();
  const { script, params, setParams, setState, setStatus } = useAppStore();
  const [localParams, setLocalParams] = useState<Record<string, string>>(params);

  if (!script) return null;

  const handleChange = (name: string, value: string) => {
    setLocalParams((prev) => ({ ...prev, [name]: value }));
  };

  const handleSubmit = async () => {
    setParams(localParams);
    setState('reviewing');
    setStatus(t('status.executing'));

    try {
      const result = await invoke<{ success: boolean; task_id?: string; error?: string }>('execute_script', {
        script,
        params: localParams,
      });

      if (!result.success) {
        setStatus(result.error || t('status.failed'));
      }
    } catch (e) {
      setStatus(`${t('status.error')}: ${e}`);
    }
  };

  const handleCancel = () => {
    setState('input');
  };

  return (
    <div className="param-form">
      <h2>{t('params.title')}</h2>

      <div className="param-list">
        {script.params_schema.slice(0, 6).map((param) => (
          <div key={param.name} className="param-field">
            <label>
              {param.label}
              {param.required && <span className="required"> *</span>}
            </label>
            <input
              type="text"
              value={localParams[param.name] || ''}
              onChange={(e) => handleChange(param.name, e.target.value)}
              placeholder={param.default || ''}
            />
          </div>
        ))}
      </div>

      <div className="action-buttons">
        <button className="action-btn execute" onClick={handleSubmit}>
          ▶ {t('params.submit')}
        </button>
        <button className="action-btn cancel" onClick={handleCancel}>
          {t('common.cancel')}
        </button>
      </div>
    </div>
  );
}