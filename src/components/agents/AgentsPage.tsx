import { useState, useEffect } from "react";
import { useAgentStore } from "../../stores/agentStore";
import { SetupWizard } from "./SetupWizard";
import { WhatsAppPanel } from "./WhatsAppPanel";

type Tab = "status" | "whatsapp" | "settings";

export function AgentsPage() {
  const setupComplete = useAgentStore((s) => s.setupComplete);
  const openclawRunning = useAgentStore((s) => s.openclawRunning);
  const openclawPort = useAgentStore((s) => s.openclawPort);
  const startOpenClaw = useAgentStore((s) => s.startOpenClaw);
  const stopOpenClaw = useAgentStore((s) => s.stopOpenClaw);
  const refreshStatus = useAgentStore((s) => s.refreshStatus);
  const provider = useAgentStore((s) => s.provider);
  const model = useAgentStore((s) => s.model);

  const [tab, setTab] = useState<Tab>("status");

  // Refresh status on mount
  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  // Show setup wizard if not configured yet
  if (!setupComplete) {
    return <SetupWizard />;
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Header with tabs */}
      <div className="p-4 border-b border-border">
        <div className="flex items-center justify-between mb-3">
          <h1 className="text-lg font-bold">Agent Dashboard</h1>
          <div className="flex items-center gap-2">
            <span
              className={`w-2 h-2 rounded-full ${
                openclawRunning ? "bg-green-500" : "bg-muted-foreground"
              }`}
            />
            <span className="text-xs text-muted-foreground">
              {openclawRunning
                ? `Gateway running (port ${openclawPort})`
                : "Gateway stopped"}
            </span>
          </div>
        </div>
        <div className="flex gap-1">
          {(["status", "whatsapp", "settings"] as const).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`px-3 py-1.5 text-xs rounded capitalize transition-colors ${
                tab === t
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-muted"
              }`}
            >
              {t === "whatsapp" ? "WhatsApp" : t}
            </button>
          ))}
        </div>
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto p-4">
        {tab === "status" && (
          <StatusTab
            running={openclawRunning}
            port={openclawPort}
            provider={provider}
            model={model}
            onStart={startOpenClaw}
            onStop={stopOpenClaw}
          />
        )}
        {tab === "whatsapp" && <WhatsAppPanel />}
        {tab === "settings" && <AgentSettingsTab />}
      </div>
    </div>
  );
}

function StatusTab({
  running,
  port,
  provider,
  model,
  onStart,
  onStop,
}: {
  running: boolean;
  port: number | null;
  provider: string;
  model: string;
  onStart: () => Promise<void>;
  onStop: () => Promise<void>;
}) {
  const [actionInProgress, setActionInProgress] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleAction = async (action: () => Promise<void>) => {
    setActionInProgress(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(
        e instanceof Error
          ? e.message
          : typeof e === "object"
            ? JSON.stringify(e)
            : String(e)
      );
    } finally {
      setActionInProgress(false);
    }
  };

  return (
    <div className="max-w-2xl space-y-6">
      {/* Gateway status */}
      <section>
        <h3 className="text-sm font-medium mb-3">OpenClaw Gateway</h3>
        <div className="border border-border rounded-lg p-4">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <span
                className={`w-3 h-3 rounded-full ${
                  running ? "bg-green-500" : "bg-red-500/50"
                }`}
              />
              <span className="text-sm font-medium">
                {running ? "Running" : "Stopped"}
              </span>
            </div>
            <button
              onClick={() => handleAction(running ? onStop : onStart)}
              disabled={actionInProgress}
              className={`px-4 py-1.5 text-xs rounded-md transition-colors disabled:opacity-50 ${
                running
                  ? "border border-border hover:bg-muted"
                  : "bg-primary text-primary-foreground hover:opacity-90"
              }`}
            >
              {actionInProgress
                ? "..."
                : running
                  ? "Stop Gateway"
                  : "Start Gateway"}
            </button>
          </div>
          {port && (
            <p className="text-xs text-muted-foreground">
              Listening on 127.0.0.1:{port} (loopback only)
            </p>
          )}
        </div>
      </section>

      {/* Provider info */}
      <section>
        <h3 className="text-sm font-medium mb-3">Active Provider</h3>
        <div className="border border-border rounded-lg p-4">
          <div className="flex items-center gap-3">
            <div
              className={`w-10 h-10 rounded-lg flex items-center justify-center text-sm font-bold ${
                provider === "anthropic"
                  ? "bg-orange-500/20 text-orange-400"
                  : provider === "openai"
                    ? "bg-green-500/20 text-green-400"
                    : "bg-purple-500/20 text-purple-400"
              }`}
            >
              {provider === "anthropic"
                ? "A"
                : provider === "openai"
                  ? "O"
                  : "L"}
            </div>
            <div>
              <p className="text-sm font-medium capitalize">{provider}</p>
              <p className="text-xs text-muted-foreground">{model}</p>
            </div>
          </div>
        </div>
      </section>

      {/* Error display */}
      {error && (
        <div className="text-xs px-3 py-2 rounded-md bg-destructive/10 text-destructive">
          {error}
        </div>
      )}
    </div>
  );
}

