import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useModelStore } from "../../stores/modelStore";
import { useChatStore } from "../../stores/chatStore";

export function ModelLoader() {
  const loadModel = useModelStore((s) => s.loadModel);
  const isLoading = useModelStore((s) => s.isLoading);
  const loadingModelPath = useModelStore((s) => s.loadingModelPath);
  const autoLoadAttempted = useModelStore((s) => s.autoLoadAttempted);
  const setLoadedModelHandle = useChatStore((s) => s.setLoadedModelHandle);
  const [error, setError] = useState<string | null>(null);

  const handlePickModel = async () => {
    setError(null);
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "GGUF Model",
            extensions: ["gguf"],
          },
        ],
      });

      if (selected) {
        const handle = await loadModel(selected as string);
        if (handle) {
          setLoadedModelHandle(handle);
        } else {
          setError("Failed to load model. Check console for details.");
        }
      }
    } catch (e) {
      setError(String(e));
    }
  };

  // Show auto-loading state
  if (isLoading && !autoLoadAttempted) {
    const modelName = loadingModelPath?.split(/[/\\]/).pop() ?? "model";
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <span className="animate-spin inline-block w-8 h-8 border-3 border-primary border-t-transparent rounded-full mb-4" />
          <p className="text-muted-foreground text-sm">Loading {modelName}...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex items-center justify-center">
      <div className="text-center max-w-md px-8">
        <div className="mb-6">
          <div className="w-16 h-16 mx-auto mb-4 rounded-2xl bg-primary/10 flex items-center justify-center">
            <svg
              width="32"
              height="32"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="text-primary"
            >
              <path d="M12 2L2 7l10 5 10-5-10-5z" />
              <path d="M2 17l10 5 10-5" />
              <path d="M2 12l10 5 10-5" />
            </svg>
          </div>
          <h2 className="text-2xl font-bold mb-2">Welcome to LlmPrivate</h2>
          <p className="text-muted-foreground text-sm mb-6">
            Load a GGUF model to start chatting with AI locally. Your data never
            leaves your machine.
          </p>
        </div>

        <button
          onClick={handlePickModel}
          disabled={isLoading}
          className="w-full py-3 px-6 bg-primary text-primary-foreground rounded-lg font-medium hover:opacity-90 transition-opacity disabled:opacity-50"
        >
          {isLoading ? (
            <span className="flex items-center justify-center gap-2">
              <span className="animate-spin w-4 h-4 border-2 border-primary-foreground border-t-transparent rounded-full" />
              Loading model...
            </span>
          ) : (
            "Load GGUF Model"
          )}
        </button>

        {error && (
          <p className="text-destructive text-sm mt-3">{error}</p>
        )}

        <p className="text-xs text-muted-foreground mt-4">
          Supports .gguf files (Llama, Mistral, Phi, etc.)
        </p>
      </div>
    </div>
  );
}
