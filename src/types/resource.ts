export interface ResourceSnapshot {
  cpu_usage_percent: number;
  cpu_cores: number;
  ram_total_bytes: number;
  ram_used_bytes: number;
  ram_available_bytes: number;
  gpus: GpuSnapshot[];
  timestamp_ms: number;
}

export interface GpuSnapshot {
  name: string;
  vram_total_bytes: number;
  vram_used_bytes: number;
  vram_free_bytes: number;
  utilization_percent: number | null;
  temperature_celsius: number | null;
}

export interface GpuInfo {
  index: number;
  name: string;
  vendor: string;
  vram_total_bytes: number;
  driver_version: string;
  compute_capability: string | null;
  supported_backends: string[];
}
