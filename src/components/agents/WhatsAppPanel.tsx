import { useState, useEffect, useRef, useCallback } from "react";
import { useAgentStore } from "../../stores/agentStore";
import { WhatsAppAllowlistPanel } from "./WhatsAppAllowlistPanel";

export function WhatsAppPanel() {
  const whatsappConnected = useAgentStore((s) => s.whatsappConnected);
  const whatsappAccountId = useAgentStore((s) => s.whatsappAccountId);
  const openclawRunning = useAgentStore((s) => s.openclawRunning);
  const refreshChannelStatus = useAgentStore((s) => s.refreshChannelStatus);
  const whatsappLogout = useAgentStore((s) => s.whatsappLogout);
  const whatsappError = useAgentStore((s) => s.whatsappError);

  const [showQrModal, setShowQrModal] = useState(false);
  const [unlinking, setUnlinking] = useState(false);

  // Refresh status on mount
  useEffect(() => {
    if (openclawRunning) {
      refreshChannelStatus();
    }
  }, [openclawRunning, refreshChannelStatus]);

  const handleUnlink = async () => {
    setUnlinking(true);
    await whatsappLogout();
    await refreshChannelStatus();
    setUnlinking(false);
  };

  if (!openclawRunning) {
    return (
      <div className="max-w-2xl">
        <div className="border border-border rounded-lg p-6 text-center">
          <p className="text-sm text-muted-foreground mb-2">
            Start the OpenClaw gateway first to manage WhatsApp.
          </p>
          <p className="text-xs text-muted-foreground">
            Go to the Status tab and click "Start Gateway".
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-2xl space-y-6">
      {/* Connection status */}
      <section>
        <h3 className="text-sm font-medium mb-3">WhatsApp Connection</h3>
        <div className="border border-border rounded-lg p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div
                className={`w-10 h-10 rounded-lg flex items-center justify-center text-lg ${
                  whatsappConnected
                    ? "bg-green-500/20 text-green-400"
                    : "bg-muted text-muted-foreground"
                }`}
              >
                {whatsappConnected ? "\u2713" : "W"}
              </div>
              <div>
                <p className="text-sm font-medium">
                  {whatsappConnected ? "Connected" : "Not Connected"}
                </p>
                <p className="text-xs text-muted-foreground">
                  {whatsappConnected && whatsappAccountId
                    ? `Account: ${whatsappAccountId}`
                    : "Scan a QR code to link your WhatsApp"}
                </p>
              </div>
            </div>
            <div>
              {whatsappConnected ? (
                <button
                  onClick={handleUnlink}
                  disabled={unlinking}
                  className="px-3 py-1.5 text-xs border border-border rounded-md hover:bg-muted text-muted-foreground hover:text-destructive transition-colors disabled:opacity-50"
                >
                  {unlinking ? "Unlinking..." : "Unlink"}
                </button>
              ) : (
                <button
                  onClick={() => setShowQrModal(true)}
                  className="px-4 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90 transition-opacity"
                >
                  Link WhatsApp
                </button>
              )}
            </div>
          </div>
        </div>
      </section>

      {/* Error display */}
      {whatsappError && (
        <div className="text-xs px-3 py-2 rounded-md bg-destructive/10 text-destructive">
          {whatsappError}
        </div>
      )}

      {/* Message policy / allowlist */}
      <WhatsAppAllowlistPanel />

      {/* How it works */}
      <section>
        <h3 className="text-sm font-medium mb-2">How it works</h3>
        <div className="text-xs text-muted-foreground space-y-2">
          <p>
            WhatsApp connection uses the same protocol as WhatsApp Web. Your
            messages are end-to-end encrypted and processed locally by the
            OpenClaw gateway.
          </p>
          <ul className="space-y-1 ml-4">
            <li>
              &bull; Click "Link WhatsApp" to generate a QR code
            </li>
            <li>
              &bull; Open WhatsApp on your phone {">"} Linked Devices {">"} Link a
              Device
            </li>
            <li>&bull; Scan the QR code displayed here</li>
            <li>&bull; Your agent will respond to incoming messages</li>
          </ul>
        </div>
      </section>

      {/* QR Modal */}
      {showQrModal && (
        <QrModal
          onClose={() => {
            setShowQrModal(false);
            refreshChannelStatus();
          }}
        />
      )}
    </div>
  );
}

// ── QR Code Modal ───────────────────────────────────────────────────

function QrModal({ onClose }: { onClose: () => void }) {
  const whatsappStartLogin = useAgentStore((s) => s.whatsappStartLogin);
  const whatsappWaitForScan = useAgentStore((s) => s.whatsappWaitForScan);
  const startOpenClaw = useAgentStore((s) => s.startOpenClaw);
  const refreshChannelStatus = useAgentStore((s) => s.refreshChannelStatus);
  const qrDataUrl = useAgentStore((s) => s.qrDataUrl);
  const qrLoading = useAgentStore((s) => s.qrLoading);

  const [status, setStatus] = useState<string>("Generating QR code...");
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState(false);
  const abortRef = useRef(false);
  const refreshIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Start the QR login flow
  const startFlow = useCallback(
    async (force: boolean) => {
      abortRef.current = false;
      setError(null);
      setStatus("Generating QR code...");

      // If force, logout first to clear stale sessions
      if (force) {
        try {
          await useAgentStore.getState().whatsappLogout();
          await new Promise((r) => setTimeout(r, 800));
        } catch {
          // Ignore logout errors
        }
      }

      // Get initial QR code
      const qrResp = await whatsappStartLogin(force);
      if (!qrResp || abortRef.current) return;

      if (qrResp.qr_data_url) {
        setStatus("Scan this QR code with your phone");

        // Start QR refresh interval (every 18s — Baileys rotates QRs ~every 20s)
        refreshIntervalRef.current = setInterval(async () => {
          if (abortRef.current) {
            if (refreshIntervalRef.current) {
              clearInterval(refreshIntervalRef.current);
            }
            return;
          }
          await whatsappStartLogin(false);
        }, 18000);

        // Start long-poll for scan
        const waitResp = await whatsappWaitForScan();
        if (abortRef.current) return;

        // Stop refresh
        if (refreshIntervalRef.current) {
          clearInterval(refreshIntervalRef.current);
          refreshIntervalRef.current = null;
        }

        if (!waitResp) {
          setError("Connection failed. Try again.");
          return;
        }

        if (waitResp.connected) {
          setConnected(true);
          setStatus("WhatsApp connected successfully!");
          useAgentStore.setState({ whatsappConnected: true, qrDataUrl: null });
          // Auto-close after a moment
          setTimeout(() => {
            if (!abortRef.current) onClose();
          }, 2500);
          return;
        }

        // Handle special cases
        if (waitResp.message === "stream_errored") {
          // 515: QR scanned successfully but Baileys WebSocket needs restart
          setStatus("QR scanned! Restarting gateway to complete connection...");
          try {
            await startOpenClaw();
            // Poll for connection
            for (let i = 0; i < 15; i++) {
              if (abortRef.current) return;
              await new Promise((r) => setTimeout(r, 2000));
              await refreshChannelStatus();
              const state = useAgentStore.getState();
              if (state.whatsappConnected) {
                setConnected(true);
                setStatus("WhatsApp connected successfully!");
                setTimeout(() => {
                  if (!abortRef.current) onClose();
                }, 2500);
                return;
              }
            }
            setError("Connection timed out after gateway restart. Try again.");
          } catch {
            setError("Failed to restart gateway. Please try again.");
          }
          return;
        }

        if (waitResp.message === "qr_expired") {
          setError("QR code expired. Click 'Try Again' to generate a new one.");
          return;
        }

        setError(`Unexpected response: ${waitResp.message}`);
      } else {
        setError(qrResp.message || "Failed to generate QR code");
      }
    },
    [whatsappStartLogin, whatsappWaitForScan, startOpenClaw, refreshChannelStatus, onClose]
  );

  // Auto-start on mount
  useEffect(() => {
    startFlow(false);
    return () => {
      abortRef.current = true;
      if (refreshIntervalRef.current) {
        clearInterval(refreshIntervalRef.current);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-card border border-border rounded-xl p-6 w-[400px] max-h-[90vh] overflow-y-auto shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-sm font-medium">Link WhatsApp</h3>
          <button
            onClick={() => {
              abortRef.current = true;
              onClose();
            }}
            className="text-muted-foreground hover:text-foreground text-lg leading-none"
          >
            &times;
          </button>
        </div>

        {/* QR Code area */}
        <div className="flex flex-col items-center gap-4">
          {connected ? (
            <div className="w-48 h-48 rounded-lg bg-green-500/10 flex items-center justify-center">
              <span className="text-5xl text-green-400">&#10003;</span>
            </div>
          ) : qrDataUrl ? (
            <div className="bg-white p-3 rounded-lg">
              <img
                src={qrDataUrl}
                alt="WhatsApp QR Code"
                className="w-48 h-48"
              />
            </div>
          ) : qrLoading ? (
            <div className="w-48 h-48 rounded-lg bg-muted flex items-center justify-center">
              <span className="text-sm text-muted-foreground animate-pulse">
                Loading...
              </span>
            </div>
          ) : (
            <div className="w-48 h-48 rounded-lg bg-muted flex items-center justify-center">
              <span className="text-sm text-muted-foreground">
                No QR code
              </span>
            </div>
          )}

          {/* Status message */}
          <p
            className={`text-xs text-center ${
              connected ? "text-green-400" : "text-muted-foreground"
            }`}
          >
            {status}
          </p>

          {/* Error */}
          {error && (
            <div className="w-full text-xs px-3 py-2 rounded-md bg-destructive/10 text-destructive text-center">
              {error}
            </div>
          )}

          {/* Instructions */}
          {!connected && !error && qrDataUrl && (
            <div className="text-xs text-muted-foreground text-center space-y-1">
              <p>Open WhatsApp on your phone</p>
              <p>
                Go to <strong>Settings {">"} Linked Devices {">"} Link a Device</strong>
              </p>
              <p>Point your phone at this QR code</p>
            </div>
          )}

          {/* Retry button */}
          {error && (
            <button
              onClick={() => startFlow(true)}
              className="px-4 py-2 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90"
            >
              Try Again
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
