// Database module using sql.js (SQLite in WASM)

import { FSBasedFileManager, MemvidDatabase } from "memvid";
import type {
  Script,
  ParamDeclaration,
  WidgetType,
  AppSettings,
} from "../types.ts";

export interface ToolCache {
  tool_name: string;
  version: string;
  help_text: string;
  ai_summary?: string;
  usage_guide?: string;
  embedding: number[];
  cached_at: string;
}

export interface ExecutionRecord {
  id: string;
  script_id: string;
  params_used: Record<string, unknown>;
  exit_code?: number;
  stderr_summary?: string;
  executed_at: string;
}

export class Database {
  private db: MemvidDatabase | null = null;
  private dbPath: string;

  constructor(dbPath: string) {
    this.dbPath = dbPath;
  }

  async init(): Promise<void> {
    const manager = new FSBasedFileManager();
    this.db = new MemvidDatabase(this.dbPath, manager);

    // Create schema if needed
    await this.createSchema();
  }

  private async createSchema(): Promise<void> {
    if (!this.db) throw new Error("DB not initialized");

    await this.db.execute(`
      CREATE TABLE IF NOT EXISTS scripts (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        content TEXT NOT NULL,
        runtime TEXT NOT NULL,
        shell_target TEXT,
        params_schema TEXT,
        alias TEXT,
        use_count INTEGER DEFAULT 0,
        created_at TEXT NOT NULL,
        last_used_at TEXT,
        embedding BLOB
      )
    `);

    await this.db.execute(`
      CREATE TABLE IF NOT EXISTS execution_records (
        id TEXT PRIMARY KEY,
        script_id TEXT NOT NULL,
        params_used TEXT NOT NULL,
        exit_code INTEGER,
        stderr_summary TEXT,
        executed_at TEXT NOT NULL
      )
    `);

    await this.db.execute(`
      CREATE TABLE IF NOT EXISTS tool_cache (
        tool_name TEXT PRIMARY KEY,
        version TEXT NOT NULL,
        help_text TEXT NOT NULL,
        ai_summary TEXT,
        usage_guide TEXT,
        embedding BLOB,
        cached_at TEXT NOT NULL
      )
    `);

    await this.db.execute(`
      CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
      )
    `);
  }

  // Script operations
  async insertScript(script: Script): Promise<void> {
    if (!this.db) throw new Error("DB not initialized");

    await this.db.execute(
      `INSERT OR REPLACE INTO scripts
       (id, name, description, content, runtime, shell_target, params_schema, alias, use_count, created_at, last_used_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      [
        script.id,
        script.name,
        script.description,
        JSON.stringify(script.content),
        script.runtime,
        script.shell_target ?? null,
        JSON.stringify(script.params_schema),
        script.alias ?? null,
        script.use_count,
        script.created_at,
        script.last_used_at ?? null,
      ]
    );
  }

  async listScripts(): Promise<Script[]> {
    if (!this.db) throw new Error("DB not initialized");

    const rows = await this.db.all<{
      id: string;
      name: string;
      description: string;
      content: string;
      runtime: string;
      shell_target?: string;
      params_schema: string;
      alias?: string;
      use_count: number;
      created_at: string;
      last_used_at?: string;
    }>(`SELECT * FROM scripts ORDER BY created_at DESC`);

    return rows.map((row) => ({
      id: row.id,
      name: row.name,
      description: row.description,
      content: row.content,
      runtime: row.runtime as Script["runtime"],
      shell_target: row.shell_target as Script["shell_target"],
      params_schema: JSON.parse(row.params_schema || "[]"),
      alias: row.alias ?? undefined,
      use_count: row.use_count,
      created_at: row.created_at,
      last_used_at: row.last_used_at ?? undefined,
    }));
  }

  async getScript(id: string): Promise<Script | null> {
    if (!this.db) throw new Error("DB not initialized");

    const row = await this.db.get(
      `SELECT * FROM scripts WHERE id = ?`,
      [id]
    );
    if (!row) return null;

    return {
      id: row.id,
      name: row.name,
      description: row.description,
      content: row.content,
      runtime: row.runtime as Script["runtime"],
      shell_target: row.shell_target as Script["shell_target"],
      params_schema: JSON.parse(row.params_schema || "[]"),
      alias: row.alias ?? undefined,
      use_count: row.use_count,
      created_at: row.created_at,
      last_used_at: row.last_used_at ?? undefined,
    };
  }

  async incrementUseCount(scriptId: string): Promise<void> {
    if (!this.db) throw new Error("DB not initialized");
    await this.db.execute(
      `UPDATE scripts SET use_count = use_count + 1 WHERE id = ?`,
      [scriptId]
    );
  }

  // Tool cache operations
  async getAllToolEmbeddings(): Promise<Array<{ name: string; description: string; embedding: number[] }>> {
    if (!this.db) throw new Error("DB not initialized");

    const rows = await this.db.all<{ tool_name: string; help_text: string; embedding: Blob }>(
      `SELECT tool_name, help_text, embedding FROM tool_cache`
    );

    return rows.map((row) => ({
      name: row.tool_name,
      description: row.help_text,
      embedding: this.blobToEmbedding(row.embedding),
    }));
  }

  async getToolCache(name: string): Promise<ToolCache | null> {
    if (!this.db) throw new Error("DB not initialized");

    const row = await this.db.get(`SELECT * FROM tool_cache WHERE tool_name = ?`, [name]);
    if (!row) return null;

    return {
      tool_name: row.tool_name,
      version: row.version,
      help_text: row.help_text,
      ai_summary: row.ai_summary ?? undefined,
      usage_guide: row.usage_guide ?? undefined,
      embedding: this.blobToEmbedding(row.embedding),
      cached_at: row.cached_at,
    };
  }

  async listToolCache(): Promise<ToolCache[]> {
    if (!this.db) throw new Error("DB not initialized");

    const rows = await this.db.all(`SELECT * FROM tool_cache`);
    return rows.map((row) => ({
      tool_name: row.tool_name,
      version: row.version,
      help_text: row.help_text,
      ai_summary: row.ai_summary ?? undefined,
      usage_guide: row.usage_guide ?? undefined,
      embedding: this.blobToEmbedding(row.embedding),
      cached_at: row.cached_at,
    }));
  }

  async insertToolCache(tool: ToolCache): Promise<void> {
    if (!this.db) throw new Error("DB not initialized");

    await this.db.execute(
      `INSERT OR REPLACE INTO tool_cache
       (tool_name, version, help_text, ai_summary, usage_guide, embedding, cached_at)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
      [
        tool.tool_name,
        tool.version,
        tool.help_text,
        tool.ai_summary ?? null,
        tool.usage_guide ?? null,
        this.embeddingToBlob(tool.embedding),
        tool.cached_at,
      ]
    );
  }

