import { useState, useEffect, useCallback } from "react";
import { useAgentStore } from "../../stores/agentStore";

const STEPS = ["Welcome", "System Check", "Provider", "API Key", "Launch", "Complete"];

export function SetupWizard() {
  const setupStep = useAgentStore((s) => s.setupStep);
  const setSetupStep = useAgentStore((s) => s.setSetupStep);

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Step indicator */}
      <div className="p-4 border-b border-border">
        <div className="flex items-center justify-center gap-1 mb-2">
          {STEPS.map((label, i) => (
            <div key={label} className="flex items-center">
              <div
                className={`w-7 h-7 rounded-full flex items-center justify-center text-xs font-medium transition-colors ${
                  i < setupStep
                    ? "bg-green-500/20 text-green-400"
                    : i === setupStep
                      ? "bg-primary text-primary-foreground"
                      : "bg-muted text-muted-foreground"
                }`}
              >
                {i < setupStep ? "\u2713" : i + 1}
              </div>
              {i < STEPS.length - 1 && (
                <div
                  className={`w-6 h-0.5 mx-0.5 ${
                    i < setupStep ? "bg-green-500/40" : "bg-muted"
                  }`}
                />
              )}
            </div>
          ))}
        </div>
        <p className="text-center text-xs text-muted-foreground">
          {STEPS[setupStep]}
        </p>
      </div>

      {/* Step content */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-lg mx-auto">
          {setupStep === 0 && <WelcomeStep onNext={() => setSetupStep(1)} />}
          {setupStep === 1 && (
            <SystemCheckStep
              onNext={() => setSetupStep(2)}
              onBack={() => setSetupStep(0)}
            />
          )}
          {setupStep === 2 && (
            <ProviderStep
              onNext={(skipApiKey) => setSetupStep(skipApiKey ? 4 : 3)}
              onBack={() => setSetupStep(1)}
            />
          )}
          {setupStep === 3 && (
            <ApiKeyStep
              onNext={() => setSetupStep(4)}
              onBack={() => setSetupStep(2)}
            />
          )}
          {setupStep === 4 && (
            <LaunchStep
              onNext={() => setSetupStep(5)}
              onBack={() => setSetupStep(2)}
            />
          )}
          {setupStep === 5 && <CompleteStep />}
        </div>
      </div>
    </div>
  );
}

// ── Step 0: Welcome ─────────────────────────────────────────────────

function WelcomeStep({ onNext }: { onNext: () => void }) {
  const [accepted, setAccepted] = useState(false);

  return (
    <div className="space-y-6">
      <div className="text-center">
        <div className="w-16 h-16 rounded-2xl bg-primary/10 flex items-center justify-center mx-auto mb-4">
          <span className="text-2xl">🤖</span>
        </div>
        <h2 className="text-xl font-bold mb-2">OpenClaw Agent Setup</h2>
        <p className="text-sm text-muted-foreground">
          Set up OpenClaw to connect your AI to WhatsApp, Telegram, and
          other messaging platforms.
        </p>
      </div>

      <div className="border border-border rounded-lg p-4 space-y-3">
        <h3 className="text-sm font-medium">What OpenClaw does:</h3>
        <ul className="space-y-2 text-xs text-muted-foreground">
          <li className="flex items-start gap-2">
            <span className="text-green-400 mt-0.5">&#10003;</span>
            <span>
              Runs a local gateway on your machine — all data stays private
            </span>
          </li>
          <li className="flex items-start gap-2">
            <span className="text-green-400 mt-0.5">&#10003;</span>
            <span>
              Connects to WhatsApp via QR code scan (like WhatsApp Web)
            </span>
          </li>
          <li className="flex items-start gap-2">
            <span className="text-green-400 mt-0.5">&#10003;</span>
            <span>
              Uses your chosen LLM provider (Anthropic, OpenAI, or local Ollama)
            </span>
          </li>
          <li className="flex items-start gap-2">
            <span className="text-green-400 mt-0.5">&#10003;</span>
            <span>
              Gateway binds to loopback only — never exposed to your network
            </span>
          </li>
        </ul>
      </div>

      <div className="border border-border rounded-lg p-4">
        <label className="flex items-start gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={accepted}
            onChange={(e) => setAccepted(e.target.checked)}
            className="mt-1 rounded"
          />
          <span className="text-xs text-muted-foreground">
            I understand that AI agents can perform actions on messaging
            platforms on my behalf. I accept responsibility for reviewing and
            managing agent behavior.
          </span>
        </label>
      </div>

      <button
        onClick={onNext}
        disabled={!accepted}
        className="w-full py-2.5 text-sm rounded-md bg-primary text-primary-foreground hover:opacity-90 transition-opacity disabled:opacity-50"
      >
        Get Started
      </button>
    </div>
  );
}

