import { useState, useRef, useEffect } from "react";
import type { DocFolder } from "../../types/document";
import { useDocumentStore } from "../../stores/documentStore";

interface FolderTreeItemProps {
  folder: DocFolder;
  depth: number;
}

export function FolderTreeItem({ folder, depth }: FolderTreeItemProps) {
  const selectedFolderId = useDocumentStore((s) => s.selectedFolderId);
  const selectFolder = useDocumentStore((s) => s.selectFolder);
  const createFolder = useDocumentStore((s) => s.createFolder);
  const renameFolder = useDocumentStore((s) => s.renameFolder);
  const deleteFolder = useDocumentStore((s) => s.deleteFolder);

  const [expanded, setExpanded] = useState(false);
  const [showMenu, setShowMenu] = useState(false);
  const [menuPos, setMenuPos] = useState({ x: 0, y: 0 });
  const [isRenaming, setIsRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(folder.name);
  const menuRef = useRef<HTMLDivElement>(null);
  const renameRef = useRef<HTMLInputElement>(null);

  const isSelected = selectedFolderId === folder.id;
  const hasChildren = folder.children.length > 0;

  // Close context menu on click outside
  useEffect(() => {
    if (!showMenu) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setShowMenu(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showMenu]);

  // Focus rename input
  useEffect(() => {
    if (isRenaming && renameRef.current) {
      renameRef.current.focus();
      renameRef.current.select();
    }
  }, [isRenaming]);

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setMenuPos({ x: e.clientX, y: e.clientY });
    setShowMenu(true);
  };

  const handleSelect = () => {
    selectFolder(folder.id);
    if (hasChildren) setExpanded(true);
  };

  const handleAddSubfolder = async () => {
    setShowMenu(false);
    const name = prompt("New subfolder name:");
    if (name?.trim()) {
      await createFolder(name.trim(), folder.id);
      setExpanded(true);
    }
  };

  const handleRename = () => {
    setShowMenu(false);
    setRenameValue(folder.name);
    setIsRenaming(true);
  };

  const submitRename = async () => {
    setIsRenaming(false);
    const trimmed = renameValue.trim();
    if (trimmed && trimmed !== folder.name) {
      await renameFolder(folder.id, trimmed);
    }
  };

  const handleDelete = async () => {
    setShowMenu(false);
    const ok = confirm(`Delete folder "${folder.name}" and all its contents?`);
    if (ok) {
      await deleteFolder(folder.id);
    }
  };

  return (
    <div>
      <div
        onClick={handleSelect}
        onContextMenu={handleContextMenu}
        className={`flex items-center gap-1 px-2 py-1.5 text-sm cursor-pointer transition-colors rounded-sm mx-1 ${
          isSelected
            ? "bg-primary/15 text-foreground"
            : "text-muted-foreground hover:bg-muted hover:text-foreground"
        }`}
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
      >
        {/* Expand/collapse toggle */}
        <button
          onClick={(e) => {
            e.stopPropagation();
            setExpanded(!expanded);
          }}
          className="w-4 h-4 flex items-center justify-center flex-shrink-0"
        >
          {hasChildren ? (
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              className={`transition-transform ${expanded ? "rotate-90" : ""}`}
              fill="currentColor"
            >
              <path d="M3 1l5 4-5 4z" />
            </svg>
          ) : (
            <span className="w-1 h-1 rounded-full bg-current opacity-30" />
          )}
        </button>

        {/* Folder icon */}
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          className="flex-shrink-0"
        >
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
        </svg>

        {/* Folder name or rename input */}
        {isRenaming ? (
          <input
            ref={renameRef}
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
            onBlur={submitRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") submitRename();
              if (e.key === "Escape") setIsRenaming(false);
            }}
            className="flex-1 bg-background border border-border rounded px-1 text-sm outline-none"
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <span className="truncate flex-1">{folder.name}</span>
        )}

        {/* Document count badge */}
        {folder.document_count > 0 && (
          <span className="text-[10px] bg-muted text-muted-foreground rounded px-1 flex-shrink-0">
            {folder.document_count}
          </span>
        )}
      </div>

      {/* Children */}
      {expanded &&
        folder.children.map((child) => (
          <FolderTreeItem key={child.id} folder={child} depth={depth + 1} />
        ))}

      {/* Context menu */}
      {showMenu && (
        <div
          ref={menuRef}
          className="fixed z-50 bg-popover border border-border rounded-md shadow-lg py-1 min-w-[140px]"
          style={{ left: menuPos.x, top: menuPos.y }}
        >
          <button
            onClick={handleAddSubfolder}
            className="w-full text-left px-3 py-1.5 text-sm hover:bg-muted transition-colors"
          >
            Add Subfolder
          </button>
          <button
            onClick={handleRename}
            className="w-full text-left px-3 py-1.5 text-sm hover:bg-muted transition-colors"
          >
            Rename
          </button>
          <div className="border-t border-border my-1" />
          <button
            onClick={handleDelete}
            className="w-full text-left px-3 py-1.5 text-sm text-destructive hover:bg-muted transition-colors"
          >
            Delete
          </button>
        </div>
      )}
    </div>
  );
}
