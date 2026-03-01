use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
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
        // Redirect llama.cpp's C-level logging to our tracing system instead of
        // letting it write to stderr (which is NUL on Windows to suppress CRT assertions).
        unsafe {
            unsafe extern "C" fn tracing_log(
                level: llama_cpp_sys_2::ggml_log_level,
                text: *const ::std::os::raw::c_char,
                _user_data: *mut ::std::os::raw::c_void,
            ) {
                if text.is_null() {
                    return;
                }
                let msg = std::ffi::CStr::from_ptr(text).to_string_lossy();
                let msg = msg.trim();
                if msg.is_empty() {
                    return;
                }
                // ggml_log_level: 0=NONE, 1=DEBUG, 2=INFO, 3=WARN, 4=ERROR, 5=CONT
                match level {
                    4 => tracing::error!("[llama.cpp] {}", msg),
                    3 => tracing::warn!("[llama.cpp] {}", msg),
                    2 => tracing::info!("[llama.cpp] {}", msg),
                    _ => tracing::debug!("[llama.cpp] {}", msg),
                }
            }
            llama_cpp_sys_2::llama_log_set(Some(tracing_log), std::ptr::null_mut());

            // Also redirect clip/mtmd logging (separate system from llama_log_set).
            // Without this, mtmd errors go to stderr which is NUL on Windows.
            #[cfg(feature = "mtmd")]
            llama_cpp_sys_2::mtmd_log_set(Some(tracing_log), std::ptr::null_mut());
        }

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
        supports_vision: false,
        mmproj_path: None,
    }
}

// ── Multimodal helpers ────────────────────────────────────────────────────

/// Scan the model's directory for a matching mmproj GGUF file.
///
/// Strategy:
/// 1. Check recommended model configs for a known filename→mmproj mapping
/// 2. Score by meaningful token overlap between model and mmproj names
/// 3. If exactly one mmproj exists, use it (unambiguous)
/// 4. Otherwise return None to avoid mismatches
pub fn find_mmproj_file(model_path: &Path) -> Option<PathBuf> {
    let parent = model_path.parent()?;
    let model_filename = model_path.file_name()?.to_str()?.to_lowercase();

    let entries: Vec<PathBuf> = std::fs::read_dir(parent)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.ends_with(".gguf") && name.contains("mmproj")
        })
        .map(|e| e.path())
        .collect();

    if entries.is_empty() {
        return None;
    }

    // Strategy 1: Use recommended model configs for known mappings
    for rec in crate::models::recommended::get_recommended_models() {
        if let Some(ref mmproj_name) = rec.mmproj_filename {
            if rec.filename.to_lowercase() == model_filename {
                let expected = parent.join(mmproj_name);
                if expected.exists() {
                    tracing::info!(
                        "Matched mmproj via recommended config: {} → {}",
                        model_filename, mmproj_name
                    );
                    return Some(expected);
                }
            }
        }
    }

    // Strategy 2: Score by meaningful token overlap.
    // We aggressively filter out generic tokens so that only the model's
    // identity name (e.g. "smolvlm2", "gemma", "phi") drives the match.
    let model_stem = model_path.file_stem()?.to_str()?.to_lowercase();
    const GENERIC: &[&str] = &[
        "model", "ggml", "gguf", "mmproj", "f16", "f32",
        "q2", "q3", "q4", "q5", "q6", "q8", "k", "m", "s", "l",
        "text", "ct", "instruct", "it", "chat", "base", "v1", "v2",
    ];

    /// Returns true if the token is too generic to be meaningful for matching:
    /// pure numbers ("2", "16"), size tokens ("2b", "7b", "13b"), version-like ("v1.5").
    fn is_generic_token(t: &str) -> bool {
        if GENERIC.contains(&t) {
            return true;
        }
        // Pure numeric: "2", "16", "1024"
        if t.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
        // Size tokens: "2b", "7b", "13b", "0b"
        if t.len() >= 2
            && t.ends_with('b')
            && t[..t.len() - 1].chars().all(|c| c.is_ascii_digit())
        {
            return true;
        }
        // Single character
        if t.len() == 1 {
            return true;
        }
        false
    }

    let model_tokens: Vec<&str> = model_stem
        .split(|c: char| c == '-' || c == '_' || c == '.')
        .filter(|t| !t.is_empty() && !is_generic_token(t))
        .collect();

    tracing::debug!(
        "mmproj matching: model_stem={} meaningful_tokens={:?}",
        model_stem, model_tokens
    );

    let mut best: Option<(PathBuf, usize)> = None;
    for entry in &entries {
        let mmproj_name = entry
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let mmproj_tokens: Vec<&str> = mmproj_name
            .split(|c: char| c == '-' || c == '_' || c == '.')
            .filter(|t| !t.is_empty() && !is_generic_token(t))
            .collect();

        let score = model_tokens
            .iter()
            .filter(|t| mmproj_tokens.contains(t))
            .count();

        tracing::debug!(
            "mmproj candidate: {} tokens={:?} score={}",
            mmproj_name, mmproj_tokens, score
        );

        if let Some((_, best_score)) = &best {
            if score > *best_score {
                best = Some((entry.clone(), score));
            }
        } else if score > 0 {
            best = Some((entry.clone(), score));
        }
    }

    match best {
        Some((path, score)) if score > 0 => {
            tracing::info!(
                "Matched mmproj: {} (score={})",
                path.display(), score
            );
            Some(path)
        }
        // Don't fallback to a random mmproj just because there's only one.
        // A mismatched mmproj (e.g. SmolVLM2 mmproj with Gemma) will cause
        // the vision server to hang or produce garbage.
        _ => {
            tracing::info!(
                "No mmproj match for model: {} (candidates: {})",
                model_stem, entries.len()
            );
            None
        }
    }
}

