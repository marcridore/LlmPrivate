use async_trait::async_trait;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::backend::types::*;
use crate::error::AppError;

#[async_trait]
pub trait InferenceBackend: Send + Sync {
    fn name(&self) -> &str;

    fn supported_formats(&self) -> &[&str];

    fn supports_gpu(&self) -> bool;

    async fn load_model(
        &self,
        path: PathBuf,
        params: ModelLoadParams,
    ) -> Result<ModelHandle, AppError>;

    async fn unload_model(&self, handle: ModelHandle) -> Result<(), AppError>;

    fn is_model_loaded(&self, handle: ModelHandle) -> bool;

    async fn generate(
        &self,
        handle: ModelHandle,
        request: GenerationRequest,
    ) -> Result<GenerationResponse, AppError>;

    async fn generate_stream(
        &self,
        handle: ModelHandle,
        request: GenerationRequest,
        token_sender: mpsc::UnboundedSender<TokenEvent>,
    ) -> Result<GenerationResponse, AppError>;

    async fn stop_generation(&self, handle: ModelHandle) -> Result<(), AppError>;

    fn get_model_info(&self, handle: ModelHandle) -> Result<ModelInfo, AppError>;

    fn list_loaded_models(&self) -> Vec<(ModelHandle, ModelInfo)>;

    fn get_model_memory_usage(&self, handle: ModelHandle) -> Result<MemoryUsage, AppError>;
}
