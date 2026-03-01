import { create } from "zustand";
import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  LocalModelEntry,
  RecommendedModel,
  DownloadProgress,
} from "../types/model";

/** Extract a human-readable message from a Tauri error (which can be string, object, etc.) */
function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    if ("message" in e && typeof (e as { message: unknown }).message === "string")
      return (e as { message: string }).message;
    try {
      return JSON.stringify(e);
    } catch {
      return String(e);
    }
  }
  return String(e);
}

interface ActiveDownload {
  modelId: string;
  filename: string;
  progress: DownloadProgress;
}

interface ModelState {
  localModels: LocalModelEntry[];
  recommendedModels: RecommendedModel[];
  loadedModelHandle: number | null;
  loadedModelName: string | null;
  supportsVision: boolean;
  isLoading: boolean;
  loadingModelPath: string | null;
  loadError: string | null;
  autoLoadAttempted: boolean;
  activeDownloads: Map<string, ActiveDownload>;
  downloadErrors: Map<string, string>;

  scanLocalModels: () => Promise<void>;
  loadRecommendedModels: () => Promise<void>;
  loadModel: (path: string) => Promise<number | null>;
  unloadModel: (handle: number) => Promise<void>;
  autoLoadModel: () => Promise<void>;
  downloadModel: (model: RecommendedModel) => Promise<void>;
  cancelDownload: (filename: string) => Promise<void>;
  deleteModel: (path: string) => Promise<void>;
  clearLoadError: () => void;
}

