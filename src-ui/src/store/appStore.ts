import { create } from 'zustand';
import type { Script, AppSettings, WindowState } from '../types';

interface AppState {
  // 状态
  currentState: WindowState;

  // 脚本数据
  script: Script | null;
  scriptContent: string;

  // 执行状态
  isExecuting: boolean;
  runningTaskId: string;

  // 参数
  params: Record<string, string>;

  // 输入文本（用于 ASR）
  inputText: string;

  // ASR
  asrActive: boolean;

  // 设置
  isSettingsVisible: boolean;
  settings: AppSettings;

  // 系统状态
  statusText: string;
  uvAvailable: boolean;

  // 操作
  setState: (state: WindowState) => void;
  setScript: (script: Script | null) => void;
  setScriptContent: (content: string) => void;
  appendScriptContent: (content: string) => void;
  setParams: (params: Record<string, string>) => void;
  setExecuting: (executing: boolean) => void;
  setTaskId: (taskId: string) => void;
  setStatus: (text: string) => void;
  setAsrActive: (active: boolean) => void;
  openSettings: () => void;
  closeSettings: () => void;
  updateSettings: (settings: Partial<AppSettings>) => void;
  setUvAvailable: (available: boolean) => void;
  setInputText: (text: string) => void;
  reset: () => void;
}

const initialState = {
  currentState: 'input' as WindowState,
  script: null,
  scriptContent: '',
  isExecuting: false,
  runningTaskId: '',
  params: {},
  asrActive: false,
  isSettingsVisible: false,
  settings: {
    api_key: '',
    api_base: undefined,
    model: 'claude-sonnet-4-6',
    hotkey: 'ctrl+space',
    asr_enabled: true,
    asr_model_path: undefined,
  },
  statusText: '',
  uvAvailable: true,
  inputText: '',
};

export const useAppStore = create<AppState>((set) => ({
  ...initialState,

  setState: (state) => set({ currentState: state }),

  setScript: (script) => set({
    script,
    scriptContent: script?.content || '',
    params: (script?.params_schema || []).reduce((acc, p: { name: string; default?: string }) => {
      acc[p.name] = p.default || '';
      return acc;
    }, {} as Record<string, string>),
  }),

  setScriptContent: (content) => set({ scriptContent: content }),

  appendScriptContent: (content) => set((state) => ({
    scriptContent: state.scriptContent + content,
  })),

  setParams: (params) => set({ params }),

  setExecuting: (executing) => set({ isExecuting: executing }),

  setTaskId: (taskId) => set({ runningTaskId: taskId }),

  setStatus: (text) => set({ statusText: text }),

  setAsrActive: (active) => set({ asrActive: active }),

  openSettings: () => set({ isSettingsVisible: true }),

  closeSettings: () => set({ isSettingsVisible: false }),

  updateSettings: (settings) => set((state) => ({
    settings: { ...state.settings, ...settings },
  })),

  setUvAvailable: (available) => set({ uvAvailable: available }),

  setInputText: (text) => set({ inputText: text }),

  reset: () => set(initialState),
}));