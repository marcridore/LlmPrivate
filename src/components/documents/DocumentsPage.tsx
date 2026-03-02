import { useDocumentStore } from "../../stores/documentStore";
import { FolderPanel } from "./FolderPanel";
import { DocumentList } from "./DocumentList";
import { DocumentChat } from "./DocumentChat";
import { RecentDocChats } from "./RecentDocChats";

export function DocumentsPage() {
  const selectedFolderId = useDocumentStore((s) => s.selectedFolderId);
  const isDocChatActive = useDocumentStore((s) => s.isDocChatActive);

  return (
    <div className="flex-1 flex overflow-hidden">
      {/* Left panel: folder tree */}
      <FolderPanel />

      {/* Right panel: document list or document chat */}
      {isDocChatActive ? (
        <DocumentChat />
      ) : (
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Recent chats section (collapsible) */}
          <RecentDocChats />

          {/* Document list or empty state */}
          {selectedFolderId ? (
            <DocumentList />
          ) : (
            <div className="flex-1 flex items-center justify-center text-muted-foreground">
              <div className="text-center">
                <svg
                  width="48"
                  height="48"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1"
                  className="mx-auto mb-3 opacity-30"
                >
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                </svg>
                <p className="text-sm mb-1">Select a folder</p>
                <p className="text-xs">
                  Choose a folder from the left panel to view documents
                </p>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
