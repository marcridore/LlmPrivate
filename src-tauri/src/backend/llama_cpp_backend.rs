use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::{mpsc, RwLock};

use crate::backend::traits::InferenceBackend;
use crate::backend::types::*;
use crate::error::AppError;

// ── Types for the model thread ──────────────────────────────────────────────

enum SessionCommand {
    Generate {
        request: GenerationRequest,
        token_tx: mpsc::UnboundedSender<TokenEvent>,
        done_tx: tokio::sync::oneshot::Sender<Result<GenerationResponse, AppError>>,
        cancel_flag: Arc<AtomicBool>,
    },
    #[allow(dead_code)]
    GetInfo {
        done_tx: tokio::sync::oneshot::Sender<ModelInfo>,
    },
    Shutdown,
}

struct ModelSession {
    cmd_tx: std::sync::mpsc::Sender<SessionCommand>,
    thread: Option<std::thread::JoinHandle<()>>,
    #[allow(dead_code)]
    info: ModelInfo,
}

// ── The real backend ────────────────────────────────────────────────────────

pub struct LlamaCppBackend {
    loaded_models: RwLock<HashMap<ModelHandle, ModelSession>>,
    active_cancels: RwLock<HashMap<ModelHandle, Arc<AtomicBool>>>,
    next_handle: AtomicU64,
    backend: Arc<llama_cpp_2::llama_backend::LlamaBackend>,
}

impl LlamaCppBackend {
    pub fn new() -> Result<Self, AppError> {
        let backend = llama_cpp_2::llama_backend::LlamaBackend::init()
            .map_err(|e| AppError::BackendInit(format!("Failed to init llama.cpp: {e}")))?;

        tracing::info!("llama.cpp backend initialized successfully");

        Ok(Self {
            loaded_models: RwLock::new(HashMap::new()),
            active_cancels: RwLock::new(HashMap::new()),
            next_handle: AtomicU64::new(1),
            backend: Arc::new(backend),
        })
    }
}

// ── Helper functions ────────────────────────────────────────────────────────

fn build_model_info(
    model: &llama_cpp_2::model::LlamaModel,
    path: &PathBuf,
    params: &ModelLoadParams,
) -> ModelInfo {
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let file_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    ModelInfo {
        name: file_name,
        file_path: path.clone(),
        file_size_bytes: file_size,
        architecture: "gguf".to_string(),
        parameter_count: None,
        quantization: "auto".to_string(),
        context_length: params.n_ctx,
        embedding_length: None,
        vocab_size: Some(model.n_vocab() as u32),
        backend: "llama.cpp".to_string(),
    }
}

// ── Core generation logic (runs on the model thread) ────────────────────────

