use std::collections::HashMap;
use std::sync::Arc;

use crate::backend::traits::InferenceBackend;

pub mod llama_cpp_backend;
pub mod stub_backend;
pub mod traits;
pub mod types;
pub mod vision_server;
pub mod openclaw_server;

pub struct BackendRegistry {
    backends: HashMap<String, Arc<dyn InferenceBackend>>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
        }
    }

    pub fn register(&mut self, backend: Arc<dyn InferenceBackend>) {
        self.backends.insert(backend.name().to_string(), backend);
    }

    pub fn backend_for_format(&self, format: &str) -> Option<Arc<dyn InferenceBackend>> {
        for backend in self.backends.values() {
            if backend.supported_formats().contains(&format) {
                return Some(Arc::clone(backend));
            }
        }
        None
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn InferenceBackend>> {
        self.backends.get(name).cloned()
    }

    pub fn list(&self) -> Vec<String> {
        self.backends.keys().cloned().collect()
    }

    pub fn default_backend(&self) -> Option<Arc<dyn InferenceBackend>> {
        self.backends.values().next().cloned()
    }
}
