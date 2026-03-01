export interface Message {
  id: string;
  role: "system" | "user" | "assistant";
  content: string;
  createdAt: string;
  isStreaming?: boolean;
}

export interface Conversation {
  id: string;
  title: string;
  modelName: string;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
}

export type TokenEvent =
  | { type: "Token"; text: string; token_index: number }
  | {
      type: "Done";
      total_tokens: number;
      generation_time_ms: number;
      tokens_per_second: number;
      prompt_tokens: number;
    }
  | { type: "Error"; message: string };
