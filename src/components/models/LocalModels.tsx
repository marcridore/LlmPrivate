import { useEffect } from "react";
import { useModelStore } from "../../stores/modelStore";
import { useChatStore } from "../../stores/chatStore";
import { useUIStore } from "../../stores/uiStore";

function formatBytes(bytes: number): string {
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(0)} MB`;
  return `${bytes} B`;
}

export function LocalModels() {
  const localModels = useModelStore((s) => s.localModels);
  const scanLocalModels = useModelStore((s) => s.scanLocalModels);
  const loadModel = useModelStore((s) => s.loadModel);
  const unloadModel = useModelStore((s) => s.unloadModel);
  const deleteModel = useModelStore((s) => s.deleteModel);
  const loadedModelHandle = useModelStore((s) => s.loadedModelHandle);
  const loadedModelName = useModelStore((s) => s.loadedModelName);
  const isLoading = useModelStore((s) => s.isLoading);
  const loadingModelPath = useModelStore((s) => s.loadingModelPath);
  const loadError = useModelStore((s) => s.loadError);
  const clearLoadError = useModelStore((s) => s.clearLoadError);
  const setLoadedModelHandle = useChatStore((s) => s.setLoadedModelHandle);
  const setActivePage = useUIStore((s) => s.setActivePage);

  useEffect(() => {
    scanLocalModels();
  }, [scanLocalModels]);

  const handleLoad = async (path: string) => {
    const handle = await loadModel(path);
    if (handle) {
      setLoadedModelHandle(handle);
    }
  };

  const handleUnload = async () => {
    if (loadedModelHandle) {
      await unloadModel(loadedModelHandle);
      setLoadedModelHandle(null);
    }
  };

  const handleLoadAndChat = async (path: string) => {
    const handle = await loadModel(path);
    if (handle) {
      setLoadedModelHandle(handle);
      setActivePage("chat");
    }
  };

  if (localModels.length === 0) {
    return (
      <div className="text-center py-12 text-muted-foreground">
        <p className="text-lg mb-2">No models installed</p>
        <p className="text-sm">
          Download models from the Discover tab to get started.
        </p>
      </div>
    );
  }

  return (
    <div className="max-w-2xl space-y-3">
      {/* Error display */}
      {loadError && (
        <div className="p-3 border border-red-500/30 rounded-lg bg-red-500/10 mb-4">
          <div className="flex items-start justify-between gap-2">
            <div>
              <p className="text-sm font-medium text-red-400">Model load failed</p>
              <p className="text-xs text-red-400/80 mt-1 font-mono break-all">
                {loadError}
              </p>
            </div>
            <button
              onClick={clearLoadError}
              className="text-red-400 hover:text-red-300 text-xs shrink-0"
            >
              Dismiss
            </button>
          </div>
        </div>
      )}

      {/* Currently loaded model */}
      {loadedModelName && (
        <div className="p-3 border border-green-500/30 rounded-lg bg-green-500/5 mb-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-green-500" />
              <span className="text-sm font-medium">
                Active: {loadedModelName}
              </span>
            </div>
            <button
              onClick={handleUnload}
              className="px-3 py-1 text-xs bg-muted rounded-md hover:bg-destructive hover:text-destructive-foreground transition-colors"
            >
              Unload
            </button>
          </div>
        </div>
      )}

      {/* Model list */}
      {localModels.map((model) => {
        const isActive = loadedModelName === model.name;
        const isThisLoading = loadingModelPath === model.file_path;
        return (
          <div
            key={model.file_path}
            className={`p-4 border rounded-lg transition-colors ${
              isThisLoading
                ? "border-yellow-500/30 bg-yellow-500/5"
                : isActive
                  ? "border-green-500/30 bg-green-500/5"
                  : "border-border hover:border-muted-foreground/50"
            }`}
          >
            <div className="flex items-center justify-between">
              <div>
                <h3 className="font-medium text-sm">{model.name}</h3>
                <div className="flex gap-2 mt-1 text-xs text-muted-foreground items-center">
                  <span>{formatBytes(model.file_size_bytes)}</span>
                  {model.quantization !== "unknown" && (
                    <span className="font-mono">{model.quantization}</span>
                  )}
                  {model.has_mmproj && (
                    <span className="px-1.5 py-0.5 rounded bg-pink-500/20 text-pink-400 font-medium">
                      Vision
                    </span>
                  )}
                  {isThisLoading && (
                    <span className="text-yellow-400 animate-pulse">Loading model...</span>
                  )}
                </div>
              </div>

              <div className="flex gap-2">
                {isActive ? (
                  <button
                    onClick={() => setActivePage("chat")}
                    className="px-3 py-1.5 text-xs bg-green-500/20 text-green-400 rounded-md hover:bg-green-500/30 transition-colors"
                  >
                    Chat
                  </button>
                ) : (
                  <button
                    onClick={() => handleLoadAndChat(model.file_path)}
                    disabled={isLoading}
                    className="px-3 py-1.5 text-xs bg-primary text-primary-foreground rounded-md hover:opacity-90 transition-opacity disabled:opacity-50"
                  >
                    {isThisLoading ? "Loading..." : "Load & Chat"}
                  </button>
                )}
                {!isActive && !isThisLoading && (
                  <button
                    onClick={() => handleLoad(model.file_path)}
                    disabled={isLoading}
                    className="px-3 py-1.5 text-xs bg-muted rounded-md hover:bg-muted/80 transition-colors disabled:opacity-50"
                  >
                    Load
                  </button>
                )}
                <button
                  onClick={() => {
                    if (confirm(`Delete ${model.name}?`)) {
                      deleteModel(model.file_path);
                    }
                  }}
                  disabled={isActive || isThisLoading}
                  className="px-3 py-1.5 text-xs text-muted-foreground hover:text-destructive hover:bg-destructive/10 rounded-md transition-colors disabled:opacity-30"
                >
                  Delete
                </button>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
