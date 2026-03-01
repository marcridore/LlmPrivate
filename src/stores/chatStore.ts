import { create } from "zustand";
import { invoke, Channel } from "@tauri-apps/api/core";
import type { Message, Conversation, TokenEvent, ImageAttachment } from "../types/chat";

const PAGE_SIZE = 30;

interface ChatState {
  conversations: Conversation[];
  activeConversationId: string | null;
  messages: Message[];
  isGenerating: boolean;
  tokensPerSecond: number;
  loadedModelHandle: number | null;
  pendingImages: ImageAttachment[];
  _cleanupDone: boolean;
  hasMoreConversations: boolean;

  initConversations: () => Promise<void>;
  loadConversations: () => Promise<void>;
  loadMoreConversations: () => Promise<void>;
  selectConversation: (id: string) => Promise<void>;
  createConversation: (title?: string) => Promise<string>;
  deleteConversation: (id: string) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  stopGeneration: () => Promise<void>;
  setLoadedModelHandle: (handle: number | null) => void;
  addPendingImage: (image: ImageAttachment) => void;
  removePendingImage: (id: string) => void;
  clearPendingImages: () => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  conversations: [],
  activeConversationId: null,
  messages: [],
  isGenerating: false,
  tokensPerSecond: 0,
  loadedModelHandle: null,
  pendingImages: [],
  _cleanupDone: false,
  hasMoreConversations: false,

  initConversations: async () => {
    if (get()._cleanupDone) {
      await get().loadConversations();
      return;
    }
    // Clean up empty conversations + rename untitled ones on first load
    try {
      await invoke("cleanup_empty_conversations");
    } catch {
      // Command may not exist yet, skip
    }
    set({ _cleanupDone: true });
    await get().loadConversations();
  },

  loadConversations: async () => {
    try {
      const conversations = await invoke<Conversation[]>("get_conversations", {
        limit: PAGE_SIZE,
        offset: 0,
      });
      set({
        conversations,
        hasMoreConversations: conversations.length >= PAGE_SIZE,
      });
    } catch (e) {
      console.error("Failed to load conversations:", e);
    }
  },

  loadMoreConversations: async () => {
    if (!get().hasMoreConversations) return;
    try {
      const offset = get().conversations.length;
      const more = await invoke<Conversation[]>("get_conversations", {
        limit: PAGE_SIZE,
        offset,
      });
      set((s) => ({
        conversations: [...s.conversations, ...more],
        hasMoreConversations: more.length >= PAGE_SIZE,
      }));
    } catch (e) {
      console.error("Failed to load more conversations:", e);
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

  createConversation: async (title?: string) => {
    try {
      const id = await invoke<string>("create_conversation", { title: title ?? null });
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
    const { activeConversationId, messages, loadedModelHandle, pendingImages } = get();

    if (!loadedModelHandle) {
      console.error("No model loaded");
      return;
    }

    let conversationId = activeConversationId;
    if (!conversationId) {
      // Use first ~50 chars of message as conversation title
      const title = content.trim().slice(0, 50) || "New Chat";
      conversationId = await get().createConversation(title);
      if (!conversationId) return;
    }

    // Save user message to DB
    try {
      await invoke("save_user_message", {
        conversationId,
        content,
      });
    } catch (e) {
      console.error("Failed to save user message:", e);
    }

    const attachedImages = pendingImages.length > 0 ? [...pendingImages] : undefined;
    set({ pendingImages: [] });

    const userMessage: Message = {
      id: crypto.randomUUID(),
      role: "user",
      content,
      createdAt: new Date().toISOString(),
      images: attachedImages,
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
      images: (m.images ?? []).map((img) => ({
        id: img.id,
        file_path: img.filePath,
        alt_text: img.altText ?? null,
      })),
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

  addPendingImage: (image) =>
    set({ pendingImages: [...get().pendingImages, image] }),

  removePendingImage: (id) =>
    set({ pendingImages: get().pendingImages.filter((img) => img.id !== id) }),

  clearPendingImages: () => set({ pendingImages: [] }),
}));
