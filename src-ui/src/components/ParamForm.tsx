import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';

export function ParamForm() {
  const { script, params, setParams, setState, setStatus } = useAppStore();
  const [localParams, setLocalParams] = useState<Record<string, string>>(params);

  if (!script) return null;

  const handleChange = (name: string, value: string) => {
    setLocalParams((prev) => ({ ...prev, [name]: value }));
  };

  const handleSubmit = async () => {
    setParams(localParams);
    setState('reviewing');

    try {
      const result = await invoke<{ success: boolean; task_id?: string; error?: string }>('execute_script', {
        script,
        params: localParams,
      });

      if (!result.success) {
        setStatus(result.error || '执行失败');
      }
    } catch (e) {
      setStatus(`执行错误: ${e}`);
    }
  };

  const handleCancel = () => {
    setState('input');
  };

  return (
    <div className="param-form">
      <h2>填写参数</h2>

      <div className="param-list">
        {script.params_schema.slice(0, 6).map((param) => (
          <div key={param.name} className="param-field">
            <label>
              {param.label}
              {param.required && ' *'}
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
          提交并执行
        </button>
        <button className="action-btn cancel" onClick={handleCancel}>
          取消
        </button>
      </div>
    </div>
  );
}