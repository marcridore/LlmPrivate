use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::backend::types::*;
use crate::backend::BackendRegistry;
use crate::error::AppError;

pub struct ModelManager {
    backends: Arc<RwLock<BackendRegistry>>,
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(backends: Arc<RwLock<BackendRegistry>>, models_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&models_dir).ok();
        Self {
            backends,
            models_dir,
        }
    }

    pub async fn load_model(
        &self,
        path: PathBuf,
        params: ModelLoadParams,
    ) -> Result<ModelHandle, AppError> {
        let backends = self.backends.read().await;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("gguf");

        let backend = backends
            .backend_for_format(ext)
            .or_else(|| backends.default_backend())
            .ok_or(AppError::NoBackend)?;

        backend.load_model(path, params).await
    }

    pub async fn unload_model(&self, handle: ModelHandle) -> Result<(), AppError> {
        let backends = self.backends.read().await;
        let backend = backends.default_backend().ok_or(AppError::NoBackend)?;
        backend.unload_model(handle).await
    }

    pub fn get_model_info(&self, handle: ModelHandle) -> Result<ModelInfo, AppError> {
        // This is sync -- we need try_read
        // For MVP, just return an error if we can't get the lock
        Err(AppError::NotFound(format!(
            "Model info lookup for handle {} requires async context",
            handle
        )))
    }

    pub async fn scan_local_models(&self) -> Result<Vec<LocalModelEntry>, AppError> {
        let mut models = vec![];

        if !self.models_dir.exists() {
            return Ok(models);
        }

        // Collect all gguf filenames to detect mmproj companions
        let all_files: Vec<_> = std::fs::read_dir(&self.models_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().and_then(|ext| ext.to_str()) == Some("gguf")
            })
            .collect();

        let mmproj_names: Vec<String> = all_files
            .iter()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.contains("mmproj") {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();

        for entry in &all_files {
            let path = entry.path();
            let filename = entry.file_name().to_string_lossy().to_lowercase();

            // Skip mmproj files — they are not standalone models
            if filename.contains("mmproj") {
                continue;
            }

            let metadata = std::fs::metadata(&path)?;
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Check if there's a matching mmproj file in the same directory
            let has_mmproj = !mmproj_names.is_empty()
                && crate::backend::llama_cpp_backend::find_mmproj_file(&path).is_some();

            models.push(LocalModelEntry {
                name,
                file_path: path,
                file_size_bytes: metadata.len(),
                quantization: "unknown".to_string(),
                is_loaded: false,
                handle: None,
                has_mmproj,
            });
        }

        Ok(models)
    }

    pub async fn delete_model(&self, path: PathBuf) -> Result<(), AppError> {
        std::fs::remove_file(&path)?;
        Ok(())
    }

    pub fn models_dir(&self) -> &PathBuf {
        &self.models_dir
    }
}
