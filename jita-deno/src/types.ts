// Shared types between Jita frontend and Deno backend

export type WidgetType =
  | { type: "text"; placeholder?: string }
  | { type: "secret"; global_key?: string }
  | { type: "file"; filter: string[]; multiple: boolean }
  | { type: "directory" }
  | { type: "select"; options: string[] }
  | { type: "number"; min?: number; max?: number }
  | { type: "toggle" }
  | { type: "textarea" };

export interface ParamDeclaration {
  name: string;
  label: string;
  widget: WidgetType;
  required: boolean;
  description?: string;
  default?: string;
}

export type ScriptRuntime = "python_pep723" | "shell";
export type ShellTarget = "bash" | "pwsh" | "sh";

export interface Script {
  id: string;
  name: string;
  description: string;
  content: string;
  runtime: ScriptRuntime;
  shell_target?: ShellTarget;
  params_schema: ParamDeclaration[];
  alias?: string;
  use_count: number;
  created_at: string;
  last_used_at?: string;
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
  line_type: "stdout" | "stderr";
  content: string;
}

export interface TaskComplete {
  task_id: string;
  exit_code?: number;
  error?: string;
}

export interface AppSettings {
  api_key: string;
  api_base?: string;
  model: string;
  hotkey: string;
  language: string;
  asr_enabled: boolean;
  asr_model_path?: string;
}

// JSON-RPC types
export interface JsonRpcRequest {
  jsonrpc: "2.0";
  id: number | string;
  method: string;
  params?: unknown;
}

export interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: number | string;
  result?: unknown;
  error?: {
    code: number;
    message: string;
    data?: unknown;
  };
}

export interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}