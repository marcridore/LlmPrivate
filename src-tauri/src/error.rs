use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("Backend initialization failed: {0}")]
    BackendInit(String),

    #[error("Model load failed: {0}")]
    ModelLoad(String),

    #[error("Context creation failed: {0}")]
    ContextLoad(String),

    #[error("Model not found: handle {0}")]
    ModelNotFound(u64),

    #[error("No backend available")]
    NoBackend,

    #[error("Generation failed: {0}")]
    Generation(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Download failed: {0}")]
    Download(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Task join error: {0}")]
    TaskJoin(String),

    #[error("Lock contention")]
    LockContention,

    #[error("API server error: {0}")]
    ApiServer(String),

    #[error("Vision processing failed: {0}")]
    Vision(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Download(err.to_string())
    }
}

// AppError implements Serialize, which Tauri automatically converts to InvokeError
