import { useEffect, useRef, useCallback } from "react";
import { useChatStore } from "../../stores/chatStore";
import { useModelStore } from "../../stores/modelStore";
import { MessageBubble } from "./MessageBubble";
import { ChatInput } from "./ChatInput";
import { ModelLoader } from "./ModelLoader";

export function ChatView() {
  const messages = useChatStore((s) => s.messages);
  const isGenerating = useChatStore((s) => s.isGenerating);
  const setLoadedModelHandle = useChatStore((s) => s.setLoadedModelHandle);
  const useOpenClaw = useChatStore((s) => s.useOpenClaw);
  const loadedModelHandle = useModelStore((s) => s.loadedModelHandle);
  const supportsVision = useModelStore((s) => s.supportsVision);
  const autoLoadModel = useModelStore((s) => s.autoLoadModel);
  const scrollRef = useRef<HTMLDivElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const isStreamingRef = useRef(false);

  // Track streaming state without re-renders
  useEffect(() => {
    isStreamingRef.current = isGenerating;
  }, [isGenerating]);

  // Scroll: during streaming use instant scroll (no animation = no shake),
  // on new messages use smooth scroll
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
  }, [messages]);

  // Sync model handle to chatStore so sendMessage works
  useEffect(() => {
    setLoadedModelHandle(loadedModelHandle);
  }, [loadedModelHandle]);

  // Auto-load last used model on first visit
  useEffect(() => {
    if (!loadedModelHandle) {
      autoLoadModel();
    }
  }, []);

  // Show model loader only when using local model and no model is loaded
  if (!useOpenClaw && !loadedModelHandle) {
    return <ModelLoader />;
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Messages area */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-4 py-4">
        {messages.length === 0 && (
          <div className="h-full flex items-center justify-center">
            <div className="text-center text-muted-foreground">
              <h2 className="text-2xl font-bold mb-2">LlmPrivate</h2>
              <p className="text-sm">Start a conversation with your local AI</p>
              {supportsVision && (
                <p className="text-xs mt-3 text-pink-400">
                  Vision model loaded — attach an image using the image button below
                </p>
              )}
            </div>
          </div>
        )}

        <div className="max-w-3xl mx-auto space-y-4">
          {messages.map((msg) => (
            <MessageBubble key={msg.id} message={msg} />
          ))}
        </div>
        <div ref={messagesEndRef} />
      </div>

      {/* Input area */}
      <ChatInput />
    </div>
  );
}
