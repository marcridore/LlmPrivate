//! Vision server: manages a llama-server sidecar process for multimodal inference.
//!
//! The bundled llama-cpp-2 Rust crate has mmproj compatibility issues with newer
//! vision model files. Instead of fighting that, we spawn the latest llama-server
//! binary (pre-built with CUDA) which handles vision models perfectly.
//!
//! Architecture:
//! - Text-only inference: handled by the in-process llama-cpp-2 backend (fast)
//! - Vision inference: routed to llama-server via its OpenAI-compatible HTTP API

use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

use crate::error::AppError;
use crate::backend::types::*;

/// Port range for the vision server (avoids conflicts)
const VISION_SERVER_PORT_START: u16 = 8990;

/// Manages the llama-server child process for vision inference.
pub struct VisionServer {
    inner: RwLock<Option<VisionServerInner>>,
}

struct VisionServerInner {
    child: tokio::process::Child,
    port: u16,
    model_path: PathBuf,
    mmproj_path: PathBuf,
}

impl VisionServer {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(None),
        }
    }

    /// Find the llama-server binary in the app data directory.
    fn find_server_binary() -> Option<PathBuf> {
        if let Some(app_data) = dirs_next_appdata() {
            let server_path = app_data
                .join("bin")
                .join("llama-server")
                .join("llama-server.exe");
            if server_path.exists() {
                return Some(server_path);
            }
        }
        None
    }

    /// Start the vision server for a given model + mmproj pair.
    pub async fn start(
        &self,
        model_path: &Path,
        mmproj_path: &Path,
        n_gpu_layers: u32,
    ) -> Result<u16, AppError> {
        // Stop any existing server first
        self.stop().await;

        let server_bin = Self::find_server_binary().ok_or_else(|| {
            AppError::Vision(
                "llama-server binary not found. Please download it to the app's bin directory."
                    .to_string(),
            )
        })?;

        let port = VISION_SERVER_PORT_START;

        tracing::info!(
            "Starting vision server: {} --model {} --mmproj {} --port {}",
            server_bin.display(),
            model_path.display(),
            mmproj_path.display(),
            port
        );

        let child = tokio::process::Command::new(&server_bin)
            .arg("--model")
            .arg(model_path)
            .arg("--mmproj")
            .arg(mmproj_path)
            .arg("--port")
            .arg(port.to_string())
            .arg("--n-gpu-layers")
            .arg(n_gpu_layers.to_string())
            .arg("--ctx-size")
            .arg("4096")
            // Suppress terminal output (logs go to stderr)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Vision(format!("Failed to spawn llama-server: {e}")))?;

        tracing::info!("Vision server spawned (PID: {:?}), waiting for health...", child.id());

        // Wait for the server to become healthy (model loading takes time)
        let client = reqwest::Client::new();
        let health_url = format!("http://127.0.0.1:{}/health", port);
        let mut healthy = false;

        for attempt in 1..=120 {
            // up to 2 minutes
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            match client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!("Vision server healthy after {}s", attempt);
                    healthy = true;
                    break;
                }
                Ok(resp) => {
                    tracing::debug!(
                        "Vision server not ready (attempt {}, status={})",
                        attempt,
                        resp.status()
                    );
                }
                Err(_) => {
                    if attempt % 10 == 0 {
                        tracing::debug!("Vision server not ready (attempt {})", attempt);
                    }
                }
            }
        }

        if !healthy {
            tracing::error!("Vision server failed to become healthy within 120s");
            return Err(AppError::Vision(
                "Vision server failed to start within 2 minutes".to_string(),
            ));
        }

        // Warmup: send a tiny probe image to force mmproj initialization.
        // The health endpoint returns OK before multimodal is fully ready,
        // causing the first few real requests to get garbage responses.
        tracing::info!("Warming up vision server with probe image...");
        let warmup_url = format!("http://127.0.0.1:{}/completion", port);
        // Minimal 1x1 red PNG (68 bytes base64)
        let probe_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADklEQVQI12P4z8BQDwAEgAF/QualIQAAAABJRU5ErkJggg==";
        let warmup_body = serde_json::json!({
            "prompt": {
                "prompt_string": "<|im_start|>User:<__media__>color?<end_of_utterance>\nAssistant:",
                "multimodal_data": [probe_b64],
            },
            "n_predict": 1,
            "temperature": 0.1,
            "stream": false,
        });
        match client
            .post(&warmup_url)
            .json(&warmup_body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Vision server warmup complete");
            }
            Ok(resp) => {
                tracing::warn!("Vision warmup got status {}, continuing anyway", resp.status());
            }
            Err(e) => {
                tracing::warn!("Vision warmup failed: {e}, continuing anyway");
            }
        }

        *self.inner.write().await = Some(VisionServerInner {
            child,
            port,
            model_path: model_path.to_path_buf(),
            mmproj_path: mmproj_path.to_path_buf(),
        });

        Ok(port)
    }

    /// Stop the vision server if running.
    pub async fn stop(&self) {
        let mut lock = self.inner.write().await;
        if let Some(mut inner) = lock.take() {
            tracing::info!("Stopping vision server...");
            let _ = inner.child.kill().await;
            let _ = inner.child.wait().await;
            tracing::info!("Vision server stopped");
        }
    }

    /// Check if the vision server is running.
    pub async fn is_running(&self) -> bool {
        self.inner.read().await.is_some()
    }

    /// Get the port the vision server is listening on.
    pub async fn port(&self) -> Option<u16> {
        self.inner.read().await.as_ref().map(|i| i.port)
    }

    /// Send a vision request to the llama-server using the /completion endpoint.
    /// Uses raw prompt with <image> tokens and image_data array (the /v1/chat/completions
    /// endpoint fails to tokenize multimodal prompts for SmolVLM2).
    pub async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        max_tokens: u32,
        temperature: f32,
        token_tx: &tokio::sync::mpsc::UnboundedSender<TokenEvent>,
    ) -> Result<GenerationResponse, AppError> {
        let start = std::time::Instant::now();

        let port = self.port().await.ok_or_else(|| {
            AppError::Vision("Vision server is not running".to_string())
        })?;

        let url = format!("http://127.0.0.1:{}/completion", port);

        // Build prompt with <__media__> markers and collect base64 image data
        let (prompt_string, multimodal_data) = build_multimodal_prompt(messages)?;

        let body = serde_json::json!({
            "prompt": {
                "prompt_string": prompt_string,
                "multimodal_data": multimodal_data,
            },
            "n_predict": max_tokens,
            "temperature": temperature,
            "stream": false,
        });

        tracing::info!(
            "Sending vision request to llama-server ({} images, prompt len={})",
            multimodal_data.len(),
            prompt_string.len()
        );

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(300))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Vision server HTTP request failed: {e}");
                AppError::Vision(format!("Vision server request failed: {e}"))
            })?;

        tracing::info!("Vision server responded with status: {}", resp.status());

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            tracing::error!("Vision server error response: {} - {}", status, body_text);
            return Err(AppError::Vision(format!(
                "Vision server returned {}: {}",
                status, body_text
            )));
        }

        let api_resp: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| {
                tracing::error!("Failed to parse vision response JSON: {e}");
                AppError::Vision(format!("Failed to parse vision response: {e}"))
            })?;

        // Extract response from /completion endpoint format
        let content = api_resp["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let prompt_tokens = api_resp["timings"]["prompt_n"].as_u64().unwrap_or(0) as u32;
        let completion_tokens = api_resp["tokens_predicted"]
            .as_u64()
            .unwrap_or(0) as u32;

        let elapsed_ms = start.elapsed().as_millis() as u64;
        let tps = if elapsed_ms > 0 {
            completion_tokens as f64 / (elapsed_ms as f64 / 1000.0)
        } else {
            0.0
        };

        // Send the full response as tokens to the frontend
        let _ = token_tx.send(TokenEvent::Token {
            text: content.clone(),
            token_index: 0,
        });
        let _ = token_tx.send(TokenEvent::Done {
            total_tokens: completion_tokens,
            generation_time_ms: elapsed_ms,
            tokens_per_second: tps,
            prompt_tokens,
        });

        tracing::info!(
            "Vision response: {} tokens in {}ms ({:.1} t/s)",
            completion_tokens,
            elapsed_ms,
            tps
        );

        Ok(GenerationResponse {
            content,
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            generation_time_ms: elapsed_ms,
            tokens_per_second: tps,
            stop_reason: "stop".to_string(),
        })
    }
}

