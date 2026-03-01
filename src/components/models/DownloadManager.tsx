import { useModelStore } from "../../stores/modelStore";

function formatBytes(bytes: number): string {
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(0)} MB`;
  return `${bytes} B`;
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec >= 1_000_000)
    return `${(bytesPerSec / 1_000_000).toFixed(1)} MB/s`;
  if (bytesPerSec >= 1_000)
    return `${(bytesPerSec / 1_000).toFixed(0)} KB/s`;
  return `${bytesPerSec} B/s`;
}

export function DownloadManager() {
  const activeDownloads = useModelStore((s) => s.activeDownloads);
  const cancelDownload = useModelStore((s) => s.cancelDownload);

  const downloads = Array.from(activeDownloads.values());

  if (downloads.length === 0) {
    return (
      <div className="text-center py-12 text-muted-foreground">
        <p className="text-lg mb-2">No active downloads</p>
        <p className="text-sm">
          Go to the Discover tab to download models.
        </p>
      </div>
    );
  }

  return (
    <div className="max-w-2xl space-y-4">
      <h2 className="text-sm font-medium text-muted-foreground">
        Active Downloads ({downloads.length})
      </h2>

      {downloads.map((dl) => {
        const { progress } = dl;
        const eta =
          progress.speed_bytes_per_sec > 0
            ? Math.ceil(
                (progress.total_bytes - progress.downloaded_bytes) /
                  progress.speed_bytes_per_sec
              )
            : 0;

        const etaStr =
          eta > 3600
            ? `${Math.floor(eta / 3600)}h ${Math.floor((eta % 3600) / 60)}m`
            : eta > 60
              ? `${Math.floor(eta / 60)}m ${eta % 60}s`
              : `${eta}s`;

        return (
          <div
            key={dl.modelId}
            className="border border-border rounded-lg p-4"
          >
            <div className="flex items-center justify-between mb-2">
              <h3 className="font-medium text-sm">{dl.filename}</h3>
              <button
                onClick={() => cancelDownload(dl.filename)}
                className="px-3 py-1 text-xs text-muted-foreground hover:text-destructive hover:bg-destructive/10 rounded-md transition-colors"
              >
                Cancel
              </button>
            </div>

            {/* Progress bar */}
            <div className="w-full h-2 bg-muted rounded-full overflow-hidden mb-2">
              <div
                className="h-full bg-primary rounded-full transition-all duration-300"
                style={{ width: `${progress.percent}%` }}
              />
            </div>

            {/* Stats */}
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>
                {formatBytes(progress.downloaded_bytes)} /{" "}
                {formatBytes(progress.total_bytes)} (
                {progress.percent.toFixed(1)}%)
              </span>
              <span className="flex gap-3">
                {progress.speed_bytes_per_sec > 0 && (
                  <span>{formatSpeed(progress.speed_bytes_per_sec)}</span>
                )}
                {eta > 0 && <span>ETA: {etaStr}</span>}
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}
