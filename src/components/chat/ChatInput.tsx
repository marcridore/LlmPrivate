import { useState, useRef, useCallback } from "react";
import { useChatStore } from "../../stores/chatStore";

export function ChatInput() {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const stopGeneration = useChatStore((s) => s.stopGeneration);
  const isGenerating = useChatStore((s) => s.isGenerating);

  const handleSend = useCallback(async () => {
    const trimmed = input.trim();
    if (!trimmed || isGenerating) return;

    setInput("");
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
    await sendMessage(trimmed);
  }, [input, isGenerating, sendMessage]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    if (e.key === "Escape" && isGenerating) {
      stopGeneration();
    }
  };

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
    // Auto-resize textarea
    const el = e.target;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 200) + "px";
  };

  return (
    <div className="border-t border-border p-4">
      <div className="max-w-3xl mx-auto">
        <div className="flex gap-2 items-end bg-muted rounded-lg p-2">
          <textarea
            ref={textareaRef}
            value={input}
            onChange={handleInput}
            onKeyDown={handleKeyDown}
            placeholder="Type a message... (Enter to send, Shift+Enter for newline)"
            className="flex-1 bg-transparent resize-none outline-none text-sm min-h-[40px] max-h-[200px] px-2 py-1"
            rows={1}
            disabled={isGenerating}
          />
          {isGenerating ? (
            <button
              onClick={() => stopGeneration()}
              className="px-4 py-2 bg-destructive text-destructive-foreground rounded-md text-sm font-medium hover:opacity-90 transition-opacity flex-shrink-0"
            >
              Stop
            </button>
          ) : (
            <button
              onClick={handleSend}
              disabled={!input.trim()}
              className="px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed flex-shrink-0"
            >
              Send
            </button>
          )}
        </div>
        <p className="text-xs text-muted-foreground mt-1 text-center">
          Esc to stop generation
        </p>
      </div>
    </div>
  );
}