export const useModelStore = create<ModelState>((set, get) => ({
  localModels: [],
  recommendedModels: [],
  loadedModelHandle: null,
  loadedModelName: null,
  supportsVision: false,
  isLoading: false,
  loadingModelPath: null,
  loadError: null,
  autoLoadAttempted: false,
  activeDownloads: new Map(),
  downloadErrors: new Map(),

  scanLocalModels: async () => {
    try {
      const models = await invoke<LocalModelEntry[]>("list_local_models");
      set({ localModels: models });
    } catch (e) {
      console.error("Failed to scan models:", e);
    }
  },

  loadRecommendedModels: async () => {
    try {
      const models = await invoke<RecommendedModel[]>(
        "get_recommended_models"
      );
      set({ recommendedModels: models });
    } catch (e) {
      console.error("Failed to load recommended models:", e);
    }
  },

  loadModel: async (path: string) => {
    // Prevent loading if already loading
    if (get().isLoading) return null;

    // Unload previous model first to free GPU VRAM
    const prevHandle = get().loadedModelHandle;
    if (prevHandle) {
      try {
        await invoke("unload_model", { handle: prevHandle });
      } catch {
        // Best effort unload
      }
      set({ loadedModelHandle: null, loadedModelName: null, supportsVision: false });
    }

    set({ isLoading: true, loadingModelPath: path, loadError: null });
    try {
      const handle = await invoke<number>("load_model", {
        modelPath: path,
        params: null,
      });
      const name = path.split(/[/\\]/).pop()?.replace(".gguf", "") ?? "Model";
      // Query model capabilities (vision support)
      let supportsVision = false;
      try {
        const caps = await invoke<{ supports_vision: boolean }>(
          "get_model_capabilities",
          { modelHandle: handle }
        );
        supportsVision = caps.supports_vision;
        console.log("[modelStore] Model capabilities:", caps, "supportsVision:", supportsVision);
      } catch (capErr) {
        console.error("[modelStore] get_model_capabilities failed:", capErr);
      }

      set({
        loadedModelHandle: handle,
        loadedModelName: name,
        supportsVision,
        isLoading: false,
        loadingModelPath: null,
        loadError: null,
      });
      // Remember this model for next launch
      invoke("update_settings", {
        settings: { last_model_path: path },
      }).catch(() => {});
      return handle;
    } catch (e) {
      const errorMsg = errorMessage(e);
      console.error("Failed to load model:", errorMsg);
      set({ isLoading: false, loadingModelPath: null, loadError: errorMsg });
      return null;
    }
  },

  unloadModel: async (handle: number) => {
    try {
      await invoke("unload_model", { handle });
      set({ loadedModelHandle: null, loadedModelName: null, supportsVision: false });
    } catch (e) {
      console.error("Failed to unload model:", e);
    }
  },

  autoLoadModel: async () => {
    // Guard against concurrent calls (React StrictMode, multiple renders)
    if (get().autoLoadAttempted || get().loadedModelHandle || get().isLoading) return;
    set({ autoLoadAttempted: true });

    // Try last used model from settings
    try {
      const settings = await invoke<Record<string, string>>("get_settings", {
        keys: ["last_model_path"],
      });
      const lastPath = settings.last_model_path;
      if (lastPath) {
        const handle = await get().loadModel(lastPath);
        if (handle) return;
      }
    } catch {
      // Settings not available, continue
    }

    // Fall back to first local model
    await get().scanLocalModels();
    const locals = get().localModels;
    if (locals.length > 0) {
      await get().loadModel(locals[0].file_path);
    }
  },

  downloadModel: async (model: RecommendedModel) => {
    const { activeDownloads, downloadErrors } = get();

    // Already downloading?
    if (activeDownloads.has(model.id)) return;

    const newDownloads = new Map(activeDownloads);
    newDownloads.set(model.id, {
      modelId: model.id,
      filename: model.filename,
      progress: {
        downloaded_bytes: 0,
        total_bytes: model.size_bytes,
        percent: 0,
        speed_bytes_per_sec: 0,
      },
    });

    const newErrors = new Map(downloadErrors);
    newErrors.delete(model.id);

    set({ activeDownloads: newDownloads, downloadErrors: newErrors });

    const onProgress = new Channel<DownloadProgress>();
    onProgress.onmessage = (progress: DownloadProgress) => {
      const current = new Map(get().activeDownloads);
      const dl = current.get(model.id);
      if (dl) {
        current.set(model.id, { ...dl, progress });
        set({ activeDownloads: current });
      }
    };

    try {
      await invoke<string>("download_model", {
        repoId: model.repo_id,
        filename: model.filename,
        onProgress,
      });

      // Auto-download companion mmproj file for vision models
      if (model.mmproj_filename) {
        const mmRepo = model.mmproj_repo_id ?? model.repo_id;
        const mmProgress = new Channel<DownloadProgress>();
        // Show mmproj download in the same progress slot
        mmProgress.onmessage = (progress: DownloadProgress) => {
          const current = new Map(get().activeDownloads);
          const dl = current.get(model.id);
          if (dl) {
            current.set(model.id, {
              ...dl,
              filename: model.mmproj_filename!,
              progress,
            });
            set({ activeDownloads: current });
          }
        };
        await invoke<string>("download_model", {
          repoId: mmRepo,
          filename: model.mmproj_filename,
          onProgress: mmProgress,
        });
      }

      // Download complete -- remove from active and refresh local models
      const updated = new Map(get().activeDownloads);
      updated.delete(model.id);
      set({ activeDownloads: updated });

      await get().scanLocalModels();

      // If this model (or its companion) was just downloaded and the model
      // is currently loaded, reload it so vision support gets detected.
      const currentName = get().loadedModelName;
      if (currentName && model.filename.replace(".gguf", "") === currentName) {
        const locals = get().localModels;
        const entry = locals.find((m) => m.name === currentName);
        if (entry) {
          await get().loadModel(entry.file_path);
        }
      }
    } catch (e) {
      const updated = new Map(get().activeDownloads);
      updated.delete(model.id);
      const errors = new Map(get().downloadErrors);
      errors.set(model.id, errorMessage(e));
      set({ activeDownloads: updated, downloadErrors: errors });
    }
  },

  cancelDownload: async (filename: string) => {
    try {
      await invoke("cancel_download", { filename });
    } catch (e) {
      console.error("Failed to cancel download:", e);
    }
  },

  deleteModel: async (path: string) => {
    try {
      await invoke("delete_model", { modelPath: path });
      await get().scanLocalModels();
    } catch (e) {
      console.error("Failed to delete model:", e);
    }
  },

  clearLoadError: () => set({ loadError: null }),
}));
