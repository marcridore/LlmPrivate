import { useEffect, useRef, useCallback } from "react";
import { useDocumentStore } from "../../stores/documentStore";
import { useModelStore } from "../../stores/modelStore";
import { MessageBubble } from "../chat/MessageBubble";
import { DocumentChatInput } from "./DocumentChatInput";

export function DocumentChat() {
  const docChatMessages = useDocumentStore((s) => s.docChatMessages);
  const docChatMode = useDocumentStore((s) => s.docChatMode);
  const isDocGenerating = useDocumentStore((s) => s.isDocGenerating);
  const docTokensPerSecond = useDocumentStore((s) => s.docTokensPerSecond);
  const closeDocChat = useDocumentStore((s) => s.closeDocChat);
  const sendDocMessage = useDocumentStore((s) => s.sendDocMessage);
  const selectedDocumentIds = useDocumentStore((s) => s.selectedDocumentIds);
  const documents = useDocumentStore((s) => s.documents);
  const docConversationId = useDocumentStore((s) => s.docConversationId);
  const toggleDocChatPin = useDocumentStore((s) => s.toggleDocChatPin);
  const recentDocChats = useDocumentStore((s) => s.recentDocChats);
  const loadedModelHandle = useModelStore((s) => s.loadedModelHandle);

  const scrollRef = useRef<HTMLDivElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const isStreamingRef = useRef(false);
  const autoSentRef = useRef(false);

  // Track streaming state
  useEffect(() => {
    isStreamingRef.current = isDocGenerating;
  }, [isDocGenerating]);

  // Auto-scroll
  const scrollToBottom = useCallback(() => {
    const container = scrollRef.current;
    if (!container) return;
    if (isStreamingRef.current) {
      container.scrollTop = container.scrollHeight;
    } else {
      messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [docChatMessages]);

  // Auto-send for summarize/quiz modes (skip if resuming a session with existing messages)
  useEffect(() => {
    if (autoSentRef.current) return;
    if (!loadedModelHandle) return;
    if (docChatMessages.length > 0) return; // Resuming — don't re-send

    if (docChatMode === "summarize") {
      autoSentRef.current = true;
      sendDocMessage("Please summarize this document.", loadedModelHandle);
    } else if (docChatMode === "quiz") {
      autoSentRef.current = true;
      sendDocMessage(
        "Generate 5 multiple-choice questions with answer key from this document.",
        loadedModelHandle
      );
    }
  }, [docChatMode, loadedModelHandle, sendDocMessage, docChatMessages.length]);

  const selectedDocNames = documents
    .filter((d) => selectedDocumentIds.includes(d.id))
    .map((d) => d.filename);

  const modeLabel =
    docChatMode === "chat"
      ? "Chat"
      : docChatMode === "summarize"
        ? "Summary"
        : "Quiz";

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-3 p-3 border-b border-border">
        <button
          onClick={closeDocChat}
          className="p-1 rounded hover:bg-muted transition-colors text-muted-foreground hover:text-foreground"
          title="Back to documents"
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <polyline points="15 18 9 12 15 6" />
          </svg>
        </button>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span
              className={`text-[10px] font-bold uppercase px-1.5 py-0.5 rounded ${
                docChatMode === "chat"
                  ? "bg-blue-500/15 text-blue-400"
                  : docChatMode === "summarize"
                    ? "bg-green-500/15 text-green-400"
                    : "bg-orange-500/15 text-orange-400"
              }`}
            >
              {modeLabel}
            </span>
            <span className="text-sm font-medium truncate">
              {selectedDocNames.join(", ")}
            </span>
          </div>
        </div>

        {/* Tokens/s indicator */}
        {docTokensPerSecond > 0 && !isDocGenerating && (
          <span className="text-xs text-muted-foreground">
            {docTokensPerSecond.toFixed(1)} tok/s
          </span>
        )}

        {/* Pin button */}
        {docConversationId && (() => {
          const isPinned = recentDocChats.some(
            (c) => c.conversation_id === docConversationId && c.pinned
          );
          return (
            <button
              onClick={() => toggleDocChatPin(docConversationId)}
              className={`p-1 rounded transition-colors ${
                isPinned
                  ? "text-primary hover:bg-muted"
                  : "text-muted-foreground hover:text-foreground hover:bg-muted"
              }`}
              title={isPinned ? "Unpin chat" : "Pin chat"}
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill={isPinned ? "currentColor" : "none"}
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="M12 17v5M9 2h6l-1 7h4l-8 8-2-8H4l5-7z" />
              </svg>
            </button>
          );
        })()}
      </div>

      {/* Messages area */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-4 py-4">
        {docChatMessages.length === 0 && (
          <div className="h-full flex items-center justify-center">
            <div className="text-center text-muted-foreground">
              <p className="text-sm">
                {docChatMode === "chat"
                  ? "Ask questions about your documents"
                  : docChatMode === "summarize"
                    ? "Generating summary..."
                    : "Generating quiz..."}
              </p>
            </div>
          </div>
        )}

        <div className="max-w-3xl mx-auto space-y-4">
          {docChatMessages.map((msg) => (
            <MessageBubble key={msg.id} message={msg} />
          ))}
        </div>
        <div ref={messagesEndRef} />
      </div>

      {/* Input — always shown so user can continue after summary/quiz */}
      <DocumentChatInput />
    </div>
  );
}
