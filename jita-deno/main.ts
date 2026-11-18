// Jita Deno Backend - JSON-RPC Server
// Handles script generation, execution, memory, ASR via TypeScript

import { Database } from "./src/db/database.ts";
import { AgentModule } from "./src/agent/agent.ts";
import { TaskManager } from "./src/task_manager/task_manager.ts";
import { MemvidMemory } from "./src/memory/memory.ts";
import type {
  JsonRpcRequest,
  JsonRpcResponse,
  Script,
  GenerateResult,
  ExecuteResult,
} from "./src/types.ts";

const DB_PATH = `${Deno.env.get("HOME")}/.local/share/jita/jita.db`;
const MODELS_DIR = `${Deno.env.get("HOME")}/.local/share/jita/models`;

// Global instances
let db!: Database;
let agent!: AgentModule;
let taskManager!: TaskManager;
let memory!: MemvidMemory;

// Initialize all modules
async function init(): Promise<void> {
  console.error("[Jita Deno] Initializing...");

  // Ensure directories exist
  await Deno.mkdir(`${Deno.env.get("HOME")}/.local/share/jita`, { recursive: true });
  await Deno.mkdir(MODELS_DIR, { recursive: true });

  // Initialize database
  db = new Database(DB_PATH);
  await db.init();

  // Initialize memory (memvid)
  memory = new MemvidMemory(db);

  // Initialize agent (LLM client)
  const settings = await db.getSettings();
  agent = new AgentModule(settings, memory);

  // Initialize task manager
  taskManager = new TaskManager();

  console.error("[Jita Deno] Initialization complete");
}

// JSON-RPC request handler
async function handleRequest(req: JsonRpcRequest): Promise<JsonRpcResponse> {
  const { method, params, id } = req;

  try {
    let result: unknown;

    switch (method) {
      case "generate_script": {
        const { text } = params as { text: string };
        const script = await agent.generateScript(text);
        result = { success: true, script } as GenerateResult;
        break;
      }

      case "execute_script": {
        const { script, params: scriptParams } = params as {
          script: Script;
          params: Record<string, string>;
        };
        const handle = await taskManager.spawnTask(script, scriptParams, db);
        result = { success: true, task_id: handle.taskId } as ExecuteResult;

        // Set up output streaming
        taskManager.streamOutput(handle.taskId, (line: { type: string; content: string }) => {
          emit("script_output", { line_type: line.type, content: line.content });
        });

        taskManager.onComplete(handle.taskId, (exitCode: number) => {
          emit("task_complete", {
            task_id: handle.taskId,
            exit_code: exitCode,
            error: null,
          });
        });
        break;
      }

      case "stop_script": {
        const { task_id } = params as { task_id: string };
        await taskManager.stopTask(task_id);
        result = null;
        break;
      }

      case "get_settings": {
        result = await db.getSettings();
        break;
      }

      case "save_settings": {
        const { settings } = params as { settings: unknown };
        await db.saveSettings(settings as Record<string, unknown>);
        result = null;
        break;
      }

      case "list_scripts": {
        result = await db.listScripts();
        break;
      }

      case "check_uv": {
        result = await checkUv();
        break;
      }

      case "toggle_asr": {
        // ASR integration placeholder
        result = false;
        break;
      }

      case "preload_asr": {
        result = null;
        break;
      }

      default:
        throw new Error(`Unknown method: ${method}`);
    }

    return {
      jsonrpc: "2.0",
      id: id as number | string,
      result,
    };
  } catch (error) {
    return {
      jsonrpc: "2.0",
      id: id as number | string,
      error: {
        code: -32603,
        message: error instanceof Error ? error.message : String(error),
      },
    };
  }
}

// Emit event to frontend via Tauri
function emit(event: string, payload: unknown): void {
  console.log(
    JSON.stringify({
      method: "emit",
      params: { event, payload },
    })
  );
}

// Check if uv is available
async function checkUv(): Promise<boolean> {
  try {
    const proc = Deno.run({
      cmd: ["uv", "--version"],
      stdout: "null",
      stderr: "null",
    });
    await proc.status();
    return true;
  } catch {
    return false;
  }
}

// Main server loop - read JSON-RPC requests from stdin
async function main(): Promise<void> {
  await init();

  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  const buffer = new Uint8Array(1024);

  while (true) {
    const n = await Deno.stdin.read(buffer);
    if (n === null) break;

    const line = decoder.decode(buffer.slice(0, n)).trim();
    if (!line) continue;

    try {
      const req = JSON.parse(line) as JsonRpcRequest;
      const resp = await handleRequest(req);
      if (resp.id !== undefined) {
        await Deno.stdout.write(encoder.encode(JSON.stringify(resp) + "\n"));
      }
    } catch (err) {
      console.error("[Jita Deno] Error parsing request:", err);
    }
  }
}

main();