#[cfg(feature = "mtmd")]
fn format_multimodal_prompt(
    model: &llama_cpp_2::model::LlamaModel,
    messages: &[ChatMessage],
) -> (String, Vec<String>) {
    let media_marker = llama_cpp_2::mtmd::mtmd_default_marker();
    let mut image_paths = Vec::new();

    let chat_messages: Vec<llama_cpp_2::model::LlamaChatMessage> = messages
        .iter()
        .filter_map(|m| {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            };

            let content = if !m.images.is_empty() && matches!(m.role, Role::User) {
                let markers: String = m.images.iter().map(|img| {
                    image_paths.push(img.file_path.clone());
                    format!("{}\n", media_marker)
                }).collect();
                format!("{}{}", markers, m.content)
            } else {
                m.content.clone()
            };

            llama_cpp_2::model::LlamaChatMessage::new(role.to_string(), content).ok()
        })
        .collect();

    if let Ok(template) = model.chat_template(None) {
        if let Ok(formatted) = model.apply_chat_template(&template, &chat_messages, true) {
            return (formatted, image_paths);
        }
    }

    // Fallback ChatML
    let mut prompt = String::new();
    for msg in messages {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        let content = if !msg.images.is_empty() && matches!(msg.role, Role::User) {
            let markers: String = msg.images.iter().map(|_| {
                format!("{}\n", media_marker)
            }).collect();
            format!("{}{}", markers, msg.content)
        } else {
            msg.content.clone()
        };
        prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", role, content));
    }
    prompt.push_str("<|im_start|>assistant\n");
    (prompt, image_paths)
}

