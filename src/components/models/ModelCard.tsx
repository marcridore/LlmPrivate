import { useModelStore } from "../../stores/modelStore";
import type { RecommendedModel } from "../../types/model";

interface ModelCardProps {
  model: RecommendedModel;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(0)} MB`;
  return `${bytes} B`;
}

const categoryColors: Record<string, string> = {
  small: "bg-green-500/20 text-green-400",
  general: "bg-blue-500/20 text-blue-400",
  code: "bg-purple-500/20 text-purple-400",
  large: "bg-orange-500/20 text-orange-400",
  chat: "bg-cyan-500/20 text-cyan-400",
  vision: "bg-pink-500/20 text-pink-400",
};

const quantColors: Record<string, string> = {
  Q2_K: "text-red-400",
  Q3_K_M: "text-orange-400",
  Q4_K_M: "text-yellow-400",
  Q5_K_M: "text-green-400",
  Q8_0: "text-blue-400",
  F16: "text-purple-400",
};

export function ModelCard({ model }: ModelCardProps) {
  const localModels = useModelStore((s) => s.localModels);
  const activeDownloads = useModelStore((s) => s.activeDownloads);
  const downloadErrors = useModelStore((s) => s.downloadErrors);
  const downloadModel = useModelStore((s) => s.downloadModel);

  const isDownloaded = localModels.some(
    (m) => m.name === model.filename.replace(".gguf", "")
  );
  const isDownloading = activeDownloads.has(model.id);
  const download = activeDownloads.get(model.id);
  const error = downloadErrors.get(model.id);

  const handleDownload = () => {
    if (!isDownloaded && !isDownloading) {
      downloadModel(model);
    }
  };

  return (
    <div className="border border-border rounded-lg p-4 hover:border-muted-foreground/50 transition-colors">
      {/* Header */}
      <div className="flex items-start justify-between mb-2">
        <div>
          <h3 className="font-semibold text-sm">{model.name}</h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            {model.description}
          </p>
        </div>
      </div>

      {/* Badges */}
      <div className="flex flex-wrap gap-1.5 mb-3">
        <span
          className={`px-2 py-0.5 rounded text-xs font-medium ${
            categoryColors[model.category] || "bg-muted text-muted-foreground"
          }`}
        >
          {model.category}
        </span>
        <span className="px-2 py-0.5 rounded text-xs bg-muted text-muted-foreground">
          {model.param_count}
        </span>
        <span
          className={`px-2 py-0.5 rounded text-xs font-mono bg-muted ${
            quantColors[model.quantization] || "text-muted-foreground"
          }`}
        >
          {model.quantization}
        </span>
        <span className="px-2 py-0.5 rounded text-xs bg-muted text-muted-foreground">
          {formatBytes(model.size_bytes)}
        </span>
      </div>

      {/* RAM requirement */}
      <p className="text-xs text-muted-foreground mb-3">
        Requires ~{model.min_ram_gb}GB RAM
      </p>

      {/* Download progress bar */}
      {isDownloading && download && (
        <div className="mb-3">
          <div className="flex justify-between text-xs text-muted-foreground mb-1">
            <span>{download.progress.percent.toFixed(1)}%</span>
            <span>
              {formatBytes(download.progress.downloaded_bytes)} /{" "}
              {formatBytes(download.progress.total_bytes)}
            </span>
          </div>
          <div className="w-full h-2 bg-muted rounded-full overflow-hidden">
            <div
              className="h-full bg-primary rounded-full transition-all duration-300"
              style={{ width: `${download.progress.percent}%` }}
            />
          </div>
          {download.progress.speed_bytes_per_sec > 0 && (
            <p className="text-xs text-muted-foreground mt-1">
              {formatBytes(download.progress.speed_bytes_per_sec)}/s
            </p>
          )}
        </div>
      )}

      {/* Error */}
      {error && (
        <p className="text-xs text-destructive mb-2">
          Download failed: {error}
        </p>
      )}

      {/* Action button */}
      <button
        onClick={handleDownload}
        disabled={isDownloaded || isDownloading}
        className={`w-full py-2 rounded-md text-sm font-medium transition-colors ${
          isDownloaded
            ? "bg-green-500/20 text-green-400 cursor-default"
            : isDownloading
              ? "bg-muted text-muted-foreground cursor-wait"
              : "bg-primary text-primary-foreground hover:opacity-90"
        }`}
      >
        {isDownloaded
          ? "Downloaded"
          : isDownloading
            ? "Downloading..."
            : "Download"}
      </button>
    </div>
  );
}