impl Drop for VisionServer {
    fn drop(&mut self) {
        // Best-effort synchronous cleanup — kill the child process
        let inner_opt = self.inner.get_mut();
        if let Some(inner) = inner_opt.take() {
            if let Some(id) = inner.child.id() {
                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/PID", &id.to_string()])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                }
                #[cfg(not(windows))]
                {
                    let _ = std::process::Command::new("kill")
                        .arg("-9")
                        .arg(id.to_string())
                        .status();
                }
            }
        }
    }
}

/// Build a prompt string with `<__media__>` markers and a parallel array of raw base64 strings.
/// Uses the SmolVLM2 chat template format with the mtmd media marker.
/// The llama-server replaces each `<__media__>` with actual image embeddings.
fn build_multimodal_prompt(
    messages: &[ChatMessage],
) -> Result<(String, Vec<String>), AppError> {
    let mut prompt = String::from("<|im_start|>");
    let mut multimodal_data: Vec<String> = Vec::new();

    for msg in messages {
        let role = match msg.role {
            Role::System => "System",
            Role::User => "User",
            Role::Assistant => "Assistant",
        };

        if msg.images.is_empty() {
            // Text-only message
            prompt.push_str(&format!("{}: {}<end_of_utterance>\n", role, msg.content));
        } else {
            // Message with images: images come before text, no space after colon
            prompt.push_str(&format!("{}:", role));

            for img in &msg.images {
                if img.file_path.is_empty() {
                    tracing::warn!("Skipping image with empty file path");
                    continue;
                }
                tracing::info!(
                    "Reading image: {} (path exists: {})",
                    img.file_path,
                    std::path::Path::new(&img.file_path).exists()
                );
                let data = std::fs::read(&img.file_path).map_err(|e| {
                    tracing::error!("Failed to read image file '{}': {e}", img.file_path);
                    AppError::Vision(format!("Failed to read image '{}': {e}", img.file_path))
                })?;
                tracing::info!("Image read OK: {} bytes, encoding to base64", data.len());

                let b64 = base64_encode(&data);
                multimodal_data.push(b64);

                // Use the mtmd default marker that llama-server recognizes
                prompt.push_str("<__media__>");
            }

            prompt.push_str(&format!("{}<end_of_utterance>\n", msg.content));
        }
    }

    // Add generation prompt
    prompt.push_str("Assistant:");

    Ok((prompt, multimodal_data))
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn mime_from_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else {
        "image/png" // default
    }
}

/// Get the app data directory (platform-specific).
fn dirs_next_appdata() -> Option<PathBuf> {
    // On Windows: %APPDATA%/com.llmprivate.app
    #[cfg(windows)]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("com.llmprivate.app"))
    }
    #[cfg(not(windows))]
    {
        dirs_next::data_dir().map(|p| p.join("com.llmprivate.app"))
    }
}
