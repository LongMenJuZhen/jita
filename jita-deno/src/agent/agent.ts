// Agent module - LLM client using @anthropic-ai/sdk

import Anthropic from "@anthropic-ai/sdk";
import type { Script, GenerateResult, AppSettings } from "../types.ts";
import { MemvidMemory } from "../memory/memory.ts";

interface CliTool {
  name: string;
  description: string;
  version?: string;
  help_text?: string;
  usage_guide?: string;
  embedding?: number[];
}

export class AgentModule {
  private client: Anthropic | null = null;
  private settings: AppSettings;
  private memory: MemvidMemory;

  constructor(settings: AppSettings, memory: MemvidMemory) {
    this.settings = settings;
    this.memory = memory;

    if (settings.api_key) {
      this.client = new Anthropic({
        apiKey: settings.api_key,
        baseURL: settings.api_base,
      });
    }
  }

  async generateScript(userInput: string): Promise<Script> {
    if (!this.client) {
      throw new Error("Agent not initialized - no API key");
    }

    // RAG: search related tools
    const relatedTools = await this.memory.searchRelatedTools(userInput, 5);
    const toolNames = relatedTools.map((t) => t.name);
    const toolGuide = await this.memory.getToolGuides(toolNames);

    // Build tools summary for prompt
    const allTools = await this.memory.listAllTools();
    const toolsSummary = allTools
      .map((t) => `- ${t.name}: ${t.ai_summary ?? t.description}`)
      .join("\n") || "无可用工具";

    // Context
    const contextSummary = `工作目录: ${Deno.env.get("PWD") ?? Deno.env.get("HOME")}`;

    // Build prompt
    const prompt = this.buildPrompt(userInput, toolsSummary, contextSummary, toolGuide);

    // Call LLM
    const message = await this.client.messages.create({
      model: this.settings.model || "claude-sonnet-4-6",
      max_tokens: 4096,
      messages: [{ role: "user", content: prompt }],
    });

    const response = message.content[0];
    if (response.type !== "text") {
      throw new Error("Unexpected response type from LLM");
    }

    // Parse generated script
    const script = this.parseGeneratedScript(response.text);
    return script;
  }

  private buildPrompt(
    userInput: string,
    toolsSummary: string,
    contextSummary: string,
    toolGuide: string
  ): string {
    return `你是 Jita，一个智能脚本生成助手。

用户需要执行的任务：
${userInput}

可用工具：
${toolsSummary}

执行上下文：
${contextSummary}

${toolGuide ? `工具使用指南：\n${toolGuide}\n` : ""}

请生成一个脚本来完成用户的需求。脚本应该是：
- 简洁高效
- 使用 Python PEP 723 格式（包含依赖）或 Shell 脚本
- 参数使用环境变量声明
- 有适当的错误处理

以 JSON 格式返回脚本：
{
  "name": "脚本名称",
  "description": "脚本描述",
  "content": "脚本内容",
  "runtime": "python_pep723 或 shell",
  "shell_target": "bash/pwsh/sh（仅 shell 脚本）",
  "params_schema": [
    {
      "name": "参数名（大写下划线）",
      "label": "显示标签",
      "widget": { "type": "text/secret/file/directory/select/number/toggle/textarea" },
      "required": true/false,
      "description": "参数说明",
      "default": "默认值"
    }
  ]
}

只返回 JSON，不要有其他内容。`;
  }

  private parseGeneratedScript(text: string): Script {
    // Try to extract JSON from response
    let jsonStr = text.trim();

    // Handle markdown code blocks
    const codeBlockMatch = jsonStr.match(/```(?:json)?\s*([\s\S]*?)```/);
    if (codeBlockMatch) {
      jsonStr = codeBlockMatch[1];
    }

    // Try to find JSON object
    const jsonMatch = jsonStr.match(/\{[\s\S]*\}/);
    if (jsonMatch) {
      jsonStr = jsonMatch[0];
    }

    try {
      const parsed = JSON.parse(jsonStr);
      return {
        id: crypto.randomUUID(),
        name: parsed.name || "未命名脚本",
        description: parsed.description || "",
        content: parsed.content || "",
        runtime: parsed.runtime === "python_pep723" ? "python_pep723" : "shell",
        shell_target: parsed.shell_target || undefined,
        params_schema: parsed.params_schema || [],
        alias: parsed.alias || undefined,
        use_count: 0,
        created_at: new Date().toISOString(),
        last_used_at: undefined,
      };
    } catch {
      throw new Error(`Failed to parse generated script: ${text.substring(0, 200)}`);
    }
  }

  async repairScript(
    originalScript: Script,
    stderr: string,
    exitCode: number
  ): Promise<Script> {
    if (!this.client) {
      throw new Error("Agent not initialized");
    }

    const prompt = `修复以下脚本的错误。

原始脚本：
名称：${originalScript.name}
内容：
${originalScript.content}

错误输出：
${stderr}

退出码：${exitCode}

请分析错误并生成修复后的脚本。只返回 JSON：
{
  "name": "脚本名称",
  "description": "脚本描述",
  "content": "修复后的脚本内容",
  "runtime": "python_pep723 或 shell",
  "params_schema": []
}`;

    const message = await this.client.messages.create({
      model: this.settings.model || "claude-sonnet-4-6",
      max_tokens: 4096,
      messages: [{ role: "user", content: prompt }],
    });

    const response = message.content[0];
    if (response.type !== "text") {
      throw new Error("Unexpected response type");
    }

    return this.parseGeneratedScript(response.text);
  }

  async summarizeTool(toolName: string, helpText: string): Promise<string> {
    if (!this.client) {
      return helpText.split("\n")[0];
    }

    const prompt = `用一句话描述以下工具的功能（用中文，20字以内）：
工具名：${toolName}
帮助信息：
${helpText}`;

    const message = await this.client.messages.create({
      model: this.settings.model || "claude-sonnet-4-6",
      max_tokens: 100,
      messages: [{ role: "user", content: prompt }],
    });

    const response = message.content[0];
    if (response.type !== "text") {
      return helpText.split("\n")[0];
    }

    return response.text.trim().substring(0, 50);
  }

  async generateUsageGuide(toolName: string, helpText: string): Promise<string> {
    if (!this.client) {
      return helpText;
    }

    const prompt = `为以下工具生成详细的使用指南，包含参数说明和示例：
工具名：${toolName}
帮助信息：
${helpText}

请用 Markdown 格式返回使用指南。`;

    const message = await this.client.messages.create({
      model: this.settings.model || "claude-sonnet-4-6",
      max_tokens: 1000,
      messages: [{ role: "user", content: prompt }],
    });

    const response = message.content[0];
    if (response.type !== "text") {
      return helpText;
    }

    return response.text;
  }
}