fn run_generation(
    backend: &llama_cpp_2::llama_backend::LlamaBackend,
    model: &llama_cpp_2::model::LlamaModel,
    load_params: &ModelLoadParams,
    request: GenerationRequest,
    token_tx: &mpsc::UnboundedSender<TokenEvent>,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<GenerationResponse, AppError> {
    let start = Instant::now();

    // Build the prompt from chat messages
    let prompt = format_chat_prompt(model, &request.messages);
    tracing::info!("Prompt length: {} chars", prompt.len());

    // Create context
    let n_ctx = load_params.n_ctx;
    let n_threads = load_params.n_threads.unwrap_or_else(|| {
        let cpus = num_cpus();
        std::cmp::max(1, cpus / 2) as u32
    });

    let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(n_ctx))
        .with_n_threads(n_threads as i32)
        .with_n_threads_batch(n_threads as i32);

    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| AppError::ContextLoad(format!("Failed to create context: {e}")))?;

    // Tokenize the prompt
    let tokens_list = model
        .str_to_token(&prompt, llama_cpp_2::model::AddBos::Always)
        .map_err(|e| AppError::Generation(format!("Tokenization failed: {e}")))?;

    // ── FIX #3: Guard against empty token list ──────────────────────────
    if tokens_list.is_empty() {
        return Err(AppError::Generation("Tokenization produced empty result".to_string()));
    }

    let prompt_token_count = tokens_list.len() as u32;

    // ── FIX #2: Context length overflow protection ──────────────────────
    // Reserve at least 128 tokens for generation. If prompt is too long, truncate it.
    let max_prompt_tokens = (n_ctx as usize).saturating_sub(128);
    let tokens_to_process = if tokens_list.len() > max_prompt_tokens {
        tracing::warn!(
            "Prompt ({} tokens) exceeds context limit ({}). Truncating to last {} tokens.",
            tokens_list.len(), n_ctx, max_prompt_tokens
        );
        // Keep the LAST max_prompt_tokens tokens (most recent conversation context)
        &tokens_list[tokens_list.len() - max_prompt_tokens..]
    } else {
        &tokens_list[..]
    };

    let actual_prompt_tokens = tokens_to_process.len() as u32;
    let max_generation = std::cmp::min(
        request.max_tokens as i32,
        n_ctx as i32 - actual_prompt_tokens as i32,
    );
    let n_len = actual_prompt_tokens as i32 + max_generation;

    tracing::info!(
        "Prompt tokens: {} (of {} total), max generation: {}, context: {}",
        actual_prompt_tokens, prompt_token_count, max_generation, n_ctx
    );

    if max_generation <= 0 {
        return Err(AppError::Generation(format!(
            "Context full: {} prompt tokens fills the entire {} context. Try a shorter conversation.",
            actual_prompt_tokens, n_ctx
        )));
    }

    // Create batch and add prompt tokens
    let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(n_ctx as usize, 1);

    let last_index = (tokens_to_process.len() - 1) as i32; // Safe: checked non-empty above
    for (i, token) in tokens_to_process.iter().enumerate() {
        let is_last = i as i32 == last_index;
        batch
            .add(*token, i as i32, &[0], is_last)
            .map_err(|e| AppError::Generation(format!("Batch add failed: {e}")))?;
    }

    // Decode the prompt
    ctx.decode(&mut batch)
        .map_err(|e| AppError::Generation(format!("Prompt decode failed: {e}")))?;

    tracing::info!("Prompt decoded, starting generation...");

    // Build sampler chain
    let mut sampler = build_sampler(&request);

    // UTF-8 decoder for token-to-text
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    let mut n_cur = tokens_to_process.len() as i32;
    let mut n_decode: u32 = 0;
    let mut full_response = String::new();

    // ── Token generation loop ───────────────────────────────────────────
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            tracing::info!("Generation cancelled after {n_decode} tokens");
            let _ = token_tx.send(TokenEvent::Done {
                total_tokens: n_decode,
                generation_time_ms: start.elapsed().as_millis() as u64,
                tokens_per_second: calc_tps(n_decode, &start),
                prompt_tokens: actual_prompt_tokens,
            });
            return Ok(build_response(
                full_response,
                actual_prompt_tokens,
                n_decode,
                &start,
                "cancelled",
            ));
        }

        if n_cur >= n_len {
            tracing::info!("Reached max generation length at {n_decode} tokens");
            break;
        }

        // Sample a token
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);

        // Check for end-of-generation
        if model.is_eog_token(token) {
            tracing::info!("EOS token reached after {n_decode} tokens");
            break;
        }

        // Decode token to text
        let piece = model
            .token_to_piece(token, &mut decoder, false, None)
            .map_err(|e| AppError::Generation(format!("Token decode failed: {e}")))?;

        if !piece.is_empty() {
            full_response.push_str(&piece);

            let _ = token_tx.send(TokenEvent::Token {
                text: piece,
                token_index: n_decode,
            });
        }

        n_decode += 1;

        // Prepare next batch
        batch.clear();
        batch
            .add(token, n_cur, &[0], true)
            .map_err(|e| AppError::Generation(format!("Batch add failed: {e}")))?;
        n_cur += 1;

        ctx.decode(&mut batch)
            .map_err(|e| AppError::Generation(format!("Decode failed at token {n_decode}: {e}")))?;
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let tps = calc_tps(n_decode, &start);

    tracing::info!("Generation complete: {n_decode} tokens in {elapsed_ms}ms ({tps:.1} t/s)");

    let _ = token_tx.send(TokenEvent::Done {
        total_tokens: n_decode,
        generation_time_ms: elapsed_ms,
        tokens_per_second: tps,
        prompt_tokens: actual_prompt_tokens,
    });

    Ok(build_response(
        full_response,
        actual_prompt_tokens,
        n_decode,
        &start,
        "stop",
    ))
}

fn build_sampler(request: &GenerationRequest) -> llama_cpp_2::sampling::LlamaSampler {
    use llama_cpp_2::sampling::LlamaSampler;

    let mut samplers = vec![];

    if request.repeat_penalty != 1.0 {
        samplers.push(LlamaSampler::penalties(
            64,
            request.repeat_penalty,
            0.0,
            0.0,
        ));
    }

    if request.temperature > 0.0 {
        samplers.push(LlamaSampler::top_k(request.top_k as i32));
        samplers.push(LlamaSampler::top_p(request.top_p, 1));
        samplers.push(LlamaSampler::temp(request.temperature));
        samplers.push(LlamaSampler::dist(1234));
    } else {
        samplers.push(LlamaSampler::greedy());
    }

    LlamaSampler::chain_simple(samplers)
}

