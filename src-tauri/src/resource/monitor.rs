use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sysinfo::System;

use crate::error::AppError;
use crate::resource::types::*;

pub struct SystemResourceMonitor {
    system: Mutex<System>,
}

impl SystemResourceMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            system: Mutex::new(sys),
        }
    }

    pub fn snapshot(&self) -> Result<ResourceSnapshot, AppError> {
        let mut sys = self.system.lock().map_err(|_| AppError::LockContention)?;
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let cpu_usage = sys.global_cpu_usage();
        let cpu_cores = sys.cpus().len() as u32;
        let ram_total = sys.total_memory();
        let ram_used = sys.used_memory();
        let ram_available = ram_total.saturating_sub(ram_used);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(ResourceSnapshot {
            cpu_usage_percent: cpu_usage,
            cpu_cores,
            ram_total_bytes: ram_total,
            ram_used_bytes: ram_used,
            ram_available_bytes: ram_available,
            gpus: vec![], // GPU detection added in Phase 6
            timestamp_ms: timestamp,
        })
    }

    pub fn gpu_info(&self) -> Result<Vec<GpuInfo>, AppError> {
        // GPU detection will be implemented in Phase 6 with nvml-wrapper
        Ok(vec![])
    }

    pub fn recommend_params(
        &self,
        model_size_bytes: u64,
        _model_quant: &str,
    ) -> Result<ModelLoadRecommendation, AppError> {
        let sys = self.system.lock().map_err(|_| AppError::LockContention)?;
        let ram_available = sys.total_memory().saturating_sub(sys.used_memory());

        let can_load = ram_available > model_size_bytes;
        let mut warnings = vec![];

        if !can_load {
            warnings.push(format!(
                "Model requires ~{:.1}GB but only {:.1}GB RAM available",
                model_size_bytes as f64 / 1_073_741_824.0,
                ram_available as f64 / 1_073_741_824.0
            ));
        }

        Ok(ModelLoadRecommendation {
            can_load,
            recommended_n_gpu_layers: 0,
            recommended_n_ctx: 4096,
            estimated_ram_usage_bytes: model_size_bytes,
            estimated_vram_usage_bytes: 0,
            warnings,
        })
    }
}
