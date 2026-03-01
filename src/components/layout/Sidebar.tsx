import { useEffect } from "react";
import { useChatStore } from "../../stores/chatStore";
import { useUIStore } from "../../stores/uiStore";
import { useModelStore } from "../../stores/modelStore";

export function Sidebar() {
  const conversations = useChatStore((s) => s.conversations);
  const activeConversationId = useChatStore((s) => s.activeConversationId);
  const loadConversations = useChatStore((s) => s.loadConversations);
  const selectConversation = useChatStore((s) => s.selectConversation);
  const createConversation = useChatStore((s) => s.createConversation);
  const deleteConversation = useChatStore((s) => s.deleteConversation);
  const activePage = useUIStore((s) => s.activePage);
  const setActivePage = useUIStore((s) => s.setActivePage);
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);
  const loadedModelName = useModelStore((s) => s.loadedModelName);

  useEffect(() => {
    loadConversations();
  }, [loadConversations]);

  if (sidebarCollapsed) {
    return (
      <div className="w-12 border-r border-border bg-card flex flex-col items-center py-2 gap-2">
        <NavIcon
          icon="chat"
          active={activePage === "chat"}
          onClick={() => setActivePage("chat")}
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
      <div className="flex gap-1 p-2 border-b border-border">
        {(["chat", "models", "monitor", "settings"] as const).map((page) => (
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

      {/* New Chat button */}
      {activePage === "chat" && (
        <div className="p-2">
          <button
            onClick={() => createConversation()}
            className="w-full py-2 px-3 text-sm rounded-md border border-border hover:bg-muted transition-colors flex items-center gap-2"
          >
            <span>+</span>
            <span>New Chat</span>
          </button>
        </div>
      )}

      {/* Conversation list */}
      {activePage === "chat" && (
        <div className="flex-1 overflow-y-auto">
          {conversations.map((conv) => (
            <div
              key={conv.id}
              onClick={() => selectConversation(conv.id)}
              className={`group px-3 py-2 text-sm cursor-pointer flex items-center justify-between hover:bg-muted transition-colors ${
                activeConversationId === conv.id ? "bg-muted" : ""
              }`}
            >
              <span className="truncate">{conv.title}</span>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  deleteConversation(conv.id);
                }}
                className="hidden group-hover:block text-muted-foreground hover:text-destructive text-xs"
              >
                x
              </button>
            </div>
          ))}
        </div>
      )}

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