#[cfg(feature = "mtmd")]
fn run_generation_multimodal(
    backend: &llama_cpp_2::llama_backend::LlamaBackend,
    model: &llama_cpp_2::model::LlamaModel,
    mtmd_ctx: &llama_cpp_2::mtmd::MtmdContext,
    load_params: &ModelLoadParams,
    request: GenerationRequest,
    token_tx: &mpsc::UnboundedSender<TokenEvent>,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<GenerationResponse, AppError> {
    let start = Instant::now();

    let stop_sequences = collect_stop_sequences(&request);

    // 1. Build prompt with <__media__> markers
    let (prompt_text, image_paths) = format_multimodal_prompt(model, &request.messages);

    let media_marker = llama_cpp_2::mtmd::mtmd_default_marker();
    let marker_count = prompt_text.matches(media_marker).count();

    if marker_count != image_paths.len() {
        return Err(AppError::Vision(format!(
            "Marker count ({}) != image count ({})",
            marker_count, image_paths.len()
        )));
    }

    tracing::info!(
        "Multimodal prompt: {} images, {} chars",
        image_paths.len(), prompt_text.len()
    );

    // 2. Load bitmaps
    let bitmaps: Vec<llama_cpp_2::mtmd::MtmdBitmap> = image_paths
        .iter()
        .map(|path| {
            llama_cpp_2::mtmd::MtmdBitmap::from_file(mtmd_ctx, path)
                .map_err(|e| AppError::Vision(format!(
                    "Failed to load image '{}': {}", path, e
                )))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let bitmap_refs: Vec<&llama_cpp_2::mtmd::MtmdBitmap> = bitmaps.iter().collect();

    // 3. Tokenize with mtmd
    let input_text = llama_cpp_2::mtmd::MtmdInputText {
        text: prompt_text,
        add_special: true,
        parse_special: true,
    };

    let chunks = mtmd_ctx.tokenize(input_text, &bitmap_refs)
        .map_err(|e| AppError::Vision(format!("Multimodal tokenization failed: {e}")))?;

    let total_tokens = chunks.total_tokens();
    tracing::info!(
        "Multimodal tokenized: {} chunks, {} total tokens",
        chunks.len(), total_tokens
    );

    // 4. Create context
    let n_ctx = load_params.n_ctx;
    let n_threads = load_params.n_threads.unwrap_or_else(|| {
        std::cmp::max(1, num_cpus() / 2) as u32
    });

    let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(n_ctx))
        .with_n_threads(n_threads as i32)
        .with_n_threads_batch(n_threads as i32);

    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| AppError::ContextLoad(format!("Failed to create context: {e}")))?;

    // 5. Evaluate all chunks (replaces manual batch decode)
    let n_past = chunks.eval_chunks(
        mtmd_ctx,
        &ctx,
        0,
        0,
        n_ctx as i32,
        true,
    ).map_err(|e| AppError::Vision(format!("eval_chunks failed: {e}")))?;

    let prompt_token_count = total_tokens as u32;
    let max_generation = std::cmp::min(
        request.max_tokens as i32,
        n_ctx as i32 - n_past,
    );

    if max_generation <= 0 {
        return Err(AppError::Generation(format!(
            "Context full after multimodal prompt ({} positions)", n_past
        )));
    }

    let n_len = n_past + max_generation;

    tracing::info!(
        "Multimodal prompt evaluated: {} positions, max generation: {}",
        n_past, max_generation
    );

    // 6. Token generation loop (same as text-only)
    let mut sampler = build_sampler(&request);
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut n_cur = n_past;
    let mut n_decode: u32 = 0;
    let mut full_response = String::new();
    let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(n_ctx as usize, 1);

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            tracing::info!("Generation cancelled after {n_decode} tokens");
            let _ = token_tx.send(TokenEvent::Done {
                total_tokens: n_decode,
                generation_time_ms: start.elapsed().as_millis() as u64,
                tokens_per_second: calc_tps(n_decode, &start),
                prompt_tokens: prompt_token_count,
            });
            return Ok(build_response(
                full_response, prompt_token_count, n_decode, &start, "cancelled",
            ));
        }

        if n_cur >= n_len {
            break;
        }

        // Sample token — use -1 to sample from last logit position
        let sample_idx = if n_decode == 0 { -1i32 } else { batch.n_tokens() - 1 };
        let token = sampler.sample(&ctx, sample_idx);
        sampler.accept(token);

        if model.is_eog_token(token) {
            tracing::info!("EOS token reached after {n_decode} tokens");
            break;
        }

        let piece = model
            .token_to_piece(token, &mut decoder, false, None)
            .map_err(|e| AppError::Generation(format!("Token decode failed: {e}")))?;

        if !piece.is_empty() {
            full_response.push_str(&piece);

            // Check for stop sequences
            let mut hit_stop = false;
            for seq in &stop_sequences {
                if let Some(pos) = full_response.rfind(seq.as_str()) {
                    full_response.truncate(pos);
                    hit_stop = true;
                    tracing::info!("Stop sequence '{}' hit after {n_decode} tokens", seq);
                    let _ = token_tx.send(TokenEvent::Replace {
                        full_text: full_response.clone(),
                    });
                    break;
                }
            }
            if hit_stop {
                break;
            }

            let _ = token_tx.send(TokenEvent::Token {
                text: piece,
                token_index: n_decode,
            });
        }

        n_decode += 1;

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
        prompt_tokens: prompt_token_count,
    });

    Ok(build_response(
        full_response, prompt_token_count, n_decode, &start, "stop",
    ))
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

    // Collect stop sequences (always include common chat-template markers)
    let stop_sequences = collect_stop_sequences(&request);

    // Build the prompt from chat messages
    let prompt = format_chat_prompt(model, &request.messages);
    tracing::info!("Prompt length: {} chars", prompt.len());
    tracing::debug!("Prompt: {}", &prompt[..prompt.len().min(500)]);

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

            // Check for stop sequences in accumulated response
            let mut hit_stop = false;
            for seq in &stop_sequences {
                if let Some(pos) = full_response.rfind(seq.as_str()) {
                    // Trim response at stop sequence position
                    full_response.truncate(pos);
                    hit_stop = true;
                    tracing::info!("Stop sequence '{}' hit after {n_decode} tokens", seq);
                    // Send Replace event to fix the streamed text on the frontend
                    let _ = token_tx.send(TokenEvent::Replace {
                        full_text: full_response.clone(),
                    });
                    break;
                }
            }
            if hit_stop {
                break;
            }

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

