import type { DocumentSummary } from "../../types/document";

interface DocumentCardProps {
  document: DocumentSummary;
  selected: boolean;
  onToggle: () => void;
  onDelete: () => void;
}

const FILE_TYPE_ICONS: Record<string, string> = {
  pdf: "PDF",
  txt: "TXT",
  md: "MD",
  docx: "DOC",
};

const FILE_TYPE_COLORS: Record<string, string> = {
  pdf: "bg-red-500/15 text-red-400",
  txt: "bg-blue-500/15 text-blue-400",
  md: "bg-green-500/15 text-green-400",
  docx: "bg-purple-500/15 text-purple-400",
};

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function DocumentCard({
  document,
  selected,
  onToggle,
  onDelete,
}: DocumentCardProps) {
  const typeLabel = FILE_TYPE_ICONS[document.file_type] ?? document.file_type.toUpperCase();
  const typeColor = FILE_TYPE_COLORS[document.file_type] ?? "bg-muted text-muted-foreground";

  return (
    <div
      className={`group border rounded-lg p-3 transition-colors cursor-pointer ${
        selected
          ? "border-primary bg-primary/5"
          : "border-border hover:border-muted-foreground/30"
      }`}
      onClick={onToggle}
    >
      <div className="flex items-start gap-3">
        {/* Checkbox */}
        <div className="pt-0.5">
          <div
            className={`w-4 h-4 rounded border-2 flex items-center justify-center transition-colors ${
              selected
                ? "bg-primary border-primary"
                : "border-muted-foreground/40"
            }`}
          >
            {selected && (
              <svg
                width="10"
                height="10"
                viewBox="0 0 24 24"
                fill="none"
                stroke="white"
                strokeWidth="3"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            )}
          </div>
        </div>

        {/* File type badge */}
        <div
          className={`px-2 py-1 rounded text-[10px] font-bold flex-shrink-0 ${typeColor}`}
        >
          {typeLabel}
        </div>

        {/* Info */}
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium truncate">{document.filename}</p>
          <div className="flex items-center gap-2 mt-1 text-xs text-muted-foreground">
            <span>{formatFileSize(document.file_size)}</span>
            <span className="text-muted-foreground/30">|</span>
            <span>{document.chunk_count} chunks</span>
          </div>
        </div>

        {/* Delete button */}
        <button
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
          className="opacity-0 group-hover:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-destructive"
          title="Delete document"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <polyline points="3 6 5 6 21 6" />
            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
          </svg>
        </button>
      </div>
    </div>
  );
}
