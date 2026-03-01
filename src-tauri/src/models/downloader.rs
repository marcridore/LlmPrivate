use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use tauri::ipc::Channel;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

use crate::backend::types::DownloadProgress;
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct ActiveDownload {
    pub repo_id: String,
    pub filename: String,
    pub cancel: Arc<RwLock<bool>>,
}

pub struct DownloadManager {
    client: Client,
    models_dir: PathBuf,
    active_downloads: Arc<RwLock<Vec<ActiveDownload>>>,
}

impl DownloadManager {
    pub fn new(models_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&models_dir).ok();
        Self {
            client: Client::new(),
            models_dir,
            active_downloads: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn download_model(
        &self,
        repo_id: &str,
        filename: &str,
        on_progress: Channel<DownloadProgress>,
    ) -> Result<String, AppError> {
        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            repo_id, filename
        );

        let dest_path = self.models_dir.join(filename);

        // Check if already exists
        if dest_path.exists() {
            return Ok(dest_path.to_string_lossy().to_string());
        }

        // Track active download
        let cancel_flag = Arc::new(RwLock::new(false));
        let active = ActiveDownload {
            repo_id: repo_id.to_string(),
            filename: filename.to_string(),
            cancel: cancel_flag.clone(),
        };
        self.active_downloads.write().await.push(active);

        let result = self
            .do_download(&url, &dest_path, cancel_flag.clone(), on_progress)
            .await;

        // Remove from active downloads
        let mut downloads = self.active_downloads.write().await;
        downloads.retain(|d| d.filename != filename);

        match result {
            Ok(()) => Ok(dest_path.to_string_lossy().to_string()),
            Err(e) => {
                // Clean up partial file on error
                tokio::fs::remove_file(&dest_path).await.ok();
                Err(e)
            }
        }
    }

    async fn do_download(
        &self,
        url: &str,
        dest: &PathBuf,
        cancel: Arc<RwLock<bool>>,
        on_progress: Channel<DownloadProgress>,
    ) -> Result<(), AppError> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", "LlmPrivate/0.1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AppError::Download(format!(
                "HTTP {} from {}",
                response.status(),
                url
            )));
        }

        let total_bytes = response.content_length().unwrap_or(0);

        // Use a temp file during download
        let temp_path = dest.with_extension("gguf.part");
        let mut file = tokio::fs::File::create(&temp_path).await?;

        let mut downloaded: u64 = 0;
        let start = Instant::now();
        let mut last_progress = Instant::now();
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            // Check cancellation
            if *cancel.read().await {
                file.flush().await.ok();
                drop(file);
                tokio::fs::remove_file(&temp_path).await.ok();
                return Err(AppError::Download("Download cancelled".into()));
            }

            let chunk = chunk.map_err(|e| AppError::Download(e.to_string()))?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // Send progress every 100ms
            if last_progress.elapsed().as_millis() >= 100 {
                let elapsed = start.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 {
                    (downloaded as f64 / elapsed) as u64
                } else {
                    0
                };
                let percent = if total_bytes > 0 {
                    (downloaded as f64 / total_bytes as f64) * 100.0
                } else {
                    0.0
                };

                let _ = on_progress.send(DownloadProgress {
                    downloaded_bytes: downloaded,
                    total_bytes,
                    percent,
                    speed_bytes_per_sec: speed,
                });
                last_progress = Instant::now();
            }
        }

        file.flush().await?;
        drop(file);

        // Rename temp file to final path
        tokio::fs::rename(&temp_path, dest).await?;

        // Send final progress
        let _ = on_progress.send(DownloadProgress {
            downloaded_bytes: total_bytes,
            total_bytes,
            percent: 100.0,
            speed_bytes_per_sec: 0,
        });

        Ok(())
    }

    pub async fn cancel_download(&self, filename: &str) {
        let downloads = self.active_downloads.read().await;
        if let Some(d) = downloads.iter().find(|d| d.filename == filename) {
            *d.cancel.write().await = true;
        }
    }

    pub async fn active_downloads(&self) -> Vec<(String, String)> {
        self.active_downloads
            .read()
            .await
            .iter()
            .map(|d| (d.repo_id.clone(), d.filename.clone()))
            .collect()
    }
}
