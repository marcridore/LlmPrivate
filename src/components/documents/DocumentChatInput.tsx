import { useState, useRef, useCallback } from "react";
import { useDocumentStore } from "../../stores/documentStore";
import { useModelStore } from "../../stores/modelStore";

export function DocumentChatInput() {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sendDocMessage = useDocumentStore((s) => s.sendDocMessage);
  const stopDocGeneration = useDocumentStore((s) => s.stopDocGeneration);
  const isDocGenerating = useDocumentStore((s) => s.isDocGenerating);
  const loadedModelHandle = useModelStore((s) => s.loadedModelHandle);

  const handleSend = useCallback(async () => {
    const trimmed = input.trim();
    if (!trimmed || isDocGenerating || !loadedModelHandle) return;

    setInput("");
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
    await sendDocMessage(trimmed, loadedModelHandle);
  }, [input, isDocGenerating, loadedModelHandle, sendDocMessage]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    if (e.key === "Escape" && isDocGenerating && loadedModelHandle) {
      stopDocGeneration(loadedModelHandle);
    }
  };

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
    const el = e.target;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 200) + "px";
  };

  return (
    <div className="border-t border-border p-3">
      <div className="flex gap-2 items-end bg-muted rounded-lg p-2">
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleInput}
          onKeyDown={handleKeyDown}
          placeholder="Ask about your documents... (Enter to send)"
          className="flex-1 bg-transparent resize-none outline-none text-sm min-h-[40px] max-h-[200px] px-2 py-1"
          rows={1}
          disabled={isDocGenerating}
        />
        {isDocGenerating ? (
          <button
            onClick={() => loadedModelHandle && stopDocGeneration(loadedModelHandle)}
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
  );
}
