export interface ParamDeclaration {
  name: string;
  label: string;
  widget: WidgetType;
  required: boolean;
  description?: string;
  default?: string;
}

export type WidgetType =
  | { type: 'text'; placeholder?: string }
  | { type: 'secret'; global_key?: string }
  | { type: 'file'; filter: string[]; multiple: boolean }
  | { type: 'directory' }
  | { type: 'select'; options: string[] }
  | { type: 'number'; min?: number; max?: number }
  | { type: 'toggle' }
  | { type: 'textarea' };

export interface Script {
  id: string;
  name: string;
  description: string;
  content: string;
  runtime: 'python_pep723' | 'shell';
  shell_target?: 'bash' | 'pwsh' | 'sh';
  params_schema: ParamDeclaration[];
  alias?: string;
  use_count: number;
  created_at: string;
  last_used_at?: string;
}

export type WindowState = 'input' | 'generating' | 'reviewing' | 'param_input';

export interface AppSettings {
  api_key: string;
  api_base?: string;
  model: string;
  hotkey: string;
  asr_enabled: boolean;
  asr_model_path?: string;
}

export interface GenerateResult {
  success: boolean;
  script?: Script;
  error?: string;
}

export interface ExecuteResult {
  success: boolean;
  task_id?: string;
  error?: string;
}

export interface OutputLine {
  line_type: 'stdout' | 'stderr';
  content: string;
}

export interface TaskComplete {
  task_id: string;
  exit_code: number | null;
  error: string | null;
}