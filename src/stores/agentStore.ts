import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

interface SetupStatus {
  node_version: string | null;
  openclaw_version: string | null;
}

interface OpenClawStatus {
  running: boolean;
  port: number | null;
}

interface QrResponse {
  qr_data_url: string | null;
  message: string;
}

interface WaitResponse {
  connected: boolean;
  message: string;
}

interface ChannelStatus {
  whatsapp_connected: boolean;
  whatsapp_account_id: string | null;
}

interface AgentState {
  // Setup state
  setupComplete: boolean;
  setupStep: number;
  nodeVersion: string | null;
  openclawVersion: string | null;
  setupProgress: string | null;
  setupError: string | null;

  // Runtime state
  openclawRunning: boolean;
  openclawPort: number | null;

  // WhatsApp state
  whatsappConnected: boolean;
  whatsappAccountId: string | null;
  qrDataUrl: string | null;
  qrLoading: boolean;
  whatsappError: string | null;

  // Provider config (stored locally for UI state)
  provider: string;
  model: string;

  // Actions
  checkPrerequisites: () => Promise<SetupStatus>;
  installNode: () => Promise<void>;
  installOpenClaw: () => Promise<void>;
  startOpenClaw: () => Promise<void>;
  stopOpenClaw: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  configureProvider: (
    provider: string,
    model: string,
    apiKey: string
  ) => Promise<void>;
  whatsappStartLogin: (force: boolean) => Promise<QrResponse | null>;
  whatsappWaitForScan: () => Promise<WaitResponse | null>;
  whatsappLogout: () => Promise<void>;
  refreshChannelStatus: () => Promise<void>;
  setSetupStep: (step: number) => void;
  setSetupComplete: (complete: boolean) => void;
  setSetupProgress: (msg: string | null) => void;
  setSetupError: (msg: string | null) => void;
}

function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "object" && e !== null) {
    const obj = e as Record<string, unknown>;
    if (typeof obj.message === "string") return obj.message;
    return JSON.stringify(e);
  }
  return String(e);
}

export const useAgentStore = create<AgentState>((set, get) => ({
  // Initial state
  setupComplete: false,
  setupStep: 0,
  nodeVersion: null,
  openclawVersion: null,
  setupProgress: null,
  setupError: null,

  openclawRunning: false,
  openclawPort: null,

  whatsappConnected: false,
  whatsappAccountId: null,
  qrDataUrl: null,
  qrLoading: false,
  whatsappError: null,

  provider: "anthropic",
  model: "claude-sonnet-4-20250514",

  // ── Setup actions ───────────────────────────────────────────────

  checkPrerequisites: async () => {
    try {
      const status = await invoke<SetupStatus>(
        "openclaw_check_prerequisites"
      );
      set({
        nodeVersion: status.node_version,
        openclawVersion: status.openclaw_version,
      });
      return status;
    } catch (e) {
      set({ setupError: errorMessage(e) });
      return { node_version: null, openclaw_version: null };
    }
  },

  installNode: async () => {
    set({ setupProgress: "Downloading and installing Node.js...", setupError: null });
    try {
      await invoke("openclaw_install_node");
      set({ setupProgress: "Node.js installed successfully" });
      // Re-check
      await get().checkPrerequisites();
    } catch (e) {
      set({ setupError: errorMessage(e), setupProgress: null });
      throw e;
    }
  },

  installOpenClaw: async () => {
    set({ setupProgress: "Installing OpenClaw via npm...", setupError: null });
    try {
      await invoke("openclaw_install");
      set({ setupProgress: "OpenClaw installed successfully" });
      // Re-check
      await get().checkPrerequisites();
    } catch (e) {
      set({ setupError: errorMessage(e), setupProgress: null });
      throw e;
    }
  },

  // ── Lifecycle actions ───────────────────────────────────────────

  startOpenClaw: async () => {
    set({ setupProgress: "Starting OpenClaw gateway...", setupError: null });
    try {
      const port = await invoke<number>("openclaw_start");
      set({
        openclawRunning: true,
        openclawPort: port,
        setupProgress: `OpenClaw gateway running on port ${port}`,
      });
    } catch (e) {
      set({
        setupError: errorMessage(e),
        setupProgress: null,
        openclawRunning: false,
      });
      throw e;
    }
  },

  stopOpenClaw: async () => {
    try {
      await invoke("openclaw_stop");
      set({ openclawRunning: false, openclawPort: null });
    } catch (e) {
      set({ setupError: errorMessage(e) });
    }
  },

  refreshStatus: async () => {
    try {
      const status = await invoke<OpenClawStatus>("openclaw_status");
      set({
        openclawRunning: status.running,
        openclawPort: status.port,
      });
    } catch {
      set({ openclawRunning: false, openclawPort: null });
    }
  },

  // ── Provider actions ────────────────────────────────────────────

  configureProvider: async (provider, model, apiKey) => {
    try {
      await invoke("openclaw_configure_provider", {
        provider,
        model,
        apiKey,
      });
      set({ provider, model });
    } catch (e) {
      set({ setupError: errorMessage(e) });
      throw e;
    }
  },

  // ── WhatsApp actions ────────────────────────────────────────────

  whatsappStartLogin: async (force) => {
    set({ qrLoading: true, whatsappError: null, qrDataUrl: null });
    try {
      const resp = await invoke<QrResponse>("openclaw_whatsapp_start", {
        force,
      });
      set({
        qrDataUrl: resp.qr_data_url,
        qrLoading: false,
      });
      return resp;
    } catch (e) {
      set({
        whatsappError: errorMessage(e),
        qrLoading: false,
      });
      return null;
    }
  },

  whatsappWaitForScan: async () => {
    try {
      const resp = await invoke<WaitResponse>("openclaw_whatsapp_wait");
      if (resp.connected) {
        set({ whatsappConnected: true, qrDataUrl: null });
      }
      return resp;
    } catch (e) {
      set({ whatsappError: errorMessage(e) });
      return null;
    }
  },

  whatsappLogout: async () => {
    try {
      await invoke("openclaw_whatsapp_logout");
      set({
        whatsappConnected: false,
        whatsappAccountId: null,
        qrDataUrl: null,
      });
    } catch (e) {
      set({ whatsappError: errorMessage(e) });
    }
  },

  refreshChannelStatus: async () => {
    try {
      const status = await invoke<ChannelStatus>(
        "openclaw_channel_status"
      );
      set({
        whatsappConnected: status.whatsapp_connected,
        whatsappAccountId: status.whatsapp_account_id,
      });
    } catch {
      // Gateway might not be running
    }
  },

  // ── UI state ────────────────────────────────────────────────────

  setSetupStep: (step) => set({ setupStep: step }),
  setSetupComplete: (complete) => set({ setupComplete: complete }),
  setSetupProgress: (msg) => set({ setupProgress: msg }),
  setSetupError: (msg) => set({ setupError: msg }),
}));
