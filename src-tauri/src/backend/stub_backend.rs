use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::{mpsc, RwLock};

use crate::backend::traits::InferenceBackend;
use crate::backend::types::*;
use crate::error::AppError;

struct LoadedModelEntry {
    info: ModelInfo,
}

/// A stub/mock inference backend used during development before llama-cpp-2 is
/// integrated. Returns deterministic fake responses.
pub struct StubBackend {
    loaded_models: RwLock<HashMap<ModelHandle, LoadedModelEntry>>,
    next_handle: AtomicU64,
}

impl StubBackend {
    pub fn new() -> Self {
        Self {
            loaded_models: RwLock::new(HashMap::new()),
            next_handle: AtomicU64::new(1),
        }
    }
}

#[async_trait]
impl InferenceBackend for StubBackend {
    fn name(&self) -> &str {
        "stub"
    }

    fn supported_formats(&self) -> &[&str] {
        &["gguf"]
    }

    fn supports_gpu(&self) -> bool {
        false
    }

    async fn load_model(
        &self,
        path: PathBuf,
        params: ModelLoadParams,
    ) -> Result<ModelHandle, AppError> {
        let handle = self.next_handle.fetch_add(1, Ordering::SeqCst);

        let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let info = ModelInfo {
            name: file_name,
            file_path: path,
            file_size_bytes: file_size,
            architecture: "stub".to_string(),
            parameter_count: None,
            quantization: "unknown".to_string(),
            context_length: params.n_ctx,
            embedding_length: None,
            vocab_size: None,
            backend: "stub".to_string(),
            supports_vision: false,
            mmproj_path: None,
        };

        self.loaded_models
            .write()
            .await
            .insert(handle, LoadedModelEntry { info });

        Ok(handle)
    }

    async fn unload_model(&self, handle: ModelHandle) -> Result<(), AppError> {
        self.loaded_models
            .write()
            .await
            .remove(&handle)
            .ok_or(AppError::ModelNotFound(handle))?;
        Ok(())
    }

    fn is_model_loaded(&self, handle: ModelHandle) -> bool {
        self.loaded_models
            .try_read()
            .map(|m| m.contains_key(&handle))
            .unwrap_or(false)
    }

    async fn generate(
        &self,
        handle: ModelHandle,
        request: GenerationRequest,
    ) -> Result<GenerationResponse, AppError> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let response = self.generate_stream(handle, request, tx).await?;
        while rx.try_recv().is_ok() {}
        Ok(response)
    }

    async fn generate_stream(
        &self,
        handle: ModelHandle,
        _request: GenerationRequest,
        token_sender: mpsc::UnboundedSender<TokenEvent>,
    ) -> Result<GenerationResponse, AppError> {
        if !self.is_model_loaded(handle) {
            return Err(AppError::ModelNotFound(handle));
        }

        let start = Instant::now();
        let response_text = "Hello! I'm a **stub backend** for LlmPrivate. \
            This response is generated without a real LLM engine. \
            Once `llama-cpp-2` is integrated, you'll get real AI responses here.\n\n\
            Here's a code example:\n\n\
            ```rust\nfn main() {\n    println!(\"Hello from LlmPrivate!\");\n}\n```";

        let words: Vec<&str> = response_text.split(' ').collect();
        for (i, word) in words.iter().enumerate() {
            let text = if i == 0 {
                word.to_string()
            } else {
                format!(" {}", word)
            };

            let _ = token_sender.send(TokenEvent::Token {
                text,
                token_index: i as u32,
            });

            tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        }

        let elapsed = start.elapsed().as_millis() as u64;
        let total_tokens = words.len() as u32;
        let tps = if elapsed > 0 {
            total_tokens as f64 / (elapsed as f64 / 1000.0)
        } else {
            0.0
        };

        let _ = token_sender.send(TokenEvent::Done {
            total_tokens,
            generation_time_ms: elapsed,
            tokens_per_second: tps,
            prompt_tokens: 0,
        });

        Ok(GenerationResponse {
            content: response_text.to_string(),
            prompt_tokens: 0,
            completion_tokens: total_tokens,
            total_tokens,
            generation_time_ms: elapsed,
            tokens_per_second: tps,
            stop_reason: "stop".to_string(),
        })
    }

    async fn stop_generation(&self, _handle: ModelHandle) -> Result<(), AppError> {
        Ok(())
    }

    fn get_model_info(&self, handle: ModelHandle) -> Result<ModelInfo, AppError> {
        self.loaded_models
            .try_read()
            .map_err(|_| AppError::LockContention)?
            .get(&handle)
            .map(|m| m.info.clone())
            .ok_or(AppError::ModelNotFound(handle))
    }

    fn list_loaded_models(&self) -> Vec<(ModelHandle, ModelInfo)> {
        self.loaded_models
            .try_read()
            .map(|models| models.iter().map(|(h, m)| (*h, m.info.clone())).collect())
            .unwrap_or_default()
    }

    fn get_model_memory_usage(&self, _handle: ModelHandle) -> Result<MemoryUsage, AppError> {
        Ok(MemoryUsage {
            ram_bytes: 0,
            vram_bytes: 0,
        })
    }
}
