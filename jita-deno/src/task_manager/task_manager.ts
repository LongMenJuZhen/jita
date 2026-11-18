// Task Manager - Script execution

import type { Script } from "../types.ts";

interface TaskHandle {
  taskId: string;
  process: Deno.ChildProcess;
}

interface OutputLine {
  type: "stdout" | "stderr";
  content: string;
}

type OutputCallback = (line: OutputLine) => void;
type CompleteCallback = (exitCode: number) => void;

export class TaskManager {
  private tasks: Map<string, TaskHandle> = new Map();
  private outputCallbacks: Map<string, OutputCallback[]> = new Map();
  private completeCallbacks: Map<string, CompleteCallback[]> = new Map();
  private taskCounter = 0;

  async spawnTask(
    script: Script,
    params: Record<string, string>,
    _db: { incrementUseCount: (id: string) => Promise<void> }
  ): Promise<TaskHandle> {
    const taskId = `task_${++this.taskCounter}`;

    // Write script to temp file
    const scriptDir = `${Deno.env.get("HOME")}/.local/share/jita/scripts`;
    await Deno.mkdir(scriptDir, { recursive: true });

    const ext = script.runtime === "python_pep723" ? "py" : "sh";
    const scriptPath = `${scriptDir}/${script.id}.${ext}`;
    await Deno.writeTextFile(scriptPath, script.content);

    // Build command based on runtime
    let cmd: string[];
    let args: string[];

    if (script.runtime === "python_pep723") {
      cmd = ["uv", "run", scriptPath];
    } else {
      const shell = script.shell_target ?? "bash";
      switch (shell) {
        case "bash":
          cmd = ["bash", scriptPath];
          break;
        case "pwsh":
          cmd = ["pwsh", "-File", scriptPath];
          break;
        default:
          cmd = ["sh", scriptPath];
      }
    }

    // Spawn process
    const proc = Deno.spawnChild(cmd[0], {
      args: cmd.slice(1),
      env: params,
      stdout: "piped",
      stderr: "piped",
    });

    const handle: TaskHandle = { taskId, process: proc as unknown as Deno.ChildProcess };
    this.tasks.set(taskId, handle);

    // Set up output streaming
    this.streamOutputFromProcess(taskId, proc);

    // Wait for completion
    this.waitForCompletion(taskId, proc);

    return handle;
  }

  private async streamOutputFromProcess(taskId: string, proc: Deno.ChildProcess): Promise<void> {
    const cbs = this.outputCallbacks.get(taskId) ?? [];

    // Read stdout
    (async () => {
      try {
        for await (const chunk of (proc as any).stdout.readableStreams ?? []) {
          const text = new TextDecoder().decode(chunk);
          for (const line of text.split("\n")) {
            if (line) {
              for (const cb of cbs) {
                cb({ type: "stdout", content: line });
              }
            }
          }
        }
      } catch {
        // Process ended
      }
    })();

    // Read stderr
    (async () => {
      try {
        for await (const chunk of (proc as any).stderr.readableStreams ?? []) {
          const text = new TextDecoder().decode(chunk);
          for (const line of text.split("\n")) {
            if (line) {
              for (const cb of cbs) {
                cb({ type: "stderr", content: line });
              }
            }
          }
        }
      } catch {
        // Process ended
      }
    })();
  }

  private async waitForCompletion(taskId: string, proc: Deno.ChildProcess): Promise<void> {
    try {
      const status = await proc.status;
      const exitCode = status.code;

      const cbs = this.completeCallbacks.get(taskId) ?? [];
      for (const cb of cbs) {
        cb(exitCode);
      }
    } catch {
      const cbs = this.completeCallbacks.get(taskId) ?? [];
      for (const cb of cbs) {
        cb(-1);
      }
    } finally {
      this.tasks.delete(taskId);
      this.outputCallbacks.delete(taskId);
      this.completeCallbacks.delete(taskId);
    }
  }

  streamOutput(taskId: string, callback: OutputCallback): void {
    const cbs = this.outputCallbacks.get(taskId) ?? [];
    cbs.push(callback);
    this.outputCallbacks.set(taskId, cbs);
  }

  onComplete(taskId: string, callback: CompleteCallback): void {
    const cbs = this.completeCallbacks.get(taskId) ?? [];
    cbs.push(callback);
    this.completeCallbacks.set(taskId, cbs);
  }

  async stopTask(taskId: string): Promise<void> {
    const handle = this.tasks.get(taskId);
    if (handle) {
      handle.process.kill();
      this.tasks.delete(taskId);
    }
  }

  async hasRunningTasks(): Promise<boolean> {
    return this.tasks.size > 0;
  }
}