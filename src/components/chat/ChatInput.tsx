import { useState, useRef, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { readFile } from "@tauri-apps/plugin-fs";
import { useChatStore } from "../../stores/chatStore";
import { useModelStore } from "../../stores/modelStore";
import type { ImageAttachment } from "../../types/chat";

export function ChatInput() {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const stopGeneration = useChatStore((s) => s.stopGeneration);
  const isGenerating = useChatStore((s) => s.isGenerating);
  const pendingImages = useChatStore((s) => s.pendingImages);
  const addPendingImage = useChatStore((s) => s.addPendingImage);
  const removePendingImage = useChatStore((s) => s.removePendingImage);
  const supportsVision = useModelStore((s) => s.supportsVision);

  const handleSend = useCallback(async () => {
    const trimmed = input.trim();
    if ((!trimmed && pendingImages.length === 0) || isGenerating) return;

    setInput("");
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
    await sendMessage(trimmed);
  }, [input, isGenerating, sendMessage, pendingImages.length]);

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
    const el = e.target;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 200) + "px";
  };

  const handleAttachImage = async () => {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "Images",
          extensions: ["jpg", "jpeg", "png", "bmp", "gif", "webp"],
        },
      ],
    });

    if (!selected) return;

    const paths = Array.isArray(selected) ? selected : [selected];
    for (const filePath of paths) {
      try {
        const bytes = await readFile(filePath);
        const ext = filePath.split(".").pop()?.toLowerCase() ?? "png";
        const mimeMap: Record<string, string> = {
          jpg: "image/jpeg",
          jpeg: "image/jpeg",
          png: "image/png",
          bmp: "image/bmp",
          gif: "image/gif",
          webp: "image/webp",
        };
        const mime = mimeMap[ext] ?? "image/png";
        const blob = new Blob([bytes], { type: mime });
        const previewUrl = URL.createObjectURL(blob);

        const attachment: ImageAttachment = {
          id: crypto.randomUUID(),
          filePath,
          previewUrl,
        };
        addPendingImage(attachment);
      } catch (e) {
        console.error("Failed to load image:", e);
      }
    }
  };

  const handlePaste = async (e: React.ClipboardEvent) => {
    if (!supportsVision) return;

    const items = e.clipboardData.items;
    for (const item of items) {
      if (item.type.startsWith("image/")) {
        e.preventDefault();
        const file = item.getAsFile();
        if (!file) continue;

        const previewUrl = URL.createObjectURL(file);
        const attachment: ImageAttachment = {
          id: crypto.randomUUID(),
          filePath: "", // Pasted images don't have a file path
          previewUrl,
        };
        addPendingImage(attachment);
      }
    }
  };

  return (
    <div className="border-t border-border p-4">
      <div className="max-w-3xl mx-auto">
        {/* Pending image previews */}
        {pendingImages.length > 0 && (
          <div className="flex gap-2 mb-2 flex-wrap">
            {pendingImages.map((img) => (
              <div key={img.id} className="relative group">
                <img
                  src={img.previewUrl}
                  alt={img.altText ?? "Attached image"}
                  className="w-16 h-16 object-cover rounded-md border border-border"
                />
                <button
                  onClick={() => {
                    URL.revokeObjectURL(img.previewUrl);
                    removePendingImage(img.id);
                  }}
                  className="absolute -top-1.5 -right-1.5 w-5 h-5 bg-destructive text-destructive-foreground rounded-full text-xs flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                >
                  x
                </button>
              </div>
            ))}
          </div>
        )}

        <div className="flex gap-2 items-end bg-muted rounded-lg p-2">
          {/* Image attach button — only when model supports vision */}
          {supportsVision && (
            <button
              onClick={handleAttachImage}
              disabled={isGenerating}
              className="p-2 text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50 flex-shrink-0"
              title="Attach image"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <rect width="18" height="18" x="3" y="3" rx="2" ry="2" />
                <circle cx="9" cy="9" r="2" />
                <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" />
              </svg>
            </button>
          )}

          <textarea
            ref={textareaRef}
            value={input}
            onChange={handleInput}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
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
              disabled={!input.trim() && pendingImages.length === 0}
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
