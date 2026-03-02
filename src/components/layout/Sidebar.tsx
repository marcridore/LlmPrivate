import { useEffect, useRef, useCallback, useState } from "react";
import { useChatStore } from "../../stores/chatStore";
import { useUIStore } from "../../stores/uiStore";
import { useModelStore } from "../../stores/modelStore";

export function Sidebar() {
  const conversations = useChatStore((s) => s.conversations);
  const activeConversationId = useChatStore((s) => s.activeConversationId);
  const initConversations = useChatStore((s) => s.initConversations);
  const loadMoreConversations = useChatStore((s) => s.loadMoreConversations);
  const hasMoreConversations = useChatStore((s) => s.hasMoreConversations);
  const selectConversation = useChatStore((s) => s.selectConversation);
  const createConversation = useChatStore((s) => s.createConversation);
  const deleteConversation = useChatStore((s) => s.deleteConversation);
  const activePage = useUIStore((s) => s.activePage);
  const setActivePage = useUIStore((s) => s.setActivePage);
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);
  const loadedModelName = useModelStore((s) => s.loadedModelName);

  const [historyOpen, setHistoryOpen] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    initConversations();
  }, [initConversations]);

  // Infinite scroll: load more when user scrolls near bottom
  const handleScroll = useCallback(() => {
    const el = listRef.current;
    if (!el || !hasMoreConversations) return;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 40) {
      loadMoreConversations();
    }
  }, [hasMoreConversations, loadMoreConversations]);

  if (sidebarCollapsed) {
    return (
      <div className="w-12 border-r border-border bg-card flex flex-col items-center py-2 gap-2">
        <NavIcon
          icon="chat"
          active={activePage === "chat"}
          onClick={() => setActivePage("chat")}
        />
        <NavIcon
          icon="docs"
          active={activePage === "documents"}
          onClick={() => setActivePage("documents")}
        />
        <NavIcon
          icon="model"
          active={activePage === "models"}
          onClick={() => setActivePage("models")}
        />
        <NavIcon
          icon="monitor"
          active={activePage === "monitor"}
          onClick={() => setActivePage("monitor")}
        />
        <NavIcon
          icon="settings"
          active={activePage === "settings"}
          onClick={() => setActivePage("settings")}
        />
      </div>
    );
  }

  return (
    <div className="w-64 border-r border-border bg-card flex flex-col h-full">
      {/* Nav links */}
      <div className="flex flex-wrap gap-1 p-2 border-b border-border">
        {(["chat", "documents", "models", "monitor", "settings"] as const).map((page) => (
          <button
            key={page}
            onClick={() => setActivePage(page)}
            className={`px-2 py-1 text-xs rounded capitalize transition-colors ${
              activePage === page
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-muted"
            }`}
          >
            {page}
          </button>
        ))}
      </div>

      {activePage === "chat" && (
        <>
          {/* New Chat button */}
          <div className="p-2">
            <button
              onClick={() => {
                createConversation();
                setHistoryOpen(false);
              }}
              className="w-full py-2 px-3 text-sm rounded-md border border-border hover:bg-muted transition-colors flex items-center gap-2"
            >
              <span>+</span>
              <span>New Chat</span>
            </button>
          </div>

          {/* History toggle */}
          {conversations.length > 0 && (
            <button
              onClick={() => setHistoryOpen((o) => !o)}
              className="flex items-center justify-between px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              <span>History ({conversations.length}{hasMoreConversations ? "+" : ""})</span>
              <svg
                width="12"
                height="12"
                viewBox="0 0 12 12"
                className={`transition-transform ${historyOpen ? "rotate-180" : ""}`}
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
              >
                <path d="M3 5l3 3 3-3" />
              </svg>
            </button>
          )}

          {/* Conversation list (collapsible) */}
          {historyOpen && (
            <div
              ref={listRef}
              onScroll={handleScroll}
              className="flex-1 overflow-y-auto"
            >
              {conversations.map((conv) => (
                <div
                  key={conv.id}
                  onClick={() => selectConversation(conv.id)}
                  className={`group px-3 py-2 text-sm cursor-pointer flex items-center justify-between hover:bg-muted transition-colors ${
                    activeConversationId === conv.id ? "bg-muted" : ""
                  }`}
                >
                  <span className="truncate flex-1 mr-2">{conv.title}</span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      deleteConversation(conv.id);
                    }}
                    className="hidden group-hover:block text-muted-foreground hover:text-destructive text-xs shrink-0"
                  >
                    x
                  </button>
                </div>
              ))}
              {hasMoreConversations && (
                <button
                  onClick={() => loadMoreConversations()}
                  className="w-full py-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
                >
                  Load more...
                </button>
              )}
            </div>
          )}
        </>
      )}

      {/* Spacer to push model badge to bottom when history is closed */}
      {(activePage !== "chat" || !historyOpen) && <div className="flex-1" />}

      {/* Loaded model badge */}
      <div className="p-2 border-t border-border">
        <div className="text-xs text-muted-foreground">
          {loadedModelName ? (
            <span className="flex items-center gap-1">
              <span className="w-2 h-2 rounded-full bg-green-500" />
              {loadedModelName}
            </span>
          ) : (
            <span className="flex items-center gap-1">
              <span className="w-2 h-2 rounded-full bg-muted-foreground" />
              No model loaded
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

function NavIcon({
  icon,
  active,
  onClick,
}: {
  icon: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={`w-8 h-8 rounded flex items-center justify-center text-xs transition-colors ${
        active ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:bg-muted"
      }`}
      title={icon}
    >
      {icon[0].toUpperCase()}
    </button>
  );
}
