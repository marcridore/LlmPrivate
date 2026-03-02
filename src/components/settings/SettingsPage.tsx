import { useState, useCallback } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";

type BackupProgressEvent =
  | { type: "CopyingDatabase" }
  | { type: "ArchivingDocuments"; current: number; total: number }
  | { type: "WritingZip" }
  | { type: "Done"; path: string; size_bytes: number }
  | { type: "Error"; message: string };

type RestoreProgressEvent =
  | { type: "Validating" }
  | { type: "ExtractingDocuments"; current: number; total: number }
  | { type: "ReplacingDatabase" }
  | { type: "RewritingPaths"; rewritten: number }
  | { type: "Done" }
  | { type: "Error"; message: string };

export function SettingsPage() {
  const [backupStatus, setBackupStatus] = useState<string | null>(null);
  const [restoreStatus, setRestoreStatus] = useState<string | null>(null);
  const [isWorking, setIsWorking] = useState(false);

  const handleExport = useCallback(async () => {
    const today = new Date().toISOString().slice(0, 10);
    const destPath = await save({
      defaultPath: `llmprivate-backup-${today}.zip`,
      filters: [{ name: "ZIP Archive", extensions: ["zip"] }],
    });
    if (!destPath) return;

    setIsWorking(true);
    setBackupStatus("Starting backup...");
    setRestoreStatus(null);

    const onProgress = new Channel<BackupProgressEvent>();
    onProgress.onmessage = (event: BackupProgressEvent) => {
      switch (event.type) {
        case "CopyingDatabase":
          setBackupStatus("Copying database...");
          break;
        case "ArchivingDocuments":
          setBackupStatus(
            `Archiving documents (${event.current}/${event.total})...`
          );
          break;
        case "WritingZip":
          setBackupStatus("Writing ZIP file...");
          break;
        case "Done":
          setBackupStatus(
            `Backup saved! (${(event.size_bytes / 1024 / 1024).toFixed(1)} MB)`
          );
          break;
        case "Error":
          setBackupStatus(`Error: ${event.message}`);
          break;
      }
    };

    try {
      await invoke("export_backup", { destPath, onProgress });
    } catch (e) {
      const msg =
        e instanceof Error
          ? e.message
          : typeof e === "object"
            ? JSON.stringify(e)
            : String(e);
      setBackupStatus(`Error: ${msg}`);
    } finally {
      setIsWorking(false);
    }
  }, []);

  const handleImport = useCallback(async () => {
    const sourcePath = await open({
      filters: [{ name: "LlmPrivate Backup", extensions: ["zip"] }],
    });
    if (!sourcePath) return;

    const confirmed = confirm(
      "Restoring a backup will REPLACE all current data (conversations, documents, settings).\n\nThis cannot be undone. Continue?"
    );
    if (!confirmed) return;

    setIsWorking(true);
    setRestoreStatus("Starting restore...");
    setBackupStatus(null);

    const onProgress = new Channel<RestoreProgressEvent>();
    onProgress.onmessage = (event: RestoreProgressEvent) => {
      switch (event.type) {
        case "Validating":
          setRestoreStatus("Validating backup...");
          break;
        case "ExtractingDocuments":
          setRestoreStatus(
            `Extracting documents (${event.current}/${event.total})...`
          );
          break;
        case "ReplacingDatabase":
          setRestoreStatus("Replacing database...");
          break;
        case "RewritingPaths":
          setRestoreStatus(
            event.rewritten > 0
              ? `Updated ${event.rewritten} document paths`
              : "No path updates needed"
          );
          break;
        case "Done":
          setRestoreStatus(
            "Restore complete! Please restart the app for all changes to take effect."
          );
          break;
        case "Error":
          setRestoreStatus(`Error: ${event.message}`);
          break;
      }
    };

    try {
      await invoke("import_backup", { sourcePath, onProgress });
    } catch (e) {
      const msg =
        e instanceof Error
          ? e.message
          : typeof e === "object"
            ? JSON.stringify(e)
            : String(e);
      setRestoreStatus(`Error: ${msg}`);
    } finally {
      setIsWorking(false);
    }
  }, []);

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="max-w-2xl">
        <h2 className="text-lg font-semibold mb-6">Settings</h2>

        {/* Backup & Restore Section */}
        <section className="mb-8">
          <h3 className="text-sm font-medium mb-1">Backup & Restore</h3>
          <p className="text-xs text-muted-foreground mb-4">
            Export all your data (conversations, documents, settings) to a
            single ZIP file, or restore from a previous backup. Models are not
            included — they can be re-downloaded on the new machine.
          </p>

          <div className="flex gap-3 mb-3">
            <button
              onClick={handleExport}
              disabled={isWorking}
              className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:opacity-90 transition-opacity disabled:opacity-50"
            >
              {isWorking && backupStatus ? "Exporting..." : "Export Backup"}
            </button>
            <button
              onClick={handleImport}
              disabled={isWorking}
              className="px-4 py-2 text-sm rounded-md border border-border hover:bg-muted transition-colors disabled:opacity-50"
            >
              {isWorking && restoreStatus
                ? "Restoring..."
                : "Import Backup"}
            </button>
          </div>

          {/* Status messages */}
          {backupStatus && (
            <div
              className={`text-xs px-3 py-2 rounded-md mb-2 ${
                backupStatus.startsWith("Error")
                  ? "bg-destructive/10 text-destructive"
                  : backupStatus.startsWith("Backup saved")
                    ? "bg-green-500/10 text-green-400"
                    : "bg-muted text-muted-foreground"
              }`}
            >
              {backupStatus}
            </div>
          )}
          {restoreStatus && (
            <div
              className={`text-xs px-3 py-2 rounded-md mb-2 ${
                restoreStatus.startsWith("Error")
                  ? "bg-destructive/10 text-destructive"
                  : restoreStatus.startsWith("Restore complete")
                    ? "bg-green-500/10 text-green-400"
                    : "bg-muted text-muted-foreground"
              }`}
            >
              {restoreStatus}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