/// Collect stop sequences: merges user-provided ones with common chat template markers.
fn collect_stop_sequences(request: &GenerationRequest) -> Vec<String> {
    let mut seqs: Vec<String> = request.stop_sequences.clone();

    // Always add common chat template end markers
    for marker in &[
        "<|im_end|>",
        "<|end|>",
        "<|eot_id|>",
        "<|endoftext|>",
        "</s>",
        "<|END_OF_TURN_TOKEN|>",
    ] {
        let s = marker.to_string();
        if !seqs.contains(&s) {
            seqs.push(s);
        }
    }

    seqs
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

    // First try the model's built-in chat template (from GGUF metadata)
    if let Ok(template) = model.chat_template(None) {
        let tmpl_preview = template.to_str().unwrap_or("(non-utf8)");
        tracing::info!("Found chat template: {}...", &tmpl_preview[..tmpl_preview.len().min(80)]);
        match model.apply_chat_template(&template, &chat_messages, true) {
            Ok(formatted) => {
                tracing::info!("Applied model's built-in chat template successfully");
                return formatted;
            }
            Err(e) => {
                tracing::warn!("Failed to apply model template: {e}");
            }
        }
    } else {
        tracing::warn!("Model has no built-in chat template, using ChatML fallback");
    }

    // Fallback: Try applying the standard ChatML template via llama.cpp
    // (uses the built-in ChatML template in llama.cpp itself)
    if let Ok(chatml) = llama_cpp_2::model::LlamaChatTemplate::new("chatml") {
        if let Ok(formatted) = model.apply_chat_template(&chatml, &chat_messages, true) {
            tracing::info!("Applied ChatML template via llama.cpp");
            return formatted;
        }
    }

    // Last resort: manual ChatML construction
    tracing::warn!("Manual ChatML fallback (model may not support this format)");
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

                // Try to initialize multimodal context
                #[cfg(feature = "mtmd")]
                let mtmd_ctx: Option<llama_cpp_2::mtmd::MtmdContext> = {
                    tracing::info!("Searching for mmproj companion file...");
                    let mmproj = find_mmproj_file(&model_path);
                    match &mmproj {
                        Some(p) => tracing::info!("Found mmproj: {}", p.display()),
                        None => tracing::warn!("No mmproj file found for {}", model_path.display()),
                    }
                    mmproj.and_then(|mmproj_path| {
                        let mmproj_str = mmproj_path.to_str()?;
                        let gpu_active = cfg!(feature = "cuda") || cfg!(feature = "vulkan");
                        tracing::info!(
                            "Initializing multimodal context (gpu_active={}, gpu_layers={})...",
                            gpu_active, params_clone.n_gpu_layers
                        );
                        let n_threads = params_clone.n_threads.unwrap_or_else(|| {
                            std::cmp::max(1, num_cpus() / 2) as u32
                        }) as i32;

                        // Try GPU first, fall back to CPU if that fails
                        let use_gpu_first = gpu_active && params_clone.n_gpu_layers > 0;
                        let attempts: Vec<bool> = if use_gpu_first {
                            vec![true, false] // try GPU, then CPU
                        } else {
                            vec![false]
                        };

                        let mut init_result = None;
                        for use_gpu in &attempts {
                            tracing::info!("Attempting MtmdContext init with use_gpu={}", use_gpu);
                            let mtmd_params = llama_cpp_2::mtmd::MtmdContextParams {
                                use_gpu: *use_gpu,
                                print_timings: false,
                                n_threads,
                                media_marker: std::ffi::CString::new(
                                    llama_cpp_2::mtmd::mtmd_default_marker()
                                ).unwrap(),
                            };

                            match llama_cpp_2::mtmd::MtmdContext::init_from_file(
                                mmproj_str, &model, &mtmd_params
                            ) {
                                Ok(ctx) => {
                                    tracing::info!("MtmdContext init succeeded (use_gpu={})", use_gpu);
                                    init_result = Some(ctx);
                                    break;
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "MtmdContext init failed with use_gpu={}: {e}", use_gpu
                                    );
                                }
                            }
                        }

                        if let Some(ref ctx) = init_result {
                            tracing::info!(
                                "Multimodal context loaded OK (vision={}, mmproj={})",
                                ctx.support_vision(), mmproj_str
                            );
                        } else {
                            tracing::error!("All MtmdContext init attempts failed for {}", mmproj_str);
                        }
                        init_result
                    })
                };

                let mut info = build_model_info(&model, &model_path, &params_clone);

                #[cfg(feature = "mtmd")]
                {
                    if let Some(ref ctx) = mtmd_ctx {
                        let vision = ctx.support_vision();
                        tracing::info!(
                            "MtmdContext present: support_vision()={}", vision
                        );
                        info.supports_vision = vision;
                        info.mmproj_path = find_mmproj_file(&model_path);
                    } else {
                        tracing::info!("No MtmdContext — model will be text-only");
                    }
                }

                #[cfg(not(feature = "mtmd"))]
                tracing::info!("mtmd feature not compiled — vision unavailable");

                tracing::info!(
                    "Sending model info: name={}, supports_vision={}, mmproj={:?}",
                    info.name, info.supports_vision, info.mmproj_path
                );
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

                            // Check if this request has images and we have multimodal support
                            #[cfg(feature = "mtmd")]
                            let has_images = request.messages.iter().any(|m| !m.images.is_empty());
                            #[cfg(not(feature = "mtmd"))]
                            let has_images = false;

                            // Catch panics from llama.cpp so the thread survives
                            let result = std::panic::catch_unwind(
                                std::panic::AssertUnwindSafe(|| {
                                    #[cfg(feature = "mtmd")]
                                    if has_images {
                                        if let Some(ref ctx) = mtmd_ctx {
                                            return run_generation_multimodal(
                                                &backend,
                                                &model,
                                                ctx,
                                                &params_clone,
                                                request,
                                                &token_tx,
                                                &cancel_flag,
                                            );
                                        }
                                    }
                                    let _ = has_images; // suppress unused warning

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
                            let mut info = build_model_info(&model, &model_path, &params_clone);
                            #[cfg(feature = "mtmd")]
                            if let Some(ref ctx) = mtmd_ctx {
                                info.supports_vision = ctx.support_vision();
                                info.mmproj_path = find_mmproj_file(&model_path);
                            }
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