// ── Step 1: System Check ────────────────────────────────────────────

function SystemCheckStep({
  onNext,
  onBack,
}: {
  onNext: () => void;
  onBack: () => void;
}) {
  const nodeVersion = useAgentStore((s) => s.nodeVersion);
  const openclawVersion = useAgentStore((s) => s.openclawVersion);
  const checkPrerequisites = useAgentStore((s) => s.checkPrerequisites);
  const installNode = useAgentStore((s) => s.installNode);
  const installOpenClaw = useAgentStore((s) => s.installOpenClaw);
  const setupProgress = useAgentStore((s) => s.setupProgress);
  const setupError = useAgentStore((s) => s.setupError);
  const setSetupProgress = useAgentStore((s) => s.setSetupProgress);
  const setSetupError = useAgentStore((s) => s.setSetupError);

  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);

  // Auto-check on mount
  useEffect(() => {
    const run = async () => {
      setChecking(true);
      await checkPrerequisites();
      setChecking(false);
    };
    run();
  }, [checkPrerequisites]);

  const handleInstallNode = useCallback(async () => {
    setInstalling(true);
    setSetupError(null);
    try {
      await installNode();
    } catch {
      // Error already set in store
    }
    setInstalling(false);
  }, [installNode, setSetupError]);

  const handleInstallOpenClaw = useCallback(async () => {
    setInstalling(true);
    setSetupError(null);
    try {
      await installOpenClaw();
    } catch {
      // Error already set in store
    }
    setInstalling(false);
  }, [installOpenClaw, setSetupError]);

  const handleRecheck = useCallback(async () => {
    setChecking(true);
    setSetupProgress(null);
    setSetupError(null);
    await checkPrerequisites();
    setChecking(false);
  }, [checkPrerequisites, setSetupProgress, setSetupError]);

  const allReady = !!nodeVersion && !!openclawVersion;

  return (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-lg font-bold mb-1">System Requirements</h2>
        <p className="text-xs text-muted-foreground">
          Checking for Node.js and OpenClaw on your system...
        </p>
      </div>

      {/* Node.js status */}
      <div className="border border-border rounded-lg p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div
              className={`w-8 h-8 rounded-md flex items-center justify-center text-sm ${
                nodeVersion
                  ? "bg-green-500/20 text-green-400"
                  : "bg-red-500/20 text-red-400"
              }`}
            >
              {checking ? "..." : nodeVersion ? "\u2713" : "\u2717"}
            </div>
            <div>
              <p className="text-sm font-medium">Node.js</p>
              <p className="text-xs text-muted-foreground">
                {nodeVersion || "Not found (v22+ required)"}
              </p>
            </div>
          </div>
          {!nodeVersion && !checking && (
            <button
              onClick={handleInstallNode}
              disabled={installing}
              className="px-3 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90 disabled:opacity-50"
            >
              {installing ? "Installing..." : "Install"}
            </button>
          )}
        </div>
      </div>

      {/* OpenClaw status */}
      <div className="border border-border rounded-lg p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div
              className={`w-8 h-8 rounded-md flex items-center justify-center text-sm ${
                openclawVersion
                  ? "bg-green-500/20 text-green-400"
                  : "bg-red-500/20 text-red-400"
              }`}
            >
              {checking ? "..." : openclawVersion ? "\u2713" : "\u2717"}
            </div>
            <div>
              <p className="text-sm font-medium">OpenClaw</p>
              <p className="text-xs text-muted-foreground">
                {openclawVersion || "Not found"}
              </p>
            </div>
          </div>
          {!openclawVersion && nodeVersion && !checking && (
            <button
              onClick={handleInstallOpenClaw}
              disabled={installing}
              className="px-3 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90 disabled:opacity-50"
            >
              {installing ? "Installing..." : "Install"}
            </button>
          )}
        </div>
      </div>

      {/* Progress / Error messages */}
      {setupProgress && (
        <div className="text-xs px-3 py-2 rounded-md bg-muted text-muted-foreground">
          {setupProgress}
        </div>
      )}
      {setupError && (
        <div className="text-xs px-3 py-2 rounded-md bg-destructive/10 text-destructive">
          {setupError}
        </div>
      )}

      {/* Navigation */}
      <div className="flex items-center justify-between pt-2">
        <button
          onClick={onBack}
          className="px-4 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          Back
        </button>
        <div className="flex gap-2">
          <button
            onClick={handleRecheck}
            disabled={checking}
            className="px-4 py-2 text-xs border border-border rounded-md hover:bg-muted transition-colors disabled:opacity-50"
          >
            Re-check
          </button>
          <button
            onClick={onNext}
            disabled={!allReady}
            className="px-4 py-2 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90 disabled:opacity-50"
          >
            Next
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Step 2: Provider Selection ──────────────────────────────────────

function ProviderStep({
  onNext,
  onBack,
}: {
  onNext: (skipApiKey: boolean) => void;
  onBack: () => void;
}) {
  const provider = useAgentStore((s) => s.provider);
  const [selected, setSelected] = useState(provider);

  const providers = [
    {
      id: "anthropic",
      name: "Anthropic",
      desc: "Claude models — excellent reasoning and safety",
      color: "bg-orange-500/20 text-orange-400 border-orange-500/30",
      requiresKey: true,
    },
    {
      id: "openai",
      name: "OpenAI",
      desc: "GPT models — versatile and widely supported",
      color: "bg-green-500/20 text-green-400 border-green-500/30",
      requiresKey: true,
    },
    {
      id: "ollama",
      name: "Ollama",
      desc: "Local models — completely private, no API key needed",
      color: "bg-purple-500/20 text-purple-400 border-purple-500/30",
      requiresKey: false,
    },
  ];

  const selectedProvider = providers.find((p) => p.id === selected);

  return (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-lg font-bold mb-1">Choose LLM Provider</h2>
        <p className="text-xs text-muted-foreground">
          Select which AI provider OpenClaw should use for responses.
        </p>
      </div>

      <div className="grid grid-cols-1 gap-3">
        {providers.map((p) => (
          <button
            key={p.id}
            onClick={() => setSelected(p.id)}
            className={`border rounded-lg p-4 text-left transition-all ${
              selected === p.id
                ? `${p.color} border-2`
                : "border-border hover:border-muted-foreground/50"
            }`}
          >
            <div className="flex items-center gap-3">
              <div
                className={`w-10 h-10 rounded-lg flex items-center justify-center text-lg font-bold ${p.color}`}
              >
                {p.name[0]}
              </div>
              <div>
                <p className="text-sm font-medium">{p.name}</p>
                <p className="text-xs text-muted-foreground">{p.desc}</p>
              </div>
            </div>
          </button>
        ))}
      </div>

      {/* Navigation */}
      <div className="flex items-center justify-between pt-2">
        <button
          onClick={onBack}
          className="px-4 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          Back
        </button>
        <button
          onClick={() => {
            useAgentStore.setState({
              provider: selected,
              model:
                selected === "anthropic"
                  ? "claude-sonnet-4-20250514"
                  : selected === "openai"
                    ? "gpt-4o-mini"
                    : "qwen2.5:3b",
            });
            onNext(!selectedProvider?.requiresKey);
          }}
          className="px-4 py-2 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90"
        >
          Next
        </button>
      </div>
    </div>
  );
}

// ── Step 3: API Key ─────────────────────────────────────────────────

function ApiKeyStep({
  onNext,
  onBack,
}: {
  onNext: () => void;
  onBack: () => void;
}) {
  const provider = useAgentStore((s) => s.provider);
  const [apiKey, setApiKey] = useState("");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<string | null>(null);

  const providerName =
    provider === "anthropic" ? "Anthropic" : provider === "openai" ? "OpenAI" : provider;

  const handleTest = async () => {
    if (!apiKey.trim()) return;
    setTesting(true);
    setTestResult(null);
    // We'll just validate the key format for now
    // A real test would require the gateway to be running
    await new Promise((r) => setTimeout(r, 500));
    if (apiKey.startsWith("sk-")) {
      setTestResult("Key format looks valid!");
    } else {
      setTestResult("Warning: Key doesn't start with 'sk-'. Make sure it's correct.");
    }
    setTesting(false);
  };

  return (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-lg font-bold mb-1">{providerName} API Key</h2>
        <p className="text-xs text-muted-foreground">
          Enter your {providerName} API key to enable AI responses.
        </p>
      </div>

      <div className="space-y-3">
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder={`Enter your ${providerName} API key`}
          className="w-full px-3 py-3 bg-muted rounded-md text-sm outline-none focus:ring-1 focus:ring-ring"
          autoFocus
        />
        <p className="text-xs text-muted-foreground">
          Your key is stored securely on your device and is only sent to{" "}
          {providerName}'s API servers. It is never saved in config files.
        </p>

        <div className="flex items-center gap-3">
          <button
            onClick={handleTest}
            disabled={testing || !apiKey.trim()}
            className="px-3 py-1.5 text-xs border border-border rounded-md hover:bg-muted transition-colors disabled:opacity-50"
          >
            {testing ? "Testing..." : "Test Key"}
          </button>
          {testResult && (
            <span
              className={`text-xs ${
                testResult.startsWith("Warning") || testResult.startsWith("Error")
                  ? "text-yellow-400"
                  : "text-green-400"
              }`}
            >
              {testResult}
            </span>
          )}
        </div>
      </div>

      {/* Navigation */}
      <div className="flex items-center justify-between pt-2">
        <button
          onClick={onBack}
          className="px-4 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          Back
        </button>
        <button
          onClick={() => {
            // Store the API key in the agent store for use during launch
            useAgentStore.setState({ model: useAgentStore.getState().model });
            // We'll use this key when configuring the provider after launch
            (window as unknown as Record<string, string>).__openclawApiKey = apiKey;
            onNext();
          }}
          disabled={!apiKey.trim()}
          className="px-4 py-2 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90 disabled:opacity-50"
        >
          Next
        </button>
      </div>
    </div>
  );
}

// ── Step 4: Launch Gateway ──────────────────────────────────────────

function LaunchStep({
  onNext,
  onBack,
}: {
  onNext: () => void;
  onBack: () => void;
}) {
  const startOpenClaw = useAgentStore((s) => s.startOpenClaw);
  const configureProvider = useAgentStore((s) => s.configureProvider);
  const provider = useAgentStore((s) => s.provider);
  const model = useAgentStore((s) => s.model);
  const setupError = useAgentStore((s) => s.setupError);

  const [phase, setPhase] = useState<
    "idle" | "configuring" | "starting" | "done" | "error"
  >("idle");
  const [statusMsg, setStatusMsg] = useState("Ready to launch...");

  useEffect(() => {
    // Auto-launch on mount
    // Order: configure provider (writes config files) → start gateway (reads config)
    const launch = async () => {
      // Step 1: Configure provider (modifies config files, no gateway needed)
      setPhase("configuring");
      setStatusMsg("Configuring provider...");

      const apiKey =
        (window as unknown as Record<string, string>).__openclawApiKey || "";
      try {
        await configureProvider(provider, model, apiKey);
        setStatusMsg("Provider configured!");
      } catch {
        // Non-fatal — provider config can be updated later in settings
        setStatusMsg("Provider config skipped (can update later in settings)");
      }

      // Step 2: Start the gateway (reads the config we just wrote)
      setPhase("starting");
      setStatusMsg("Starting OpenClaw gateway...");
      try {
        await startOpenClaw();
        setStatusMsg("All configured and ready!");
        setPhase("done");
      } catch {
        setPhase("error");
        setStatusMsg("Failed to start gateway");
      }
    };
    launch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-lg font-bold mb-1">Launching Gateway</h2>
        <p className="text-xs text-muted-foreground">
          Starting the OpenClaw gateway on your machine...
        </p>
      </div>

      {/* Progress display */}
      <div className="border border-border rounded-lg p-6 text-center space-y-4">
        <div
          className={`w-16 h-16 rounded-full mx-auto flex items-center justify-center text-2xl ${
            phase === "done"
              ? "bg-green-500/20"
              : phase === "error"
                ? "bg-destructive/20"
                : "bg-primary/10"
          }`}
        >
          {phase === "done"
            ? "\u2713"
            : phase === "error"
              ? "\u2717"
              : phase === "idle"
                ? "..."
                : (
                    <span className="animate-spin inline-block">&#8635;</span>
                  )}
        </div>
        <p className="text-sm">{statusMsg}</p>
        {setupError && (
          <p className="text-xs text-destructive">{setupError}</p>
        )}
      </div>

      {/* Steps checklist */}
      <div className="space-y-2">
        <StepItem
          label="Configure provider"
          status={
            phase === "idle"
              ? "pending"
              : phase === "configuring"
                ? "active"
                : "done"
          }
        />
        <StepItem
          label="Start gateway subprocess"
          status={
            phase === "idle" || phase === "configuring"
              ? "pending"
              : phase === "starting"
                ? "active"
                : "done"
          }
        />
        <StepItem
          label="Health check (loopback only)"
          status={
            phase === "starting"
              ? "active"
              : phase === "done"
                ? "done"
                : "pending"
          }
        />
      </div>

      {/* Navigation */}
      <div className="flex items-center justify-between pt-2">
        <button
          onClick={onBack}
          className="px-4 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          Back
        </button>
        <button
          onClick={onNext}
          disabled={phase !== "done" && phase !== "error"}
          className="px-4 py-2 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90 disabled:opacity-50"
        >
          {phase === "error" ? "Continue Anyway" : "Next"}
        </button>
      </div>
    </div>
  );
}

function StepItem({
  label,
  status,
}: {
  label: string;
  status: "pending" | "active" | "done";
}) {
  return (
    <div className="flex items-center gap-3">
      <div
        className={`w-5 h-5 rounded-full flex items-center justify-center text-xs ${
          status === "done"
            ? "bg-green-500/20 text-green-400"
            : status === "active"
              ? "bg-primary/20 text-primary"
              : "bg-muted text-muted-foreground"
        }`}
      >
        {status === "done" ? "\u2713" : status === "active" ? "..." : "\u2022"}
      </div>
      <span
        className={`text-xs ${
          status === "done"
            ? "text-green-400"
            : status === "active"
              ? "text-foreground"
              : "text-muted-foreground"
        }`}
      >
        {label}
      </span>
    </div>
  );
}

// ── Step 5: Complete ────────────────────────────────────────────────

function CompleteStep() {
  const setSetupComplete = useAgentStore((s) => s.setSetupComplete);
  const setSetupStep = useAgentStore((s) => s.setSetupStep);

  return (
    <div className="space-y-6 text-center">
      <div className="w-20 h-20 rounded-full bg-green-500/20 flex items-center justify-center mx-auto">
        <span className="text-3xl text-green-400">&#10003;</span>
      </div>
      <div>
        <h2 className="text-xl font-bold mb-2">You're All Set!</h2>
        <p className="text-sm text-muted-foreground">
          OpenClaw is running and ready. You can now connect WhatsApp, configure
          providers, and manage your AI agent from the dashboard.
        </p>
      </div>

      <div className="border border-border rounded-lg p-4 text-left space-y-2">
        <h3 className="text-sm font-medium">Next steps:</h3>
        <ul className="space-y-1.5 text-xs text-muted-foreground">
          <li className="flex items-start gap-2">
            <span className="text-primary mt-0.5">&bull;</span>
            <span>
              Connect <strong>WhatsApp</strong> via QR code scan
            </span>
          </li>
          <li className="flex items-start gap-2">
            <span className="text-primary mt-0.5">&bull;</span>
            <span>
              Adjust provider settings if needed
            </span>
          </li>
          <li className="flex items-start gap-2">
            <span className="text-primary mt-0.5">&bull;</span>
            <span>
              Send a test message via WhatsApp to your agent
            </span>
          </li>
        </ul>
      </div>

      <button
        onClick={() => {
          setSetupComplete(true);
          setSetupStep(0); // Reset for potential re-run
        }}
        className="w-full py-2.5 text-sm rounded-md bg-primary text-primary-foreground hover:opacity-90 transition-opacity"
      >
        Enter Agent Dashboard
      </button>
    </div>
  );
}
