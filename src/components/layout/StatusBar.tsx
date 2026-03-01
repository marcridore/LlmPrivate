import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "../../stores/chatStore";
import { useModelStore } from "../../stores/modelStore";

interface BackendCapabilities {
  gpu_compiled: boolean;
  gpu_backend: string | null;
}

export function StatusBar() {
  const tokensPerSecond = useChatStore((s) => s.tokensPerSecond);
  const isGenerating = useChatStore((s) => s.isGenerating);
  const loadedModelName = useModelStore((s) => s.loadedModelName);
  const [gpuInfo, setGpuInfo] = useState<BackendCapabilities | null>(null);

  useEffect(() => {
    invoke<BackendCapabilities>("get_backend_capabilities").then(setGpuInfo).catch(() => {});
  }, []);

  return (
    <div className="h-6 border-t border-border bg-card flex items-center px-3 text-xs text-muted-foreground gap-4">
      <span>
        {loadedModelName
          ? `Model: ${loadedModelName}`
          : "No model loaded"}
      </span>
      {isGenerating && (
        <span className="text-green-400 animate-pulse">Generating...</span>
      )}
      {tokensPerSecond > 0 && (
        <span>{tokensPerSecond.toFixed(1)} tokens/sec</span>
      )}
      {gpuInfo && (
        <span className={gpuInfo.gpu_compiled ? "text-green-400" : "text-yellow-500"}>
          {gpuInfo.gpu_compiled
            ? `GPU: ${gpuInfo.gpu_backend?.toUpperCase()}`
            : "CPU only"}
        </span>
      )}
      <span className="ml-auto">v0.1.0</span>
    </div>
  );
}
