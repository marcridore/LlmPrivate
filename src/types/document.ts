export interface DocFolder {
  id: string;
  name: string;
  parent_id: string | null;
  position: number;
  created_at: string;
  updated_at: string;
  children: DocFolder[];
  document_count: number;
}

export interface DocumentSummary {
  id: string;
  folder_id: string;
  filename: string;
  file_size: number;
  file_type: string; // "pdf" | "txt" | "md" | "docx"
  chunk_count: number;
  created_at: string;
  updated_at: string;
}

export interface Document extends DocumentSummary {
  full_text: string;
  file_path: string;
}

export interface ChunkSearchResult {
  chunk: {
    id: string;
    document_id: string;
    chunk_index: number;
    content: string;
    char_offset: number;
    char_length: number;
  };
  document_filename: string;
  rank: number;
}

export type DocumentChatMode = "chat" | "summarize" | "quiz";

export type AddDocumentProgressEvent =
  | { type: "CopyingFile" }
  | { type: "ExtractingText" }
  | { type: "TextExtracted"; char_count: number }
  | { type: "CreatingChunks"; chunk_count: number }
  | { type: "Indexing" }
  | { type: "Done" }
  | { type: "Error"; message: string };

export interface DocChatSession {
  conversation_id: string;
  title: string;
  updated_at: string;
  mode: DocumentChatMode;
  message_count: number;
  document_names: string[];
  document_ids: string[];
  pinned: boolean;
}
