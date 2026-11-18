// Memory module using memvid-js for embedding and similarity search

import { Memvid } from "memvid";
import type { Database, ToolCache } from "../db/database.ts";

export interface CliTool {
  name: string;
  description: string;
  version?: string;
  help_text?: string;
  ai_summary?: string;
  usage_guide?: string;
  embedding?: number[];
}

export class MemvidMemory {
  private memvid: Memvid;
  private db: Database;

  constructor(db: Database) {
    this.memvid = new Memvid();
    this.db = db;
  }

  // Search related tools by similarity
  async searchRelatedTools(userInput: string, limit: number = 5): Promise<CliTool[]> {
    // Generate embedding for user input
    const queryEmbedding = await this.embedText(userInput);

    // Get all tool embeddings from DB
    const tools = await this.db.listToolCache();

    if (tools.length === 0) {
      return [];
    }

    // Calculate similarities
    const scored = tools.map((tool) => {
      const sim = this.cosineSimilarity(queryEmbedding, tool.embedding ?? []);
      return { tool, score: sim };
    });

    // Sort by similarity and return top results
    scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, limit).map((s) => ({
      name: s.tool.tool_name,
      description: s.tool.help_text,
      version: s.tool.version,
      help_text: s.tool.help_text,
      ai_summary: s.tool.ai_summary,
      usage_guide: s.tool.usage_guide,
      embedding: s.tool.embedding,
    }));
  }

  // Get tool usage guides
  async getToolGuides(toolNames: string[]): Promise<string> {
    const guides: string[] = [];

    for (const name of toolNames) {
      const tool = await this.db.getToolCache(name);
      if (tool?.usage_guide) {
        guides.push(`## ${name}\n${tool.usage_guide}`);
      }
    }

    return guides.join("\n\n");
  }

  // List all tools
  async listAllTools(): Promise<CliTool[]> {
    const tools = await this.db.listToolCache();
    return tools.map((t) => ({
      name: t.tool_name,
      description: t.help_text,
      version: t.version,
      help_text: t.help_text,
      ai_summary: t.ai_summary,
      usage_guide: t.usage_guide,
      embedding: t.embedding,
    }));
  }

  // Add a tool to memory
  async addTool(tool: ToolCache): Promise<void> {
    // Generate embedding using memvid
    const textToEmbed = `${tool.tool_name} ${tool.help_text} ${tool.ai_summary ?? ""}`;
    const embedding = await this.embedText(textToEmbed);
    tool.embedding = embedding;

    // Store in memvid for similarity search
    this.memvid.add(tool.tool_name, embedding);

    // Store in database
    await this.db.insertToolCache(tool);
  }

  // Simple text embedding using memvid
  private async embedText(text: string): Promise<number[]> {
    // Memvid uses its own embedding internally
    // For now, return a simple hash-based placeholder
    // In production, could use a dedicated embedding API
    const embedding = this.memvid.embed(text);
    return embedding;
  }

  // Cosine similarity calculation
  private cosineSimilarity(a: number[], b: number[]): number {
    if (a.length !== b.length || a.length === 0) {
      return 0;
    }

    let dotProduct = 0;
    let normA = 0;
    let normB = 0;

    for (let i = 0; i < a.length; i++) {
      dotProduct += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }

    const normSum = Math.sqrt(normA) * Math.sqrt(normB);
    return normSum === 0 ? 0 : dotProduct / normSum;
  }
}