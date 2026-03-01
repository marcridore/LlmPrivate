import { useState } from "react";
import type { Message } from "../../types/chat";
import { MarkdownRenderer } from "./MarkdownRenderer";

interface MessageBubbleProps {
  message: Message;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const [copied, setCopied] = useState(false);
  const isUser = message.role === "user";
  const isSystem = message.role === "system";

  const handleCopy = async () => {
    await navigator.clipboard.writeText(message.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (isSystem) {
    return (
      <div className="text-xs text-muted-foreground italic px-4 py-1">
        System: {message.content}
      </div>
    );
  }

  return (
    <div className={`flex gap-3 group ${isUser ? "justify-end" : ""}`}>
      {!isUser && (
        <div className="w-7 h-7 rounded-full bg-primary flex items-center justify-center text-primary-foreground text-xs font-bold flex-shrink-0 mt-1">
          AI
        </div>
      )}

      <div
        className={`max-w-[80%] rounded-lg px-4 py-2 ${
          isUser
            ? "bg-primary text-primary-foreground"
            : "bg-muted"
        }`}
      >
        {/* Attached images */}
        {message.images && message.images.length > 0 && (
          <div className="flex gap-2 flex-wrap mb-2">
            {message.images.map((img) => (
              <img
                key={img.id}
                src={img.previewUrl}
                alt={img.altText ?? "Attached image"}
                className="max-w-[200px] max-h-[200px] rounded-md object-cover"
              />
            ))}
          </div>
        )}

        {isUser ? (
          <p className="text-sm whitespace-pre-wrap">{message.content}</p>
        ) : (
          <div className="text-sm prose prose-sm dark:prose-invert max-w-none">
            <MarkdownRenderer content={message.content} />
            {message.isStreaming && (
              <span className="inline-block w-2 h-4 bg-foreground animate-pulse ml-0.5" />
            )}
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="opacity-0 group-hover:opacity-100 transition-opacity flex items-start gap-1 mt-1">
        <button
          onClick={handleCopy}
          className="text-xs text-muted-foreground hover:text-foreground p-1"
          title="Copy"
        >
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
    </div>
  );
}
