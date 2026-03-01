use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::backend::types::{ChatMessage, ConversationSummary, Role};
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

            PRAGMA foreign_keys = ON;
            ",
        )
        .map_err(|e| AppError::Database(format!("Migration failed: {}", e)))?;
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

    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>, AppError> {
        let conn = self.conn.lock().map_err(|_| AppError::LockContention)?;
        let mut stmt = conn.prepare(
            "SELECT c.id, c.title, c.model_name, c.created_at, c.updated_at,
                    COUNT(m.id) as message_count
             FROM conversations c
             LEFT JOIN messages m ON m.conversation_id = c.id
             GROUP BY c.id
             ORDER BY c.updated_at DESC",
        )?;

        let conversations = stmt
            .query_map([], |row| {
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
}
