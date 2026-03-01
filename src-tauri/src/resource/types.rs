use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub cpu_usage_percent: f32,
    pub cpu_cores: u32,
    pub ram_total_bytes: u64,
    pub ram_used_bytes: u64,
    pub ram_available_bytes: u64,
    pub gpus: Vec<GpuSnapshot>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSnapshot {
    pub name: String,
    pub vram_total_bytes: u64,
    pub vram_used_bytes: u64,
    pub vram_free_bytes: u64,
    pub utilization_percent: Option<f32>,
    pub temperature_celsius: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub index: u32,
    pub name: String,
    pub vendor: String,
    pub vram_total_bytes: u64,
    pub driver_version: String,
    pub compute_capability: Option<String>,
    pub supported_backends: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLoadRecommendation {
    pub can_load: bool,
    pub recommended_n_gpu_layers: u32,
    pub recommended_n_ctx: u32,
    pub estimated_ram_usage_bytes: u64,
    pub estimated_vram_usage_bytes: u64,
    pub warnings: Vec<String>,
}
