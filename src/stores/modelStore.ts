import { create } from "zustand";
import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  LocalModelEntry,
  RecommendedModel,
  DownloadProgress,
} from "../types/model";

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
    set({ isLoading: true, loadingModelPath: path, loadError: null });
    try {
      const handle = await invoke<number>("load_model", {
        modelPath: path,
        params: null,
      });
      const name = path.split(/[/\\]/).pop()?.replace(".gguf", "") ?? "Model";
      set({
        loadedModelHandle: handle,
        loadedModelName: name,
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
      const errorMsg = typeof e === "object" && e !== null && "message" in e
        ? String((e as { message: string }).message)
        : String(e);
      console.error("Failed to load model:", errorMsg);
      set({ isLoading: false, loadingModelPath: null, loadError: errorMsg });
      return null;
    }
  },

  unloadModel: async (handle: number) => {
    try {
      await invoke("unload_model", { handle });
      set({ loadedModelHandle: null, loadedModelName: null });
    } catch (e) {
      console.error("Failed to unload model:", e);
    }
  },

  autoLoadModel: async () => {
    if (get().autoLoadAttempted || get().loadedModelHandle) return;
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

      // Download complete -- remove from active and refresh local models
      const updated = new Map(get().activeDownloads);
      updated.delete(model.id);
      set({ activeDownloads: updated });

      await get().scanLocalModels();
    } catch (e) {
      const updated = new Map(get().activeDownloads);
      updated.delete(model.id);
      const errors = new Map(get().downloadErrors);
      errors.set(model.id, String(e));
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
