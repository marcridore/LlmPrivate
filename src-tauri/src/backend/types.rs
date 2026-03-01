use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type ModelHandle = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLoadParams {
    pub n_gpu_layers: u32,
    pub n_ctx: u32,
    pub n_threads: Option<u32>,
    pub seed: Option<u32>,
    pub use_mmap: bool,
    pub use_mlock: bool,
}

impl Default for ModelLoadParams {
    fn default() -> Self {
        // When compiled with GPU support, offload all layers to GPU by default.
        // llama.cpp clamps this to the actual number of layers in the model.
        let gpu_layers = if cfg!(feature = "cuda") || cfg!(feature = "vulkan") {
            999
        } else {
            0
        };

        Self {
            n_gpu_layers: gpu_layers,
            n_ctx: 4096,
            n_threads: None,
            seed: None,
            use_mmap: true,
            use_mlock: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAttachment {
    pub id: String,
    pub file_path: String,
    #[serde(default)]
    pub alt_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    #[serde(default)]
    pub images: Vec<ImageAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub repeat_penalty: f32,
    pub stop_sequences: Vec<String>,
}

impl Default for GenerationRequest {
    fn default() -> Self {
        Self {
            messages: vec![],
            max_tokens: 2048,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.1,
            stop_sequences: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TokenEvent {
    Token {
        text: String,
        token_index: u32,
    },
    /// Replace the entire accumulated response (used when stop sequences
    /// are detected and the streamed text needs to be trimmed).
    Replace {
        full_text: String,
    },
    Done {
        total_tokens: u32,
        generation_time_ms: u64,
        tokens_per_second: f64,
        prompt_tokens: u32,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponse {
    pub content: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub generation_time_ms: u64,
    pub tokens_per_second: f64,
    pub stop_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub file_path: PathBuf,
    pub file_size_bytes: u64,
    pub architecture: String,
    pub parameter_count: Option<u64>,
    pub quantization: String,
    pub context_length: u32,
    pub embedding_length: Option<u32>,
    pub vocab_size: Option<u32>,
    pub backend: String,
    #[serde(default)]
    pub supports_vision: bool,
    #[serde(default)]
    pub mmproj_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub ram_bytes: u64,
    pub vram_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelEntry {
    pub name: String,
    pub file_path: PathBuf,
    pub file_size_bytes: u64,
    pub quantization: String,
    pub is_loaded: bool,
    pub handle: Option<ModelHandle>,
    #[serde(default)]
    pub has_mmproj: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub model_name: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f64,
    pub speed_bytes_per_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubModelEntry {
    pub repo_id: String,
    pub name: String,
    pub description: String,
    pub downloads: u64,
    pub likes: u64,
    pub files: Vec<HubModelFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubModelFile {
    pub filename: String,
    pub size_bytes: u64,
    pub quantization: Option<String>,
}