  // Execution records
  async insertExecution(record: ExecutionRecord): Promise<void> {
    if (!this.db) throw new Error("DB not initialized");

    await this.db.execute(
      `INSERT INTO execution_records (id, script_id, params_used, exit_code, stderr_summary, executed_at)
       VALUES (?, ?, ?, ?, ?, ?)`,
      [
        record.id,
        record.script_id,
        JSON.stringify(record.params_used),
        record.exit_code ?? null,
        record.stderr_summary ?? null,
        record.executed_at,
      ]
    );
  }

  async getLastExecution(scriptId: string): Promise<ExecutionRecord | null> {
    if (!this.db) throw new Error("DB not initialized");

    const row = await this.db.get(
      `SELECT * FROM execution_records WHERE script_id = ? ORDER BY executed_at DESC LIMIT 1`,
      [scriptId]
    );
    if (!row) return null;

    return {
      id: row.id,
      script_id: row.script_id,
      params_used: JSON.parse(row.params_used),
      exit_code: row.exit_code ?? undefined,
      stderr_summary: row.stderr_summary ?? undefined,
      executed_at: row.executed_at,
    };
  }

  // Settings
  async getSettings(): Promise<AppSettings> {
    if (!this.db) throw new Error("DB not initialized");

    const rows = await this.db.all<{ key: string; value: string }>(`SELECT * FROM settings`);
    const settings: Record<string, string> = {};
    for (const row of rows) {
      settings[row.key] = row.value;
    }

    return {
      api_key: settings["ai_api_key"] ?? "",
      api_base: settings["ai_api_base"] ?? undefined,
      model: settings["ai_model"] ?? "claude-sonnet-4-6",
      hotkey: settings["hotkey"] ?? "Ctrl+Space",
      language: settings["language"] ?? "zh",
      asr_enabled: settings["asr_enabled"] === "true",
      asr_model_path: settings["asr_model_path"] ?? undefined,
    };
  }

  async saveSettings(settings: Record<string, unknown>): Promise<void> {
    if (!this.db) throw new Error("DB not initialized");

    for (const [key, value] of Object.entries(settings)) {
      await this.db.execute(
        `INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)`,
        [key, String(value)]
      );
    }
  }

  // Helper: blob to number array
  private blobToEmbedding(blob: Blob | null): number[] {
    if (!blob) return [];
    // Simple implementation - in practice would need proper deserialization
    return [];
  }

  // Helper: number array to blob
  private embeddingToBlob(embedding: number[]): Blob {
    // Simple implementation - in practice would need proper serialization
    return new Blob([JSON.stringify(embedding)]);
  }
}