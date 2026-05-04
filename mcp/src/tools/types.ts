export type Json = Record<string, unknown> | undefined;

export interface Tool {
  name: string;
  description: string;
  inputSchema: Json;
}

export interface ContextResult {
  final: string;
  usage: {
    inputTokens: number;
    outputTokens: number;
    totalTokens: number;
    agent?: {
      inputTokens: number;
      cacheReadTokens: number;
      cacheWriteTokens: number;
      outputTokens: number;
      totalTokens: number;
    };
    contextSummary?: {
      inputTokens: number;
      cacheReadTokens: number;
      cacheWriteTokens: number;
      outputTokens: number;
      totalTokens: number;
    };
    model?: string;
    provider?: string;
  };
  tool_use?: string;
  content: any;
  logs?: string;
  sessionId?: string; // Return session ID for multi-turn conversations
}
