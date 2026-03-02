use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::backend::types::*;
use crate::documents::types::*;
use crate::error::AppError;
use crate::state::AppState;

// ═══════════════════════════════════════════════════════════════
// Folder commands
// ═══════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn create_doc_folder(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<String, AppError> {
    state.db.create_folder(&name, parent_id.as_deref())
}

#[tauri::command]
pub async fn rename_doc_folder(
    state: State<'_, AppState>,
    folder_id: String,
    new_name: String,
) -> Result<(), AppError> {
    state.db.rename_folder(&folder_id, &new_name)
}

#[tauri::command]
pub async fn delete_doc_folder(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<(), AppError> {
    state.db.delete_folder(&folder_id)
}

#[tauri::command]
pub async fn move_doc_folder(
    state: State<'_, AppState>,
    folder_id: String,
    new_parent_id: Option<String>,
) -> Result<(), AppError> {
    state.db.move_folder(&folder_id, new_parent_id.as_deref())
}

#[tauri::command]
pub async fn get_doc_folder_tree(
    state: State<'_, AppState>,
) -> Result<Vec<DocFolder>, AppError> {
    state.db.get_folder_tree()
}

// ═══════════════════════════════════════════════════════════════
// Document commands
// ═══════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn add_document(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: String,
    file_path: String,
    on_progress: Channel<AddDocumentProgress>,
) -> Result<DocumentSummary, AppError> {
    let source_path = std::path::PathBuf::from(&file_path);

    // Validate file exists
    if !source_path.exists() {
        return Err(AppError::Document(format!("File not found: {file_path}")));
    }

    // Determine file type from extension
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !["pdf", "txt", "md", "docx"].contains(&ext.as_str()) {
        return Err(AppError::Document(format!(
            "Unsupported file type: .{ext}. Supported: pdf, txt, md, docx"
        )));
    }

    let filename = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let file_size = std::fs::metadata(&source_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Step 1: Copy file
    let _ = on_progress.send(AddDocumentProgress::CopyingFile);
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(format!("Cannot resolve app data dir: {e}")))?;
    let docs_dir = app_data.join("documents");
    std::fs::create_dir_all(&docs_dir)?;

    let dest_filename = format!("{}.{}", uuid::Uuid::new_v4(), ext);
    let dest_path = docs_dir.join(&dest_filename);
    std::fs::copy(&source_path, &dest_path)?;

    let dest_path_str = dest_path.to_string_lossy().to_string();

    // Step 2: Extract text (CPU-bound, run on blocking thread)
    let _ = on_progress.send(AddDocumentProgress::ExtractingText);
    let extract_path = dest_path.clone();
    let full_text = tokio::task::spawn_blocking(move || {
        crate::documents::parser::extract_text(&extract_path)
    })
    .await
    .map_err(|e| AppError::Document(format!("Text extraction task failed: {e}")))??;

    let _ = on_progress.send(AddDocumentProgress::TextExtracted {
        char_count: full_text.len(),
    });

    tracing::info!(
        "Extracted {} chars from '{}' ({})",
        full_text.len(),
        filename,
        ext,
    );

    // Step 3: Chunk the text (wrapped in catch_unwind for safety — multi-byte
    // UTF-8 text could previously panic on char boundary issues)
    let chunk_text = full_text.clone();
    let chunks = std::panic::catch_unwind(|| {
        crate::documents::chunker::chunk_text(&chunk_text, 500, 100)
    })
    .map_err(|e| {
        let msg = if let Some(s) = e.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic in chunk_text".to_string()
        };
        tracing::error!("chunk_text panicked: {}", msg);
        AppError::Document(format!("Text chunking failed: {msg}"))
    })?;
    let _ = on_progress.send(AddDocumentProgress::CreatingChunks {
        chunk_count: chunks.len(),
    });
    tracing::info!("Created {} chunks for '{}'", chunks.len(), filename);

    // Step 4: Save to database
    let _ = on_progress.send(AddDocumentProgress::Indexing);
    let doc_id = state.db.insert_document(
        &folder_id,
        &filename,
        &dest_path_str,
        file_size,
        &ext,
        &full_text,
    )?;

    state.db.insert_chunks(&doc_id, &chunks)?;

    // Step 5: Done
    let _ = on_progress.send(AddDocumentProgress::Done);

    // Return summary
    Ok(DocumentSummary {
        id: doc_id,
        folder_id,
        filename,
        file_size,
        file_type: ext,
        chunk_count: chunks.len() as u32,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn delete_document(
    state: State<'_, AppState>,
    document_id: String,
) -> Result<(), AppError> {
    // Also delete the stored file copy
    if let Ok(doc) = state.db.get_document(&document_id) {
        let path = std::path::Path::new(&doc.file_path);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
    state.db.delete_document(&document_id)
}

#[tauri::command]
pub async fn get_document(
    state: State<'_, AppState>,
    document_id: String,
) -> Result<Document, AppError> {
    state.db.get_document(&document_id)
}

#[tauri::command]
pub async fn get_documents_in_folder(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<Vec<DocumentSummary>, AppError> {
    state.db.get_documents_in_folder(&folder_id)
}

// ═══════════════════════════════════════════════════════════════
// Search
// ═══════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn search_document_chunks(
    state: State<'_, AppState>,
    query: String,
    document_ids: Option<Vec<String>>,
    limit: Option<u32>,
) -> Result<Vec<ChunkSearchResult>, AppError> {
    crate::documents::retriever::search_chunks(
        &state.db,
        &query,
        document_ids.as_deref(),
        limit.unwrap_or(10),
    )
}

// ═══════════════════════════════════════════════════════════════
// Document Chat
// ═══════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn chat_with_documents(
    state: State<'_, AppState>,
    conversation_id: String,
    document_ids: Vec<String>,
    _messages: Vec<ChatMessage>,
    model_handle: u64,
    params: GenerationRequest,
    mode: String,
    on_token: Channel<TokenEvent>,
) -> Result<(), AppError> {
    const MAX_CONTEXT_CHARS: usize = 6000;

    tracing::info!(
        "chat_with_documents: mode={}, docs={}, model={}",
        mode,
        document_ids.len(),
        model_handle
    );

    // 1. Build document context based on mode
    let context = match mode.as_str() {
        "chat" => {
            // Find the last user message to use as search query
            let query = params
                .messages
                .iter()
                .rev()
                .find(|m| matches!(m.role, Role::User))
                .map(|m| m.content.as_str())
                .unwrap_or("");

            let chunks = crate::documents::retriever::search_chunks(
                &state.db,
                query,
                Some(&document_ids),
                10,
            )?;

            crate::documents::retriever::build_context_from_chunks(&chunks, MAX_CONTEXT_CHARS)
        }
        "summarize" | "quiz" => {
            // For summarize/quiz, get all chunks sequentially from all selected documents
            let mut all_chunks = Vec::new();
            for doc_id in &document_ids {
                let chunks = state.db.get_all_chunks_for_document(doc_id)?;
                all_chunks.extend(chunks);
            }
            crate::documents::retriever::build_sequential_context(&all_chunks, MAX_CONTEXT_CHARS)
        }
        _ => {
            return Err(AppError::Document(format!(
                "Unknown chat mode: {mode}. Expected: chat, summarize, quiz"
            )));
        }
    };

    if context.trim().is_empty() {
        return Err(AppError::Document(
            "No document content found for the given query/documents.".to_string(),
        ));
    }

    // 2. Build system prompt
    let system_prompt = match mode.as_str() {
        "chat" => format!(
            "You are a helpful assistant answering questions about documents. \
             Use ONLY the provided document content to answer. \
             If the answer is not in the documents, say so.\n\n\
             [DOCUMENT CONTENT]\n{}",
            context
        ),
        "summarize" => format!(
            "Summarize the following document concisely. \
             Highlight key points, main arguments, and important details.\n\n\
             [DOCUMENT CONTENT]\n{}",
            context
        ),
        "quiz" => format!(
            "Generate a quiz with 5 multiple-choice questions based on the following document content. \
             Each question should have 4 options (A-D) with one correct answer. \
             After all questions, provide an answer key.\n\n\
             [DOCUMENT CONTENT]\n{}",
            context
        ),
        _ => unreachable!(),
    };

    // 3. Prepend system message to the request messages
    let mut augmented_messages = vec![ChatMessage {
        role: Role::System,
        content: system_prompt,
        images: vec![],
    }];
    augmented_messages.extend(params.messages.clone());

    let augmented_params = GenerationRequest {
        messages: augmented_messages,
        ..params
    };

    // 4. Generate response using the standard backend (same as send_message)
    let backends = state.backends.read().await;
    let backend = backends.default_backend().ok_or(AppError::NoBackend)?;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TokenEvent>();

    let backend_clone = backend.clone();
    let gen_handle = tokio::spawn(async move {
        backend_clone
            .generate_stream(model_handle, augmented_params, tx)
            .await
    });

    while let Some(event) = rx.recv().await {
        if on_token.send(event).is_err() {
            break;
        }
    }

    let response = gen_handle
        .await
        .map_err(|e| AppError::TaskJoin(e.to_string()))??;

    // 5. Save assistant response to conversation
    state.db.save_message(
        &conversation_id,
        &ChatMessage {
            role: Role::Assistant,
            content: response.content,
            images: vec![],
        },
    )?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════
// Document Chat Sessions (history, resume, pin)
// ═══════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn create_doc_chat_session(
    state: State<'_, AppState>,
    conversation_id: String,
    document_ids: Vec<String>,
    mode: String,
) -> Result<(), AppError> {
    state
        .db
        .create_doc_chat_session(&conversation_id, &document_ids, &mode)
}

#[tauri::command]
pub async fn list_doc_chat_sessions(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<DocChatSessionSummary>, AppError> {
    state.db.list_doc_chat_sessions(limit.unwrap_or(20))
}

#[tauri::command]
pub async fn get_doc_chat_document_ids(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<String>, AppError> {
    state.db.get_doc_chat_document_ids(&conversation_id)
}

#[tauri::command]
pub async fn toggle_doc_chat_pin(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<bool, AppError> {
    state.db.toggle_doc_chat_pin(&conversation_id)
}