fn format_chat_prompt(
    model: &llama_cpp_2::model::LlamaModel,
    messages: &[ChatMessage],
) -> String {
    let chat_messages: Vec<llama_cpp_2::model::LlamaChatMessage> = messages
        .iter()
        .filter_map(|m| {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            llama_cpp_2::model::LlamaChatMessage::new(role.to_string(), m.content.clone()).ok()
        })
        .collect();

    if let Ok(template) = model.chat_template(None) {
        if let Ok(formatted) = model.apply_chat_template(&template, &chat_messages, true) {
            tracing::info!("Using model's built-in chat template");
            return formatted;
        }
    }

    // Fallback: ChatML format
    tracing::info!("Using fallback ChatML template");
    let mut prompt = String::new();
    for msg in messages {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", role, msg.content));
    }
    prompt.push_str("<|im_start|>assistant\n");
    prompt
}

fn calc_tps(n_decode: u32, start: &Instant) -> f64 {
    let elapsed_ms = start.elapsed().as_millis() as f64;
    if elapsed_ms > 0.0 {
        n_decode as f64 / (elapsed_ms / 1000.0)
    } else {
        0.0
    }
}

fn build_response(
    content: String,
    prompt_tokens: u32,
    completion_tokens: u32,
    start: &Instant,
    stop_reason: &str,
) -> GenerationResponse {
    let elapsed_ms = start.elapsed().as_millis() as u64;
    GenerationResponse {
        content,
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
        generation_time_ms: elapsed_ms,
        tokens_per_second: calc_tps(completion_tokens, start),
        stop_reason: stop_reason.to_string(),
    }
}

fn num_cpus() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4)
}

// ── InferenceBackend implementation ─────────────────────────────────────────

#[async_trait]
impl InferenceBackend for LlamaCppBackend {
    fn name(&self) -> &str {
        "llama.cpp"
    }

    fn supported_formats(&self) -> &[&str] {
        &["gguf"]
    }

    fn supports_gpu(&self) -> bool {
        cfg!(feature = "cuda") || cfg!(feature = "vulkan")
    }

    async fn load_model(
        &self,
        path: PathBuf,
        params: ModelLoadParams,
    ) -> Result<ModelHandle, AppError> {
        let handle = self.next_handle.fetch_add(1, Ordering::SeqCst);

        tracing::info!("load_model called for path: {}", path.display());
        if !path.exists() {
            tracing::error!("Model file not found: {}", path.display());
            return Err(AppError::ModelLoad(format!(
                "Model file not found: {}",
                path.display()
            )));
        }
        let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        tracing::info!("File exists, size: {:.1} GB", file_size as f64 / 1_073_741_824.0);

        let backend = Arc::clone(&self.backend);
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let model_path = path.clone();
        let params_clone = params.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<ModelInfo, String>>();

        let thread = std::thread::Builder::new()
            .name(format!("model-{}", handle))
            .spawn(move || {
                tracing::info!("Model thread {} started for: {}", handle, model_path.display());

                let gpu_layers = params_clone.n_gpu_layers;
                let model_params = llama_cpp_2::model::params::LlamaModelParams::default()
                    .with_n_gpu_layers(gpu_layers);

                tracing::info!(
                    "Loading GGUF model from file (gpu_layers={}, gpu_compiled={})...",
                    gpu_layers,
                    cfg!(feature = "cuda") || cfg!(feature = "vulkan")
                );
                let model = match llama_cpp_2::model::LlamaModel::load_from_file(
                    &backend,
                    &model_path,
                    &model_params,
                ) {
                    Ok(m) => {
                        tracing::info!("Model loaded! vocab_size={}", m.n_vocab());
                        m
                    }
                    Err(e) => {
                        tracing::error!("Model load from file failed: {e}");
                        let _ = ready_tx.send(Err(format!("Failed to load GGUF: {e}")));
                        return;
                    }
                };

                let info = build_model_info(&model, &model_path, &params_clone);
                let _ = ready_tx.send(Ok(info));

                tracing::info!("Model thread {} ready, entering command loop", handle);

                // ── FIX #1: Panic-safe command loop ─────────────────────
                loop {
                    let cmd = match cmd_rx.recv() {
                        Ok(cmd) => cmd,
                        Err(_) => break,
                    };

                    match cmd {
                        SessionCommand::Generate {
                            request,
                            token_tx,
                            done_tx,
                            cancel_flag,
                        } => {
                            tracing::info!("Generation request on model thread {}", handle);

                            // Catch panics from llama.cpp so the thread survives
                            let result = std::panic::catch_unwind(
                                std::panic::AssertUnwindSafe(|| {
                                    run_generation(
                                        &backend,
                                        &model,
                                        &params_clone,
                                        request,
                                        &token_tx,
                                        &cancel_flag,
                                    )
                                }),
                            );

                            let response = match result {
                                Ok(gen_result) => gen_result,
                                Err(panic_info) => {
                                    let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                                        format!("Inference panic: {s}")
                                    } else if let Some(s) = panic_info.downcast_ref::<String>() {
                                        format!("Inference panic: {s}")
                                    } else {
                                        "Inference panic (unknown cause)".to_string()
                                    };
                                    tracing::error!("{msg}");
                                    let _ = token_tx.send(TokenEvent::Error {
                                        message: msg.clone(),
                                    });
                                    Err(AppError::Generation(msg))
                                }
                            };

                            let _ = done_tx.send(response);
                        }
                        SessionCommand::GetInfo { done_tx } => {
                            let info = build_model_info(&model, &model_path, &params_clone);
                            let _ = done_tx.send(info);
                        }
                        SessionCommand::Shutdown => {
                            tracing::info!("Model thread {} shutting down", handle);
                            break;
                        }
                    }
                }
            })
            .map_err(|e| AppError::ModelLoad(format!("Failed to spawn model thread: {e}")))?;

