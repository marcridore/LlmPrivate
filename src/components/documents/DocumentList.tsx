import { useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useDocumentStore } from "../../stores/documentStore";
import { useModelStore } from "../../stores/modelStore";
import { DocumentCard } from "./DocumentCard";
import type { DocumentChatMode } from "../../types/document";

export function DocumentList() {
  const documents = useDocumentStore((s) => s.documents);
  const selectedFolderId = useDocumentStore((s) => s.selectedFolderId);
  const selectedDocumentIds = useDocumentStore((s) => s.selectedDocumentIds);
  const toggleDocumentSelection = useDocumentStore((s) => s.toggleDocumentSelection);
  const selectAllDocuments = useDocumentStore((s) => s.selectAllDocuments);
  const clearDocumentSelection = useDocumentStore((s) => s.clearDocumentSelection);
  const addDocument = useDocumentStore((s) => s.addDocument);
  const deleteDocument = useDocumentStore((s) => s.deleteDocument);
  const startDocChat = useDocumentStore((s) => s.startDocChat);
  const addDocumentProgress = useDocumentStore((s) => s.addDocumentProgress);
  const addDocumentError = useDocumentStore((s) => s.addDocumentError);
  const loadedModelHandle = useModelStore((s) => s.loadedModelHandle);

  const handleAddDocument = useCallback(async () => {
    if (!selectedFolderId) return;

    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "Documents",
          extensions: ["pdf", "txt", "md", "docx"],
        },
      ],
    });

    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    for (const filePath of paths) {
      await addDocument(selectedFolderId, filePath);
    }
  }, [selectedFolderId, addDocument]);

  const handleDeleteDocument = useCallback(
    async (docId: string) => {
      const doc = documents.find((d) => d.id === docId);
      const ok = confirm(`Delete "${doc?.filename}"?`);
      if (ok) await deleteDocument(docId);
    },
    [documents, deleteDocument]
  );

  const handleStartMode = useCallback(
    (mode: DocumentChatMode) => {
      if (selectedDocumentIds.length === 0) return;
      startDocChat(selectedDocumentIds, mode);
    },
    [selectedDocumentIds, startDocChat]
  );

  const hasSelection = selectedDocumentIds.length > 0;
  const allSelected =
    documents.length > 0 && selectedDocumentIds.length === documents.length;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Toolbar */}
      <div className="flex items-center justify-between p-3 border-b border-border gap-2 flex-wrap">
        <div className="flex items-center gap-2">
          <button
            onClick={handleAddDocument}
            disabled={addDocumentProgress !== null}
            className="px-3 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:opacity-90 transition-opacity disabled:opacity-50"
          >
            {addDocumentProgress ?? "+ Add Document"}
          </button>

          {documents.length > 0 && (
            <button
              onClick={allSelected ? clearDocumentSelection : selectAllDocuments}
              className="px-2 py-1.5 text-xs rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
            >
              {allSelected ? "Deselect All" : "Select All"}
            </button>
          )}
        </div>

        {/* Action buttons — visible when documents are selected */}
        {hasSelection && (
          <div className="flex items-center gap-1.5">
            <span className="text-xs text-muted-foreground mr-1">
              {selectedDocumentIds.length} selected:
            </span>
            <ActionButton
              label="Chat"
              disabled={!loadedModelHandle}
              onClick={() => handleStartMode("chat")}
              title={!loadedModelHandle ? "Load a model first" : "Chat with selected documents"}
            />
            <ActionButton
              label="Summarize"
              disabled={!loadedModelHandle}
              onClick={() => handleStartMode("summarize")}
              title={!loadedModelHandle ? "Load a model first" : "Summarize selected documents"}
            />
            <ActionButton
              label="Quiz"
              disabled={!loadedModelHandle}
              onClick={() => handleStartMode("quiz")}
              title={!loadedModelHandle ? "Load a model first" : "Generate quiz from selected documents"}
            />
          </div>
        )}
      </div>

      {/* Error banner */}
      {addDocumentError && (
        <div className="mx-3 mt-2 px-3 py-2 text-xs text-destructive bg-destructive/10 rounded-md">
          {addDocumentError}
        </div>
      )}

      {/* Document list */}
      <div className="flex-1 overflow-y-auto p-3">
        {documents.length === 0 ? (
          <div className="h-full flex items-center justify-center">
            <div className="text-center text-muted-foreground">
              <svg
                width="48"
                height="48"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="1"
                className="mx-auto mb-3 opacity-30"
              >
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                <polyline points="14 2 14 8 20 8" />
              </svg>
              <p className="text-sm mb-1">No documents in this folder</p>
              <p className="text-xs">
                Click "Add Document" to import PDF, TXT, MD, or DOCX files
              </p>
            </div>
          </div>
        ) : (
          <div className="space-y-2 max-w-3xl">
            {documents.map((doc) => (
              <DocumentCard
                key={doc.id}
                document={doc}
                selected={selectedDocumentIds.includes(doc.id)}
                onToggle={() => toggleDocumentSelection(doc.id)}
                onDelete={() => handleDeleteDocument(doc.id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function ActionButton({
  label,
  disabled,
  onClick,
  title,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  title: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className="px-2.5 py-1.5 text-xs rounded-md border border-border hover:bg-muted transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
    >
      {label}
    </button>
  );
}
