import { create } from "zustand";
import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  DocFolder,
  DocumentSummary,
  DocumentChatMode,
  AddDocumentProgressEvent,
  DocChatSession,
} from "../types/document";
import type { Message, TokenEvent } from "../types/chat";

interface DocumentState {
  // Folder tree
  folders: DocFolder[];
  selectedFolderId: string | null;

  // Documents in selected folder
  documents: DocumentSummary[];
  selectedDocumentIds: string[];

  // Document chat
  docChatMessages: Message[];
  docChatMode: DocumentChatMode;
  isDocChatActive: boolean;
  isDocGenerating: boolean;
  docTokensPerSecond: number;
  docConversationId: string | null;

  // Loading states
  addDocumentProgress: string | null; // null = not adding, string = current step
  addDocumentError: string | null;

  // Recent doc chats (history)
  recentDocChats: DocChatSession[];

  // Actions
  loadFolderTree: () => Promise<void>;
  createFolder: (name: string, parentId?: string | null) => Promise<string>;
  renameFolder: (folderId: string, newName: string) => Promise<void>;
  deleteFolder: (folderId: string) => Promise<void>;
  selectFolder: (folderId: string) => Promise<void>;

  addDocument: (folderId: string, filePath: string) => Promise<void>;
  deleteDocument: (documentId: string) => Promise<void>;

  toggleDocumentSelection: (documentId: string) => void;
  selectAllDocuments: () => void;
  clearDocumentSelection: () => void;

  startDocChat: (documentIds: string[], mode: DocumentChatMode) => Promise<void>;
  sendDocMessage: (content: string, modelHandle: number) => Promise<void>;
  stopDocGeneration: (modelHandle: number) => Promise<void>;
  closeDocChat: () => void;

  // Chat history
  loadRecentDocChats: () => Promise<void>;
  resumeDocChat: (session: DocChatSession) => Promise<void>;
  toggleDocChatPin: (conversationId: string) => Promise<void>;
}

