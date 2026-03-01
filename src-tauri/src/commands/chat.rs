use tauri::ipc::Channel;
use tauri::State;

use crate::backend::types::*;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    conversation_id: String,
    _messages: Vec<ChatMessage>,
    model_handle: u64,
    params: GenerationRequest,
    on_token: Channel<TokenEvent>,
) -> Result<(), AppError> {
    let backends = state.backends.read().await;
    let backend = backends.default_backend().ok_or(AppError::NoBackend)?;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TokenEvent>();

    let backend_clone = backend.clone();
    let gen_handle = tokio::spawn(async move {
        backend_clone
            .generate_stream(model_handle, params, tx)
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

    // Save assistant message to conversation history
    state
        .db
        .save_message(
            &conversation_id,
            &ChatMessage {
                role: Role::Assistant,
                content: response.content,
                images: vec![],
            },
        )?;

    Ok(())
}

#[tauri::command]
pub async fn stop_generation(
    state: State<'_, AppState>,
    model_handle: u64,
) -> Result<(), AppError> {
    let backends = state.backends.read().await;
    let backend = backends.default_backend().ok_or(AppError::NoBackend)?;
    backend.stop_generation(model_handle).await
}

#[tauri::command]
pub async fn get_conversations(
    state: State<'_, AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ConversationSummary>, AppError> {
    state
        .db
        .list_conversations(limit.unwrap_or(30), offset.unwrap_or(0))
}

#[tauri::command]
pub async fn get_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ChatMessage>, AppError> {
    state.db.get_messages(&conversation_id)
}

#[tauri::command]
pub async fn save_user_message(
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
) -> Result<(), AppError> {
    state.db.save_message(
        &conversation_id,
        &ChatMessage {
            role: Role::User,
            content,
            images: vec![],
        },
    )
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    title: Option<String>,
) -> Result<String, AppError> {
    state.db.create_conversation(title.as_deref())
}

#[tauri::command]
pub async fn cleanup_empty_conversations(
    state: State<'_, AppState>,
) -> Result<u64, AppError> {
    // Also auto-rename old "New Chat" conversations
    let _ = state.db.rename_untitled_conversations();
    state.db.cleanup_empty_conversations()
}

#[tauri::command]
pub async fn delete_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), AppError> {
    state.db.delete_conversation(&conversation_id)
}
