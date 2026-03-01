import { create } from "zustand";
import { invoke, Channel } from "@tauri-apps/api/core";
import type { Message, Conversation, TokenEvent } from "../types/chat";

interface ChatState {
  conversations: Conversation[];
  activeConversationId: string | null;
  messages: Message[];
  isGenerating: boolean;
  tokensPerSecond: number;
  loadedModelHandle: number | null;

  loadConversations: () => Promise<void>;
  selectConversation: (id: string) => Promise<void>;
  createConversation: () => Promise<string>;
  deleteConversation: (id: string) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  stopGeneration: () => Promise<void>;
  setLoadedModelHandle: (handle: number | null) => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  conversations: [],
  activeConversationId: null,
  messages: [],
  isGenerating: false,
  tokensPerSecond: 0,
  loadedModelHandle: null,

  loadConversations: async () => {
    try {
      const conversations = await invoke<Conversation[]>("get_conversations");
      set({ conversations });
    } catch (e) {
      console.error("Failed to load conversations:", e);
    }
  },

  selectConversation: async (id: string) => {
    try {
      const rawMessages = await invoke<{ role: string; content: string }[]>(
        "get_messages",
        { conversationId: id }
      );
      const messages: Message[] = rawMessages.map((m) => ({
        id: crypto.randomUUID(),
        role: m.role as Message["role"],
        content: m.content,
        createdAt: new Date().toISOString(),
      }));
      set({ activeConversationId: id, messages });
    } catch (e) {
      console.error("Failed to load messages:", e);
    }
  },

  createConversation: async () => {
    try {
      const id = await invoke<string>("create_conversation", { title: null });
      await get().loadConversations();
      set({ activeConversationId: id, messages: [] });
      return id;
    } catch (e) {
      console.error("Failed to create conversation:", e);
      return "";
    }
  },

  deleteConversation: async (id: string) => {
    try {
      await invoke("delete_conversation", { conversationId: id });
      await get().loadConversations();
      if (get().activeConversationId === id) {
        set({ activeConversationId: null, messages: [] });
      }
    } catch (e) {
      console.error("Failed to delete conversation:", e);
    }
  },

  sendMessage: async (content: string) => {
    const { activeConversationId, messages, loadedModelHandle } = get();

    if (!loadedModelHandle) {
      console.error("No model loaded");
      return;
    }

    let conversationId = activeConversationId;
    if (!conversationId) {
      conversationId = await get().createConversation();
      if (!conversationId) return;
    }

    // Save user message to DB
    try {
      await invoke("send_user_message_to_db", {
        conversationId,
        content,
      });
    } catch {
      // Command may not exist yet, that's ok
    }

    const userMessage: Message = {
      id: crypto.randomUUID(),
      role: "user",
      content,
      createdAt: new Date().toISOString(),
    };

    const assistantMessage: Message = {
      id: crypto.randomUUID(),
      role: "assistant",
      content: "",
      createdAt: new Date().toISOString(),
      isStreaming: true,
    };

    set({
      messages: [...messages, userMessage, assistantMessage],
      isGenerating: true,
    });

    const onToken = new Channel<TokenEvent>();
    onToken.onmessage = (event: TokenEvent) => {
      const currentMessages = get().messages;
      const lastMsg = currentMessages[currentMessages.length - 1];

      if (event.type === "Token") {
        set({
          messages: [
            ...currentMessages.slice(0, -1),
            { ...lastMsg, content: lastMsg.content + event.text },
          ],
        });
      } else if (event.type === "Done") {
        set({
          messages: [
            ...currentMessages.slice(0, -1),
            { ...lastMsg, isStreaming: false },
          ],
          isGenerating: false,
          tokensPerSecond: event.tokens_per_second,
        });
      } else if (event.type === "Error") {
        set({
          messages: [
            ...currentMessages.slice(0, -1),
            {
              ...lastMsg,
              content: `Error: ${event.message}`,
              isStreaming: false,
            },
          ],
          isGenerating: false,
        });
      }
    };

    const chatMessages = [...messages, userMessage].map((m) => ({
      role: m.role,
      content: m.content,
    }));

    try {
      await invoke("send_message", {
        conversationId,
        messages: chatMessages,
        modelHandle: loadedModelHandle,
        params: {
          messages: chatMessages,
          max_tokens: 2048,
          temperature: 0.7,
          top_p: 0.9,
          top_k: 40,
          repeat_penalty: 1.1,
          stop_sequences: [],
        },
        onToken,
      });
    } catch (e) {
      console.error("Generation error:", e);
      set({ isGenerating: false });
    }
  },

  stopGeneration: async () => {
    const { loadedModelHandle } = get();
    if (loadedModelHandle) {
      try {
        await invoke("stop_generation", { modelHandle: loadedModelHandle });
      } catch (e) {
        console.error("Failed to stop generation:", e);
      }
    }
    set({ isGenerating: false });
  },

  setLoadedModelHandle: (handle) => set({ loadedModelHandle: handle }),
}));
