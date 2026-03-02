import { useEffect } from "react";
import { useDocumentStore } from "../../stores/documentStore";
import { FolderTreeItem } from "./FolderTreeItem";

export function FolderPanel() {
  const folders = useDocumentStore((s) => s.folders);
  const loadFolderTree = useDocumentStore((s) => s.loadFolderTree);
  const createFolder = useDocumentStore((s) => s.createFolder);

  useEffect(() => {
    loadFolderTree();
  }, [loadFolderTree]);

  const handleAddRootFolder = async () => {
    const name = prompt("New folder name:");
    if (name?.trim()) {
      await createFolder(name.trim());
    }
  };

  return (
    <div className="w-60 border-r border-border bg-card flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b border-border">
        <h3 className="text-sm font-semibold">Folders</h3>
        <button
          onClick={handleAddRootFolder}
          className="w-6 h-6 rounded flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          title="New folder"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
        </button>
      </div>

      {/* Tree */}
      <div className="flex-1 overflow-y-auto py-1">
        {folders.length === 0 ? (
          <div className="px-3 py-8 text-center">
            <p className="text-xs text-muted-foreground mb-2">
              No folders yet
            </p>
            <button
              onClick={handleAddRootFolder}
              className="text-xs text-primary hover:underline"
            >
              Create your first folder
            </button>
          </div>
        ) : (
          folders.map((folder) => (
            <FolderTreeItem key={folder.id} folder={folder} depth={0} />
          ))
        )}
      </div>
    </div>
  );
}
