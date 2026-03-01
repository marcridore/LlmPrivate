export interface ModelInfo {
  name: string;
  file_path: string;
  file_size_bytes: number;
  architecture: string;
  parameter_count: number | null;
  quantization: string;
  context_length: number;
  embedding_length: number | null;
  vocab_size: number | null;
  backend: string;
}

export interface LocalModelEntry {
  name: string;
  file_path: string;
  file_size_bytes: number;
  quantization: string;
  is_loaded: boolean;
  handle: number | null;
  has_mmproj: boolean;
}

export interface DownloadProgress {
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
  speed_bytes_per_sec: number;
}

export interface RecommendedModel {
  id: string;
  name: string;
  description: string;
  repo_id: string;
  filename: string;
  size_bytes: number;
  param_count: string;
  quantization: string;
  min_ram_gb: number;
  category: "general" | "code" | "chat" | "small" | "large" | "vision";
  tags: string[];
  mmproj_filename?: string | null;
  mmproj_repo_id?: string | null;
}
