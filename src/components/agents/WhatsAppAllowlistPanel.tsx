import { useState, useEffect } from "react";
import { useAgentStore } from "../../stores/agentStore";

export function WhatsAppAllowlistPanel() {
  const whatsappConfig = useAgentStore((s) => s.whatsappConfig);
  const fetchWhatsAppConfig = useAgentStore((s) => s.fetchWhatsAppConfig);
  const saveWhatsAppConfig = useAgentStore((s) => s.saveWhatsAppConfig);

  const [dmPolicy, setDmPolicy] = useState("allowlist");
  const [allowedNumbers, setAllowedNumbers] = useState<string[]>([]);
  const [newNumber, setNewNumber] = useState("");
  const [selfChatMode, setSelfChatMode] = useState(false);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  // Load config on mount
  useEffect(() => {
    fetchWhatsAppConfig();
  }, [fetchWhatsAppConfig]);

  // Sync local state when config loads
  useEffect(() => {
    if (whatsappConfig) {
      setDmPolicy(whatsappConfig.dm_policy);
      setAllowedNumbers(whatsappConfig.allowed_numbers);
      setSelfChatMode(whatsappConfig.self_chat_mode);
    }
  }, [whatsappConfig]);

  const handleAddNumber = () => {
    let num = newNumber.trim();
    if (!num) return;
    // Auto-prefix with + if missing
    if (!num.startsWith("+")) {
      num = "+" + num;
    }
    // Don't add duplicates
    if (allowedNumbers.includes(num)) {
      setNewNumber("");
      return;
    }
    setAllowedNumbers([...allowedNumbers, num]);
    setNewNumber("");
  };

  const handleRemoveNumber = (num: string) => {
    setAllowedNumbers(allowedNumbers.filter((n) => n !== num));
  };

  const handleSave = async () => {
    setSaving(true);
    setStatus(null);
    try {
      await saveWhatsAppConfig({
        dm_policy: dmPolicy,
        allowed_numbers: allowedNumbers,
        group_policy: whatsappConfig?.group_policy ?? "allowlist",
        allowed_groups: whatsappConfig?.allowed_groups ?? [],
        self_chat_mode: selfChatMode,
      });
      setStatus("Settings saved successfully!");
    } catch (e) {
      setStatus(
        `Error: ${e instanceof Error ? e.message : String(e)}`
      );
    } finally {
      setSaving(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleAddNumber();
    }
  };

  return (
    <section>
      <h3 className="text-sm font-medium mb-3">Message Policy</h3>
      <div className="border border-border rounded-lg p-4 space-y-4">
        {/* DM Policy */}
        <div>
          <label className="text-xs text-muted-foreground block mb-1">
            Who can message the bot
          </label>
          <select
            value={dmPolicy}
            onChange={(e) => setDmPolicy(e.target.value)}
            className="w-full px-3 py-2 bg-muted rounded-md text-sm outline-none focus:ring-1 focus:ring-ring"
          >
            <option value="allow">Allow All</option>
            <option value="deny">Deny All</option>
            <option value="allowlist">Allowlist Only</option>
          </select>
        </div>

        {/* Allowed numbers (only show when allowlist) */}
        {dmPolicy === "allowlist" && (
          <div>
            <label className="text-xs text-muted-foreground block mb-2">
              Allowed phone numbers (E.164 format)
            </label>
            {/* Number list */}
            {allowedNumbers.length > 0 ? (
              <div className="space-y-1 mb-2">
                {allowedNumbers.map((num) => (
                  <div
                    key={num}
                    className="flex items-center justify-between bg-muted rounded-md px-3 py-1.5"
                  >
                    <span className="text-sm font-mono">{num}</span>
                    <button
                      onClick={() => handleRemoveNumber(num)}
                      className="text-muted-foreground hover:text-destructive text-xs ml-2"
                    >
                      &times;
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-xs text-muted-foreground/60 mb-2">
                No numbers added yet. The bot will not respond to anyone.
              </p>
            )}

            {/* Add number input */}
            <div className="flex gap-2">
              <input
                type="text"
                value={newNumber}
                onChange={(e) => setNewNumber(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="+1234567890"
                className="flex-1 px-3 py-1.5 bg-muted rounded-md text-sm outline-none focus:ring-1 focus:ring-ring font-mono"
              />
              <button
                onClick={handleAddNumber}
                disabled={!newNumber.trim()}
                className="px-3 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90 transition-opacity disabled:opacity-50"
              >
                Add
              </button>
            </div>
          </div>
        )}

        {/* Self-chat toggle */}
        <div className="flex items-center gap-2">
          <input
            type="checkbox"
            id="selfChatMode"
            checked={selfChatMode}
            onChange={(e) => setSelfChatMode(e.target.checked)}
            className="rounded"
          />
          <label
            htmlFor="selfChatMode"
            className="text-xs text-muted-foreground cursor-pointer"
          >
            Allow bot to respond to my own messages
          </label>
        </div>

        {/* Save button */}
        <div className="flex items-center gap-3 pt-2">
          <button
            onClick={handleSave}
            disabled={saving}
            className="px-4 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90 transition-opacity disabled:opacity-50"
          >
            {saving ? "Saving..." : "Save Policy"}
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
      </div>
    </section>
  );
}