        tracing::info!("Waiting for model thread to finish loading...");
        let info = tokio::time::timeout(std::time::Duration::from_secs(300), ready_rx)
            .await
            .map_err(|_| {
                tracing::error!("Model load timed out after 5 minutes");
                AppError::ModelLoad("Model load timed out (5 min)".to_string())
            })?
            .map_err(|_| {
                tracing::error!("Model thread died during load (channel dropped)");
                AppError::ModelLoad("Model thread died during load".to_string())
            })?
            .map_err(|e| {
                tracing::error!("Model thread reported error: {e}");
                AppError::ModelLoad(e)
            })?;

        let session = ModelSession {
            cmd_tx,
            thread: Some(thread),
            info,
        };

        self.loaded_models.write().await.insert(handle, session);
        tracing::info!("Model loaded with handle {handle} — ready for inference");
        Ok(handle)
    }

    async fn unload_model(&self, handle: ModelHandle) -> Result<(), AppError> {
        let mut models = self.loaded_models.write().await;
        let mut session = models
            .remove(&handle)
            .ok_or(AppError::ModelNotFound(handle))?;

        let _ = session.cmd_tx.send(SessionCommand::Shutdown);

        if let Some(thread) = session.thread.take() {
            let _ = thread.join();
        }

        self.active_cancels.write().await.remove(&handle);
        tracing::info!("Model unloaded: handle {handle}");
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
        request: GenerationRequest,
        token_sender: mpsc::UnboundedSender<TokenEvent>,
    ) -> Result<GenerationResponse, AppError> {
        let models = self.loaded_models.read().await;
        let session = models
            .get(&handle)
            .ok_or(AppError::ModelNotFound(handle))?;

        let cancel_flag = Arc::new(AtomicBool::new(false));

        self.active_cancels
            .write()
            .await
            .insert(handle, cancel_flag.clone());

        let (done_tx, done_rx) = tokio::sync::oneshot::channel();

        session
            .cmd_tx
            .send(SessionCommand::Generate {
                request,
                token_tx: token_sender,
                done_tx,
                cancel_flag,
            })
            .map_err(|_| AppError::Generation("Model thread not responding".to_string()))?;

        drop(models);

        let result = done_rx
            .await
            .map_err(|_| AppError::Generation("Model thread died during generation".to_string()))?;

        self.active_cancels.write().await.remove(&handle);

        result
    }

    async fn stop_generation(&self, handle: ModelHandle) -> Result<(), AppError> {
        let cancels = self.active_cancels.read().await;
        if let Some(flag) = cancels.get(&handle) {
            flag.store(true, Ordering::Relaxed);
            tracing::info!("Stop signal sent for model handle {handle}");
        }
        Ok(())
    }

    fn get_model_info(&self, handle: ModelHandle) -> Result<ModelInfo, AppError> {
        self.loaded_models
            .try_read()
            .map_err(|_| AppError::LockContention)?
            .get(&handle)
            .map(|s| s.info.clone())
            .ok_or(AppError::ModelNotFound(handle))
    }

    fn list_loaded_models(&self) -> Vec<(ModelHandle, ModelInfo)> {
        self.loaded_models
            .try_read()
            .map(|models| {
                models
                    .iter()
                    .map(|(h, s)| (*h, s.info.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_model_memory_usage(&self, _handle: ModelHandle) -> Result<MemoryUsage, AppError> {
        Ok(MemoryUsage {
            ram_bytes: 0,
            vram_bytes: 0,
        })
    }
}