function AgentSettingsTab() {
  const provider = useAgentStore((s) => s.provider);
  const model = useAgentStore((s) => s.model);
  const configureProvider = useAgentStore((s) => s.configureProvider);
  const openclawRunning = useAgentStore((s) => s.openclawRunning);
  const setSetupComplete = useAgentStore((s) => s.setSetupComplete);

  const [selectedProvider, setSelectedProvider] = useState(provider);
  const [selectedModel, setSelectedModel] = useState(model);
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const providers = [
    {
      id: "anthropic",
      name: "Anthropic",
      models: ["claude-sonnet-4-20250514", "claude-haiku-4-20250414"],
      requiresKey: true,
      color: "bg-orange-500/20 text-orange-400",
    },
    {
      id: "openai",
      name: "OpenAI",
      models: ["gpt-4o-mini", "gpt-4o", "gpt-4-turbo"],
      requiresKey: true,
      color: "bg-green-500/20 text-green-400",
    },
    {
      id: "ollama",
      name: "Ollama",
      models: ["qwen2.5:3b", "llama3.1", "mistral", "phi3:mini"],
      requiresKey: false,
      color: "bg-purple-500/20 text-purple-400",
    },
  ];

  const currentProvider = providers.find((p) => p.id === selectedProvider);

  const handleSave = async () => {
    setSaving(true);
    setStatus(null);
    try {
      await configureProvider(selectedProvider, selectedModel, apiKey);
      setStatus(
        openclawRunning
          ? "Provider configured! Restart the gateway to apply changes."
          : "Provider configured successfully!"
      );
    } catch (e) {
      setStatus(
        `Error: ${e instanceof Error ? e.message : String(e)}`
      );
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="max-w-2xl space-y-6">
      {/* Provider selection */}
      <section>
        <h3 className="text-sm font-medium mb-3">LLM Provider</h3>
        <div className="grid grid-cols-3 gap-3">
          {providers.map((p) => (
            <button
              key={p.id}
              onClick={() => {
                setSelectedProvider(p.id);
                setSelectedModel(p.models[0]);
              }}
              className={`border rounded-lg p-3 text-left transition-colors ${
                selectedProvider === p.id
                  ? "border-primary bg-primary/5"
                  : "border-border hover:border-muted-foreground/50"
              }`}
            >
              <div
                className={`w-8 h-8 rounded-md flex items-center justify-center text-xs font-bold mb-2 ${p.color}`}
              >
                {p.name[0]}
              </div>
              <p className="text-sm font-medium">{p.name}</p>
              <p className="text-xs text-muted-foreground">
                {p.requiresKey ? "API key required" : "Local inference"}
              </p>
            </button>
          ))}
        </div>
      </section>

      {/* Model selection */}
      {currentProvider && (
        <section>
          <h3 className="text-sm font-medium mb-2">Model</h3>
          <select
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
            className="w-full px-3 py-2 bg-muted rounded-md text-sm outline-none focus:ring-1 focus:ring-ring"
          >
            {currentProvider.models.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </section>
      )}

      {/* API Key */}
      {currentProvider?.requiresKey && (
        <section>
          <h3 className="text-sm font-medium mb-2">API Key</h3>
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={`Enter your ${currentProvider.name} API key`}
            className="w-full px-3 py-2 bg-muted rounded-md text-sm outline-none focus:ring-1 focus:ring-ring"
          />
          <p className="text-xs text-muted-foreground mt-1">
            Stored securely on your device. Never sent anywhere except{" "}
            {currentProvider.name}.
          </p>
        </section>
      )}

      {/* Save button */}
      <div className="flex items-center gap-3">
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:opacity-90 transition-opacity disabled:opacity-50"
        >
          {saving ? "Saving..." : "Save Provider Settings"}
        </button>
        {status && (
          <span
            className={`text-xs ${
              status.startsWith("Error")
                ? "text-destructive"
                : "text-green-400"
            }`}
          >
            {status}
          </span>
        )}
      </div>

      {/* Re-run setup */}
      <section className="pt-4 border-t border-border">
        <button
          onClick={() => setSetupComplete(false)}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          Re-run Setup Wizard
        </button>
      </section>
    </div>
  );
}
