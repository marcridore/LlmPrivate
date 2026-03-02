use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::backend::types::{ChatMessage, ConversationSummary, Role};
use crate::documents::chunker::TextChunk;
use crate::documents::types::*;
use crate::error::AppError;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(data_dir: PathBuf) -> Result<Self, AppError> {
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("llmprivate.db");
        let conn = Connection::open(db_path)
            .map_err(|e| AppError::Database(format!("Failed to open database: {}", e)))?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    pub fn run_migrations(&self) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS conversations (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL DEFAULT 'New Chat',
                model_name  TEXT NOT NULL DEFAULT '',
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id              TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                role            TEXT NOT NULL,
                content         TEXT NOT NULL,
                token_count     INTEGER,
                created_at      TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_messages_conversation
                ON messages(conversation_id, created_at);

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- Document folder tree (adjacency list for unlimited depth)
            CREATE TABLE IF NOT EXISTS doc_folders (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                parent_id   TEXT REFERENCES doc_folders(id) ON DELETE CASCADE,
                position    INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_doc_folders_parent
                ON doc_folders(parent_id);

            -- Documents (metadata + full extracted text)
            CREATE TABLE IF NOT EXISTS documents (
                id          TEXT PRIMARY KEY,
                folder_id   TEXT NOT NULL REFERENCES doc_folders(id) ON DELETE CASCADE,
                filename    TEXT NOT NULL,
                file_path   TEXT NOT NULL,
                file_size   INTEGER NOT NULL DEFAULT 0,
                file_type   TEXT NOT NULL,
                full_text   TEXT NOT NULL DEFAULT '',
                chunk_count INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_documents_folder
                ON documents(folder_id);

            -- Document chunks for retrieval
            CREATE TABLE IF NOT EXISTS document_chunks (
                id          TEXT PRIMARY KEY,
                document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                chunk_index INTEGER NOT NULL,
                content     TEXT NOT NULL,
                char_offset INTEGER NOT NULL DEFAULT 0,
                char_length INTEGER NOT NULL DEFAULT 0,
                embedding   BLOB,
                created_at  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_document
                ON document_chunks(document_id, chunk_index);

            -- Document chat sessions (links conversations to documents)
            CREATE TABLE IF NOT EXISTS doc_chat_sessions (
                conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
                mode            TEXT NOT NULL DEFAULT 'chat',
                pinned          INTEGER NOT NULL DEFAULT 0,
                created_at      TEXT NOT NULL
            );

            -- Junction table: which documents belong to which chat session
            CREATE TABLE IF NOT EXISTS doc_chat_documents (
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                document_id     TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                PRIMARY KEY (conversation_id, document_id)
            );

            PRAGMA foreign_keys = ON;
            ",
        )
        .map_err(|e| AppError::Database(format!("Migration failed: {}", e)))?;

        // FTS5 virtual table (separate because CREATE VIRTUAL TABLE IF NOT EXISTS
        // syntax requires its own statement)
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                content,
                content='document_chunks',
                content_rowid='rowid'
            );",
        )
        .map_err(|e| AppError::Database(format!("FTS5 migration failed: {}", e)))?;

        Ok(())
    }

    pub fn create_conversation(&self, title: Option<&str>) -> Result<String, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let title = title.unwrap_or("New Chat");

        conn.execute(
            "INSERT INTO conversations (id, title, model_name, created_at, updated_at)
             VALUES (?1, ?2, '', ?3, ?3)",
            rusqlite::params![id, title, now],
        )?;

        Ok(id)
    }

    pub fn list_conversations(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ConversationSummary>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let mut stmt = conn.prepare(
            "SELECT c.id, c.title, c.model_name, c.created_at, c.updated_at,
                    COUNT(m.id) as message_count
             FROM conversations c
             LEFT JOIN messages m ON m.conversation_id = c.id
             GROUP BY c.id
             ORDER BY c.updated_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;

        let conversations = stmt
            .query_map(rusqlite::params![limit, offset], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    model_name: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    message_count: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(conversations)
    }

    /// Auto-rename conversations still titled "New Chat" using the first user message.
    pub fn rename_untitled_conversations(&self) -> Result<u64, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let renamed = conn.execute(
            "UPDATE conversations SET title = (
                SELECT SUBSTR(m.content, 1, 50)
                FROM messages m
                WHERE m.conversation_id = conversations.id AND m.role = 'user'
                ORDER BY m.created_at ASC
                LIMIT 1
            )
            WHERE title = 'New Chat'
            AND EXISTS (
                SELECT 1 FROM messages m
                WHERE m.conversation_id = conversations.id AND m.role = 'user'
            )",
            [],
        )?;
        Ok(renamed as u64)
    }

    pub fn get_messages(&self, conversation_id: &str) -> Result<Vec<ChatMessage>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let mut stmt = conn.prepare(
            "SELECT role, content FROM messages
             WHERE conversation_id = ?1
             ORDER BY created_at ASC",
        )?;

        let messages = stmt
            .query_map(rusqlite::params![conversation_id], |row| {
                let role_str: String = row.get(0)?;
                let role = match role_str.as_str() {
                    "system" => Role::System,
                    "user" => Role::User,
                    "assistant" => Role::Assistant,
                    _ => Role::User,
                };
                Ok(ChatMessage {
                    role,
                    content: row.get(1)?,
                    images: vec![],
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(messages)
    }

    pub fn save_message(
        &self,
        conversation_id: &str,
        message: &ChatMessage,
    ) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let role = match message.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, conversation_id, role, message.content, now],
        )?;

        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, conversation_id],
        )?;

        Ok(())
    }

    pub fn cleanup_empty_conversations(&self) -> Result<u64, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;

        // Delete orphan messages for conversations we're about to remove
        conn.execute(
            "DELETE FROM messages WHERE conversation_id IN (
                SELECT c.id FROM conversations c
                WHERE c.id NOT IN (
                    SELECT DISTINCT conversation_id FROM messages WHERE role = 'user'
                )
            )",
            [],
        )?;

        // Delete conversations that have no user messages
        // (covers both truly empty ones and ones with only assistant responses from old bugs)
        let deleted = conn.execute(
            "DELETE FROM conversations WHERE id NOT IN (
                SELECT DISTINCT conversation_id FROM messages WHERE role = 'user'
            )",
            [],
        )?;
        Ok(deleted as u64)
    }

    pub fn delete_conversation(&self, conversation_id: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            rusqlite::params![conversation_id],
        )?;
        conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            rusqlite::params![conversation_id],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let result = stmt
            .query_row(rusqlite::params![key], |row| row.get(0))
            .ok();
        Ok(result)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    // Document Folders
    // ═══════════════════════════════════════════════════════════════

    pub fn create_folder(
        &self,
        name: &str,
        parent_id: Option<&str>,
    ) -> Result<String, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO doc_folders (id, name, parent_id, position, created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?4)",
            rusqlite::params![id, name, parent_id, now],
        )?;
        Ok(id)
    }

    pub fn rename_folder(&self, folder_id: &str, new_name: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE doc_folders SET name = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![new_name, now, folder_id],
        )?;
        Ok(())
    }

    pub fn delete_folder(&self, folder_id: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;

        // First remove FTS entries for all chunks in documents in this folder
        // (and recursively in subfolders via CASCADE)
        conn.execute(
            "INSERT INTO chunks_fts(chunks_fts, rowid, content)
             SELECT 'delete', dc.rowid, dc.content
             FROM document_chunks dc
             JOIN documents d ON d.id = dc.document_id
             WHERE d.folder_id = ?1",
            rusqlite::params![folder_id],
        )?;

        // CASCADE will handle documents, chunks, and child folders
        conn.execute(
            "DELETE FROM doc_folders WHERE id = ?1",
            rusqlite::params![folder_id],
        )?;
        Ok(())
    }

    pub fn move_folder(
        &self,
        folder_id: &str,
        new_parent_id: Option<&str>,
    ) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE doc_folders SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![new_parent_id, now, folder_id],
        )?;
        Ok(())
    }

    /// Get all folders as a flat list. The frontend builds the tree using parent_id.
    pub fn get_folder_tree(&self) -> Result<Vec<DocFolder>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let mut stmt = conn.prepare(
            "SELECT f.id, f.name, f.parent_id, f.position, f.created_at, f.updated_at,
                    (SELECT COUNT(*) FROM documents d WHERE d.folder_id = f.id) as doc_count
             FROM doc_folders f
             ORDER BY f.position ASC, f.name ASC",
        )?;

        let flat: Vec<DocFolder> = stmt
            .query_map([], |row| {
                Ok(DocFolder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    position: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    children: vec![],
                    document_count: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Build tree from flat list: group children by parent_id
        use std::collections::HashMap;
        let mut by_id: HashMap<String, DocFolder> = HashMap::new();
        let mut child_ids: HashMap<Option<String>, Vec<String>> = HashMap::new();

        for folder in flat {
            child_ids
                .entry(folder.parent_id.clone())
                .or_default()
                .push(folder.id.clone());
            by_id.insert(folder.id.clone(), folder);
        }

        // Recursive helper to assemble tree
        fn build_children(
            parent_id: &Option<String>,
            by_id: &mut HashMap<String, DocFolder>,
            child_ids: &HashMap<Option<String>, Vec<String>>,
        ) -> Vec<DocFolder> {
            let ids = match child_ids.get(parent_id) {
                Some(ids) => ids.clone(),
                None => return vec![],
            };
            let mut result = Vec::new();
            for id in ids {
                if let Some(mut folder) = by_id.remove(&id) {
                    folder.children =
                        build_children(&Some(id.clone()), by_id, child_ids);
                    result.push(folder);
                }
            }
            result
        }

        let roots = build_children(&None, &mut by_id, &child_ids);
        Ok(roots)
    }

    // ═══════════════════════════════════════════════════════════════
    // Documents
    // ═══════════════════════════════════════════════════════════════

    pub fn insert_document(
        &self,
        folder_id: &str,
        filename: &str,
        file_path: &str,
        file_size: u64,
        file_type: &str,
        full_text: &str,
    ) -> Result<String, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO documents (id, folder_id, filename, file_path, file_size, file_type, full_text, chunk_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8)",
            rusqlite::params![id, folder_id, filename, file_path, file_size as i64, file_type, full_text, now],
        )?;
        Ok(id)
    }

    pub fn delete_document(&self, document_id: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;

        // Remove FTS entries first
        conn.execute(
            "INSERT INTO chunks_fts(chunks_fts, rowid, content)
             SELECT 'delete', dc.rowid, dc.content
             FROM document_chunks dc
             WHERE dc.document_id = ?1",
            rusqlite::params![document_id],
        )?;

        // Delete chunks then document (CASCADE would handle this too)
        conn.execute(
            "DELETE FROM document_chunks WHERE document_id = ?1",
            rusqlite::params![document_id],
        )?;
        conn.execute(
            "DELETE FROM documents WHERE id = ?1",
            rusqlite::params![document_id],
        )?;
        Ok(())
    }

    pub fn get_document(&self, document_id: &str) -> Result<Document, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        conn.query_row(
            "SELECT id, folder_id, filename, file_path, file_size, file_type, full_text, chunk_count, created_at, updated_at
             FROM documents WHERE id = ?1",
            rusqlite::params![document_id],
            |row| {
                Ok(Document {
                    id: row.get(0)?,
                    folder_id: row.get(1)?,
                    filename: row.get(2)?,
                    file_path: row.get(3)?,
                    file_size: row.get::<_, i64>(4)? as u64,
                    file_type: row.get(5)?,
                    full_text: row.get(6)?,
                    chunk_count: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )
        .map_err(|e| AppError::NotFound(format!("Document not found: {e}")))
    }

    pub fn get_documents_in_folder(
        &self,
        folder_id: &str,
    ) -> Result<Vec<DocumentSummary>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let mut stmt = conn.prepare(
            "SELECT id, folder_id, filename, file_size, file_type, chunk_count, created_at, updated_at
             FROM documents WHERE folder_id = ?1
             ORDER BY created_at DESC",
        )?;

        let docs = stmt
            .query_map(rusqlite::params![folder_id], |row| {
                Ok(DocumentSummary {
                    id: row.get(0)?,
                    folder_id: row.get(1)?,
                    filename: row.get(2)?,
                    file_size: row.get::<_, i64>(3)? as u64,
                    file_type: row.get(4)?,
                    chunk_count: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(docs)
    }

    // ═══════════════════════════════════════════════════════════════
    // Document Chunks & FTS5
    // ═══════════════════════════════════════════════════════════════

    /// Insert chunks for a document and sync with FTS5 index.
    pub fn insert_chunks(
        &self,
        document_id: &str,
        chunks: &[TextChunk],
    ) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let now = chrono::Utc::now().to_rfc3339();

        // Wrap all inserts in a single transaction for performance.
        // Without this, each INSERT does a separate fsync → extremely slow.
        let tx = conn.unchecked_transaction()?;

        for chunk in chunks {
            let chunk_id = uuid::Uuid::new_v4().to_string();

            tx.execute(
                "INSERT INTO document_chunks (id, document_id, chunk_index, content, char_offset, char_length, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    chunk_id,
                    document_id,
                    chunk.index as i32,
                    chunk.content,
                    chunk.char_offset as i32,
                    chunk.char_length as i32,
                    now,
                ],
            )?;

            // Sync with FTS5 index using the rowid just inserted
            let rowid = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO chunks_fts(rowid, content) VALUES (?1, ?2)",
                rusqlite::params![rowid, chunk.content],
            )?;
        }

        // Update document chunk count
        tx.execute(
            "UPDATE documents SET chunk_count = ?1 WHERE id = ?2",
            rusqlite::params![chunks.len() as i32, document_id],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Search chunks using FTS5 full-text search with BM25 ranking.
    pub fn search_document_chunks(
        &self,
        fts_query: &str,
        document_ids: Option<&[String]>,
        limit: u32,
    ) -> Result<Vec<ChunkSearchResult>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;

        let (sql, params) = if let Some(doc_ids) = document_ids {
            if doc_ids.is_empty() {
                return Ok(vec![]);
            }
            // Build IN clause with placeholders
            let placeholders: Vec<String> = (0..doc_ids.len())
                .map(|i| format!("?{}", i + 3))
                .collect();
            let in_clause = placeholders.join(", ");

            let sql = format!(
                "SELECT dc.id, dc.document_id, dc.chunk_index, dc.content,
                        dc.char_offset, dc.char_length, d.filename, rank
                 FROM chunks_fts
                 JOIN document_chunks dc ON dc.rowid = chunks_fts.rowid
                 JOIN documents d ON d.id = dc.document_id
                 WHERE chunks_fts MATCH ?1
                   AND dc.document_id IN ({in_clause})
                 ORDER BY rank
                 LIMIT ?2"
            );

            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
                Box::new(fts_query.to_string()),
                Box::new(limit),
            ];
            for id in doc_ids {
                params.push(Box::new(id.clone()));
            }
            (sql, params)
        } else {
            let sql = "SELECT dc.id, dc.document_id, dc.chunk_index, dc.content,
                              dc.char_offset, dc.char_length, d.filename, rank
                       FROM chunks_fts
                       JOIN document_chunks dc ON dc.rowid = chunks_fts.rowid
                       JOIN documents d ON d.id = dc.document_id
                       WHERE chunks_fts MATCH ?1
                       ORDER BY rank
                       LIMIT ?2"
                .to_string();

            let params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
                Box::new(fts_query.to_string()),
                Box::new(limit),
            ];
            (sql, params)
        };

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let results = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(ChunkSearchResult {
                    chunk: DocumentChunk {
                        id: row.get(0)?,
                        document_id: row.get(1)?,
                        chunk_index: row.get(2)?,
                        content: row.get(3)?,
                        char_offset: row.get(4)?,
                        char_length: row.get(5)?,
                    },
                    document_filename: row.get(6)?,
                    rank: row.get(7)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    /// Get all chunks for a document, ordered by chunk_index.
    /// Used for summarize/quiz where we want sequential content.
    pub fn get_all_chunks_for_document(
        &self,
        document_id: &str,
    ) -> Result<Vec<ChunkSearchResult>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let mut stmt = conn.prepare(
            "SELECT dc.id, dc.document_id, dc.chunk_index, dc.content,
                    dc.char_offset, dc.char_length, d.filename
             FROM document_chunks dc
             JOIN documents d ON d.id = dc.document_id
             WHERE dc.document_id = ?1
             ORDER BY dc.chunk_index ASC",
        )?;

        let results = stmt
            .query_map(rusqlite::params![document_id], |row| {
                Ok(ChunkSearchResult {
                    chunk: DocumentChunk {
                        id: row.get(0)?,
                        document_id: row.get(1)?,
                        chunk_index: row.get(2)?,
                        content: row.get(3)?,
                        char_offset: row.get(4)?,
                        char_length: row.get(5)?,
                    },
                    document_filename: row.get(6)?,
                    rank: 0.0,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    // ═══════════════════════════════════════════════════════════════
    // Document Chat Sessions
    // ═══════════════════════════════════════════════════════════════

    pub fn create_doc_chat_session(
        &self,
        conversation_id: &str,
        document_ids: &[String],
        mode: &str,
    ) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO doc_chat_sessions (conversation_id, mode, pinned, created_at)
             VALUES (?1, ?2, 0, ?3)",
            rusqlite::params![conversation_id, mode, now],
        )?;

        for doc_id in document_ids {
            conn.execute(
                "INSERT INTO doc_chat_documents (conversation_id, document_id)
                 VALUES (?1, ?2)",
                rusqlite::params![conversation_id, doc_id],
            )?;
        }
        Ok(())
    }

    pub fn list_doc_chat_sessions(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::documents::types::DocChatSessionSummary>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let mut stmt = conn.prepare(
            "SELECT c.id, c.title, c.updated_at, dcs.mode, dcs.pinned,
                    COUNT(m.id) as message_count
             FROM doc_chat_sessions dcs
             JOIN conversations c ON c.id = dcs.conversation_id
             LEFT JOIN messages m ON m.conversation_id = c.id
             GROUP BY c.id
             HAVING message_count > 0
             ORDER BY dcs.pinned DESC, c.updated_at DESC
             LIMIT ?1",
        )?;

        let mut sessions: Vec<crate::documents::types::DocChatSessionSummary> = stmt
            .query_map(rusqlite::params![limit], |row| {
                Ok(crate::documents::types::DocChatSessionSummary {
                    conversation_id: row.get(0)?,
                    title: row.get(1)?,
                    updated_at: row.get(2)?,
                    mode: row.get(3)?,
                    pinned: row.get(4)?,
                    message_count: row.get(5)?,
                    document_names: vec![],
                    document_ids: vec![],
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Fill in document names for each session
        for session in &mut sessions {
            if let Ok(mut doc_stmt) = conn.prepare(
                "SELECT dcd.document_id, d.filename
                 FROM doc_chat_documents dcd
                 LEFT JOIN documents d ON d.id = dcd.document_id
                 WHERE dcd.conversation_id = ?1",
            ) {
                if let Ok(docs) = doc_stmt.query_map(
                    rusqlite::params![session.conversation_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                ) {
                    for doc in docs.flatten() {
                        session.document_ids.push(doc.0);
                        session.document_names.push(doc.1.unwrap_or_else(|| "(deleted)".to_string()));
                    }
                }
            }
        }

        Ok(sessions)
    }

    pub fn get_doc_chat_document_ids(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let mut stmt = conn.prepare(
            "SELECT document_id FROM doc_chat_documents WHERE conversation_id = ?1",
        )?;
        let ids = stmt
            .query_map(rusqlite::params![conversation_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    pub fn toggle_doc_chat_pin(
        &self,
        conversation_id: &str,
    ) -> Result<bool, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        conn.execute(
            "UPDATE doc_chat_sessions SET pinned = NOT pinned WHERE conversation_id = ?1",
            rusqlite::params![conversation_id],
        )?;
        let pinned: bool = conn.query_row(
            "SELECT pinned FROM doc_chat_sessions WHERE conversation_id = ?1",
            rusqlite::params![conversation_id],
            |row| row.get(0),
        )?;
        Ok(pinned)
    }
}