export const useDocumentStore = create<DocumentState>((set, get) => ({
  folders: [],
  selectedFolderId: null,
  documents: [],
  selectedDocumentIds: [],
  docChatMessages: [],
  docChatMode: "chat",
  isDocChatActive: false,
  isDocGenerating: false,
  docTokensPerSecond: 0,
  docConversationId: null,
  addDocumentProgress: null,
  addDocumentError: null,
  recentDocChats: [],

  loadFolderTree: async () => {
    try {
      const folders = await invoke<DocFolder[]>("get_doc_folder_tree");
      set({ folders });
    } catch (e) {
      console.error("Failed to load folder tree:", e);
    }
  },

  createFolder: async (name, parentId) => {
    try {
      const id = await invoke<string>("create_doc_folder", {
        name,
        parentId: parentId ?? null,
      });
      await get().loadFolderTree();
      return id;
    } catch (e) {
      console.error("Failed to create folder:", e);
      return "";
    }
  },

  renameFolder: async (folderId, newName) => {
    try {
      await invoke("rename_doc_folder", { folderId, newName });
      await get().loadFolderTree();
    } catch (e) {
      console.error("Failed to rename folder:", e);
    }
  },

  deleteFolder: async (folderId) => {
    try {
      await invoke("delete_doc_folder", { folderId });
      const { selectedFolderId } = get();
      if (selectedFolderId === folderId) {
        set({ selectedFolderId: null, documents: [], selectedDocumentIds: [] });
      }
      await get().loadFolderTree();
    } catch (e) {
      console.error("Failed to delete folder:", e);
    }
  },

  selectFolder: async (folderId) => {
    try {
      const documents = await invoke<DocumentSummary[]>("get_documents_in_folder", {
        folderId,
      });
      set({
        selectedFolderId: folderId,
        documents,
        selectedDocumentIds: [],
        isDocChatActive: false,
      });
    } catch (e) {
      console.error("Failed to load documents:", e);
    }
  },

  addDocument: async (folderId, filePath) => {
    set({ addDocumentProgress: "Copying file...", addDocumentError: null });

    const onProgress = new Channel<AddDocumentProgressEvent>();
    onProgress.onmessage = (event: AddDocumentProgressEvent) => {
      switch (event.type) {
        case "CopyingFile":
          set({ addDocumentProgress: "Copying file..." });
          break;
        case "ExtractingText":
          set({ addDocumentProgress: "Extracting text..." });
          break;
        case "TextExtracted":
          set({ addDocumentProgress: `Extracted ${event.char_count.toLocaleString()} chars` });
          break;
        case "CreatingChunks":
          set({ addDocumentProgress: `Created ${event.chunk_count} chunks` });
          break;
        case "Indexing":
          set({ addDocumentProgress: "Indexing..." });
          break;
        case "Done":
          set({ addDocumentProgress: "Done!" });
          break;
        case "Error":
          set({ addDocumentProgress: null, addDocumentError: event.message });
          break;
      }
    };

    try {
      await invoke<DocumentSummary>("add_document", { folderId, filePath, onProgress });
      // Reload documents and folder tree (to update counts)
      const documents = await invoke<DocumentSummary[]>("get_documents_in_folder", {
        folderId,
      });
      set({ documents });
      // Keep "Done!" visible briefly before clearing
      setTimeout(() => set({ addDocumentProgress: null }), 800);
      await get().loadFolderTree();
    } catch (e) {
      console.error("Failed to add document:", e);
      set({
        addDocumentProgress: null,
        addDocumentError: e instanceof Error ? e.message : typeof e === "object" ? JSON.stringify(e) : String(e),
      });
    }
  },

  deleteDocument: async (documentId) => {
    try {
      await invoke("delete_document", { documentId });
      const { selectedFolderId } = get();
      if (selectedFolderId) {
        const documents = await invoke<DocumentSummary[]>("get_documents_in_folder", {
          folderId: selectedFolderId,
        });
        set({
          documents,
          selectedDocumentIds: get().selectedDocumentIds.filter((id) => id !== documentId),
        });
      }
      await get().loadFolderTree();
    } catch (e) {
      console.error("Failed to delete document:", e);
    }
  },

  toggleDocumentSelection: (documentId) => {
    const { selectedDocumentIds } = get();
    if (selectedDocumentIds.includes(documentId)) {
      set({
        selectedDocumentIds: selectedDocumentIds.filter((id) => id !== documentId),
      });
    } else {
      set({
        selectedDocumentIds: [...selectedDocumentIds, documentId],
      });
    }
  },

  selectAllDocuments: () => {
    set({
      selectedDocumentIds: get().documents.map((d) => d.id),
    });
  },

  clearDocumentSelection: () => {
    set({ selectedDocumentIds: [] });
  },

  startDocChat: async (documentIds, mode) => {
    try {
      // Create a conversation for this document chat session
      const docNames = get()
        .documents.filter((d) => documentIds.includes(d.id))
        .map((d) => d.filename)
        .join(", ");

      const modeLabel = mode === "chat" ? "Chat" : mode === "summarize" ? "Summary" : "Quiz";
      const title = `${modeLabel}: ${docNames}`.slice(0, 100);

      const conversationId = await invoke<string>("create_conversation", {
        title,
      });

      // Create session linkage for chat history
      await invoke("create_doc_chat_session", {
        conversationId,
        documentIds,
        mode,
      });

      set({
        selectedDocumentIds: documentIds,
        docChatMode: mode,
        isDocChatActive: true,
        docChatMessages: [],
        docConversationId: conversationId,
        docTokensPerSecond: 0,
      });
    } catch (e) {
      console.error("Failed to start doc chat:", e);
    }
  },

  sendDocMessage: async (content, modelHandle) => {
    const { selectedDocumentIds, docChatMessages, docChatMode, docConversationId } = get();

    if (!modelHandle || selectedDocumentIds.length === 0 || !docConversationId) return;

    // Save user message to DB
    try {
      await invoke("save_user_message", {
        conversationId: docConversationId,
        content,
      });
    } catch (e) {
      console.error("Failed to save user message:", e);
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
      docChatMessages: [...docChatMessages, userMessage, assistantMessage],
      isDocGenerating: true,
    });

    const onToken = new Channel<TokenEvent>();
    onToken.onmessage = (event: TokenEvent) => {
      const currentMessages = get().docChatMessages;
      const lastMsg = currentMessages[currentMessages.length - 1];

      if (event.type === "Token") {
        set({
          docChatMessages: [
            ...currentMessages.slice(0, -1),
            { ...lastMsg, content: lastMsg.content + event.text },
          ],
        });
      } else if (event.type === "Replace") {
        set({
          docChatMessages: [
            ...currentMessages.slice(0, -1),
            { ...lastMsg, content: event.full_text },
          ],
        });
      } else if (event.type === "Done") {
        set({
          docChatMessages: [
            ...currentMessages.slice(0, -1),
            { ...lastMsg, isStreaming: false },
          ],
          isDocGenerating: false,
          docTokensPerSecond: event.tokens_per_second,
        });
      } else if (event.type === "Error") {
        set({
          docChatMessages: [
            ...currentMessages.slice(0, -1),
            {
              ...lastMsg,
              content: `Error: ${event.message}`,
              isStreaming: false,
            },
          ],
          isDocGenerating: false,
        });
      }
    };

    const chatMessages = [...docChatMessages, userMessage].map((m) => ({
      role: m.role,
      content: m.content,
      images: [],
    }));

    try {
      await invoke("chat_with_documents", {
        conversationId: docConversationId,
        documentIds: selectedDocumentIds,
        messages: chatMessages,
        modelHandle,
        params: {
          messages: chatMessages,
          max_tokens: 2048,
          temperature: 0.7,
          top_p: 0.9,
          top_k: 40,
          repeat_penalty: 1.1,
          stop_sequences: [],
        },
        mode: docChatMode,
        onToken,
      });
    } catch (e) {
      console.error("Document chat error:", e);
      const currentMessages = get().docChatMessages;
      const lastMsg = currentMessages[currentMessages.length - 1];
      if (lastMsg && lastMsg.role === "assistant") {
        set({
          docChatMessages: [
            ...currentMessages.slice(0, -1),
            {
              ...lastMsg,
              content: `Error: ${e instanceof Error ? e.message : typeof e === "object" ? JSON.stringify(e) : String(e)}`,
              isStreaming: false,
            },
          ],
          isDocGenerating: false,
        });
      } else {
        set({ isDocGenerating: false });
      }
    }
  },

  stopDocGeneration: async (modelHandle) => {
    if (modelHandle) {
      try {
        await invoke("stop_generation", { modelHandle });
      } catch (e) {
        console.error("Failed to stop doc generation:", e);
      }
    }
    set({ isDocGenerating: false });
  },

  closeDocChat: () => {
    set({
      isDocChatActive: false,
      docChatMessages: [],
      docConversationId: null,
      docTokensPerSecond: 0,
    });
    // Refresh recent chats in background
    get().loadRecentDocChats();
  },

  // ═══════════════════════════════════════════════════════════════
  // Chat History
  // ═══════════════════════════════════════════════════════════════

  loadRecentDocChats: async () => {
    try {
      const sessions = await invoke<DocChatSession[]>("list_doc_chat_sessions", {
        limit: 20,
      });
      set({ recentDocChats: sessions });
    } catch (e) {
      console.error("Failed to load doc chat sessions:", e);
    }
  },

  resumeDocChat: async (session) => {
    try {
      // Load messages from the existing conversation
      const rawMessages = await invoke<{ role: string; content: string }[]>(
        "get_messages",
        { conversationId: session.conversation_id }
      );
      const messages: Message[] = rawMessages.map((m) => ({
        id: crypto.randomUUID(),
        role: m.role as Message["role"],
        content: m.content,
        createdAt: new Date().toISOString(),
      }));

      set({
        selectedDocumentIds: session.document_ids,
        docChatMode: session.mode,
        isDocChatActive: true,
        docChatMessages: messages,
        docConversationId: session.conversation_id,
        docTokensPerSecond: 0,
      });
    } catch (e) {
      console.error("Failed to resume doc chat:", e);
    }
  },

  toggleDocChatPin: async (conversationId) => {
    try {
      const newPinned = await invoke<boolean>("toggle_doc_chat_pin", { conversationId });
      // Update local state optimistically
      set({
        recentDocChats: get()
          .recentDocChats.map((c) =>
            c.conversation_id === conversationId ? { ...c, pinned: newPinned } : c
          )
          .sort((a, b) => {
            if (a.pinned !== b.pinned) return b.pinned ? 1 : -1;
            return b.updated_at.localeCompare(a.updated_at);
          }),
      });
    } catch (e) {
      console.error("Failed to toggle pin:", e);
    }
  },
}));
