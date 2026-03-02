use serde::{Deserialize, Serialize};

/// A folder in the document hierarchy (adjacency list tree).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub position: i32,
    pub created_at: String,
    pub updated_at: String,
    /// Populated client-side from flat list
    #[serde(default)]
    pub children: Vec<DocFolder>,
    /// Number of documents directly in this folder
    #[serde(default)]
    pub document_count: u32,
}

/// Full document record (includes extracted text).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub folder_id: String,
    pub filename: String,
    pub file_path: String,
    pub file_size: u64,
    pub file_type: String,
    pub full_text: String,
    pub chunk_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight document info for list views (no full_text).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: String,
    pub folder_id: String,
    pub filename: String,
    pub file_size: u64,
    pub file_type: String,
    pub chunk_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

/// A chunk of document text with positional info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: String,
    pub document_id: String,
    pub chunk_index: u32,
    pub content: String,
    pub char_offset: u32,
    pub char_length: u32,
}

/// A search result from FTS5 with BM25 ranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSearchResult {
    pub chunk: DocumentChunk,
    pub document_filename: String,
    pub rank: f64,
}

/// Mode for document chat interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentChatMode {
    Chat,
    Summarize,
    Quiz,
}

/// Progress events emitted during document add (via Channel).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AddDocumentProgress {
    CopyingFile,
    ExtractingText,
    TextExtracted { char_count: usize },
    CreatingChunks { chunk_count: usize },
    Indexing,
    Done,
    Error { message: String },
}

/// Summary of a past document chat session (for history list).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocChatSessionSummary {
    pub conversation_id: String,
    pub title: String,
    pub updated_at: String,
    pub mode: String,
    pub message_count: u32,
    pub document_names: Vec<String>,
    pub document_ids: Vec<String>,
    pub pinned: bool,
}
