import { useEffect, useState } from "react";
import { useDocumentStore } from "../../stores/documentStore";
import type { DocChatSession } from "../../types/document";

const MODE_COLORS: Record<string, string> = {
  chat: "bg-blue-500/15 text-blue-400",
  summarize: "bg-green-500/15 text-green-400",
  quiz: "bg-orange-500/15 text-orange-400",
};

const MODE_LABELS: Record<string, string> = {
  chat: "Chat",
  summarize: "Sum",
  quiz: "Quiz",
};

function formatRelativeTime(dateStr: string): string {
  const now = Date.now();
  const date = new Date(dateStr).getTime();
  const diffMs = now - date;
  const diffMin = Math.floor(diffMs / 60_000);

  if (diffMin < 1) return "now";
  if (diffMin < 60) return `${diffMin}m`;
  const diffHrs = Math.floor(diffMin / 60);
  if (diffHrs < 24) return `${diffHrs}h`;
  const diffDays = Math.floor(diffHrs / 24);
  if (diffDays < 7) return `${diffDays}d`;
  return `${Math.floor(diffDays / 7)}w`;
}

export function RecentDocChats() {
  const recentDocChats = useDocumentStore((s) => s.recentDocChats);
  const loadRecentDocChats = useDocumentStore((s) => s.loadRecentDocChats);
  const resumeDocChat = useDocumentStore((s) => s.resumeDocChat);
  const toggleDocChatPin = useDocumentStore((s) => s.toggleDocChatPin);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    loadRecentDocChats();
  }, [loadRecentDocChats]);

  if (recentDocChats.length === 0) return null;

  return (
    <div className="border-b border-border">
      <button
        onClick={() => setExpanded((o) => !o)}
        className="flex items-center justify-between w-full px-3 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
      >
        <span>Recent Chats ({recentDocChats.length})</span>
        <svg
          width="12"
          height="12"
          viewBox="0 0 12 12"
          className={`transition-transform ${expanded ? "rotate-180" : ""}`}
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
        >
          <path d="M3 5l3 3 3-3" />
        </svg>
      </button>

      {expanded && (
        <div className="max-h-48 overflow-y-auto">
          {recentDocChats.map((session) => (
            <SessionRow
              key={session.conversation_id}
              session={session}
              onResume={() => resumeDocChat(session)}
              onTogglePin={() => toggleDocChatPin(session.conversation_id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function SessionRow({
  session,
  onResume,
  onTogglePin,
}: {
  session: DocChatSession;
  onResume: () => void;
  onTogglePin: () => void;
}) {
  const modeColor = MODE_COLORS[session.mode] ?? "bg-muted text-muted-foreground";
  const modeLabel = MODE_LABELS[session.mode] ?? session.mode;

  return (
    <div
      onClick={onResume}
      className="group px-3 py-2 text-sm cursor-pointer hover:bg-muted transition-colors flex items-center gap-2"
    >
      {/* Pin icon */}
      <button
        onClick={(e) => {
          e.stopPropagation();
          onTogglePin();
        }}
        className={`p-0.5 transition-opacity flex-shrink-0 ${
          session.pinned
            ? "opacity-100 text-primary"
            : "opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-foreground"
        }`}
        title={session.pinned ? "Unpin" : "Pin"}
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill={session.pinned ? "currentColor" : "none"}
          stroke="currentColor"
          strokeWidth="2"
        >
          <path d="M12 17v5M9 2h6l-1 7h4l-8 8-2-8H4l5-7z" />
        </svg>
      </button>

      {/* Mode badge */}
      <span
        className={`text-[10px] font-bold uppercase px-1 py-0.5 rounded flex-shrink-0 ${modeColor}`}
      >
        {modeLabel}
      </span>

      {/* Document names */}
      <span className="truncate flex-1 text-foreground">
        {session.document_names.join(", ")}
      </span>

      {/* Relative time */}
      <span className="text-xs text-muted-foreground/50 flex-shrink-0">
        {formatRelativeTime(session.updated_at)}
      </span>
    </div>
  );
}
