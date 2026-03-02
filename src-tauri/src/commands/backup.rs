use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::error::AppError;
use crate::state::AppState;

// ═══════════════════════════════════════════════════════════════
// Progress event types
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum BackupProgress {
    CopyingDatabase,
    ArchivingDocuments { current: u32, total: u32 },
    WritingZip,
    Done { path: String, size_bytes: u64 },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum RestoreProgress {
    Validating,
    ExtractingDocuments { current: u32, total: u32 },
    ReplacingDatabase,
    RewritingPaths { rewritten: u64 },
    Done,
    Error { message: String },
}

// ═══════════════════════════════════════════════════════════════
// Backup manifest (stored inside the ZIP)
// ═══════════════════════════════════════════════════════════════

#[derive(Serialize, Deserialize)]
struct BackupManifest {
    /// Schema version for forward compatibility
    version: String,
    /// App version at time of backup
    app_version: String,
    /// ISO 8601 timestamp
    created_at: String,
    /// Original data directory (for path rewriting on restore)
    source_data_dir: String,
}

// ═══════════════════════════════════════════════════════════════
// Export
// ═══════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn export_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    dest_path: String,
    on_progress: Channel<BackupProgress>,
) -> Result<String, AppError> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Backup(format!("Cannot resolve app data dir: {e}")))?;
    let docs_dir = data_dir.join("documents");
    let dest = PathBuf::from(&dest_path);

    // Step 1: VACUUM INTO a temp copy of the database (consistent snapshot)
    let _ = on_progress.send(BackupProgress::CopyingDatabase);
    let temp_db = data_dir.join("llmprivate_backup_temp.db");
    if temp_db.exists() {
        let _ = std::fs::remove_file(&temp_db);
    }
    state.db.vacuum_into(&temp_db)?;
    tracing::info!("Backup: database snapshot created");

    // Step 2: Build ZIP archive in a blocking thread
    let data_dir_str = data_dir.to_string_lossy().to_string();
    let progress = on_progress.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<(String, u64), AppError> {
        use zip::write::SimpleFileOptions;

        let file = std::fs::File::create(&dest)
            .map_err(|e| AppError::Backup(format!("Cannot create ZIP file: {e}")))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Write manifest
        let manifest = BackupManifest {
            version: "1".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            source_data_dir: data_dir_str,
        };
        zip.start_file("manifest.json", options)
            .map_err(|e| AppError::Backup(format!("ZIP write error: {e}")))?;
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| AppError::Backup(format!("JSON serialize error: {e}")))?;
        zip.write_all(manifest_json.as_bytes())?;

        // Write database snapshot
        zip.start_file("llmprivate.db", options)
            .map_err(|e| AppError::Backup(format!("ZIP write error: {e}")))?;
        let db_bytes = std::fs::read(&temp_db)?;
        zip.write_all(&db_bytes)?;

        // Write documents
        if docs_dir.exists() {
            let entries: Vec<_> = std::fs::read_dir(&docs_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .collect();
            let total = entries.len() as u32;

            for (i, entry) in entries.iter().enumerate() {
                let fname = entry.file_name();
                let archive_path = format!("documents/{}", fname.to_string_lossy());
                zip.start_file(&archive_path, options)
                    .map_err(|e| AppError::Backup(format!("ZIP write error: {e}")))?;
                let bytes = std::fs::read(entry.path())?;
                zip.write_all(&bytes)?;
                let _ = progress.send(BackupProgress::ArchivingDocuments {
                    current: (i + 1) as u32,
                    total,
                });
            }
        }

        let _ = progress.send(BackupProgress::WritingZip);
        zip.finish()
            .map_err(|e| AppError::Backup(format!("ZIP finalize error: {e}")))?;

        // Clean up temp DB
        let _ = std::fs::remove_file(&temp_db);

        let size = std::fs::metadata(&dest)
            .map(|m| m.len())
            .unwrap_or(0);
        let path_str = dest.to_string_lossy().to_string();
        Ok((path_str, size))
    })
    .await
    .map_err(|e| AppError::Backup(format!("Backup task failed: {e}")))?;

    match result {
        Ok((path, size)) => {
            tracing::info!("Backup: exported to {} ({} bytes)", path, size);
            let _ = on_progress.send(BackupProgress::Done {
                path: path.clone(),
                size_bytes: size,
            });
            Ok(path)
        }
        Err(e) => {
            let _ = on_progress.send(BackupProgress::Error {
                message: e.to_string(),
            });
            Err(e)
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Import / Restore
// ═══════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn import_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    source_path: String,
    on_progress: Channel<RestoreProgress>,
) -> Result<(), AppError> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Backup(format!("Cannot resolve app data dir: {e}")))?;
    let docs_dir = data_dir.join("documents");
    let source = PathBuf::from(&source_path);

    // Step 1: Validate and extract in a blocking thread
    let _ = on_progress.send(RestoreProgress::Validating);

    let data_dir_clone = data_dir.clone();
    let docs_dir_clone = docs_dir.clone();
    let progress = on_progress.clone();

    let (temp_db_path, source_data_dir) = tokio::task::spawn_blocking(
        move || -> Result<(PathBuf, String), AppError> {
            let file = std::fs::File::open(&source)
                .map_err(|e| AppError::Backup(format!("Cannot open backup file: {e}")))?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| AppError::Backup(format!("Invalid ZIP archive: {e}")))?;

            // Validate manifest
            let manifest: BackupManifest = {
                let manifest_file = archive.by_name("manifest.json").map_err(|_| {
                    AppError::Backup(
                        "Not a valid LlmPrivate backup: missing manifest.json".to_string(),
                    )
                })?;
                serde_json::from_reader(manifest_file).map_err(|e| {
                    AppError::Backup(format!("Invalid backup manifest: {e}"))
                })?
            };

            // Validate DB exists in archive
            archive.by_name("llmprivate.db").map_err(|_| {
                AppError::Backup(
                    "Not a valid LlmPrivate backup: missing llmprivate.db".to_string(),
                )
            })?;

            tracing::info!(
                "Backup: restoring from v{} backup created at {}",
                manifest.app_version,
                manifest.created_at
            );

            // Step 2: Extract documents (replacing existing)
            std::fs::create_dir_all(&docs_dir_clone)?;

            // Count document entries for progress
            let doc_count = (0..archive.len())
                .filter(|i| {
                    archive
                        .by_index(*i)
                        .map(|f| {
                            let name = f.name().to_string();
                            name.starts_with("documents/") && name.len() > "documents/".len()
                        })
                        .unwrap_or(false)
                })
                .count() as u32;

            // Clear existing documents directory
            if docs_dir_clone.exists() {
                for entry in std::fs::read_dir(&docs_dir_clone)? {
                    if let Ok(entry) = entry {
                        if entry.path().is_file() {
                            let _ = std::fs::remove_file(entry.path());
                        }
                    }
                }
            }

            // Extract document files
            let mut extracted = 0u32;
            for i in 0..archive.len() {
                let mut file = archive
                    .by_index(i)
                    .map_err(|e| AppError::Backup(format!("ZIP read error: {e}")))?;
                let name = file.name().to_string();
                if name.starts_with("documents/") && name.len() > "documents/".len() {
                    let fname = &name["documents/".len()..];
                    // Safety: prevent path traversal
                    if fname.contains("..") || fname.contains('/') || fname.contains('\\') {
                        tracing::warn!("Backup: skipping suspicious path: {}", fname);
                        continue;
                    }
                    let dest_file = docs_dir_clone.join(fname);
                    let mut out = std::fs::File::create(&dest_file)?;
                    std::io::copy(&mut file, &mut out)?;
                    extracted += 1;
                    let _ = progress.send(RestoreProgress::ExtractingDocuments {
                        current: extracted,
                        total: doc_count,
                    });
                }
            }
            tracing::info!("Backup: extracted {} documents", extracted);

            // Step 3: Extract database to temp location
            let temp_db = data_dir_clone.join("llmprivate_restore_temp.db");
            {
                let mut db_entry = archive
                    .by_name("llmprivate.db")
                    .map_err(|e| AppError::Backup(format!("ZIP read error: {e}")))?;
                let mut out = std::fs::File::create(&temp_db)?;
                std::io::copy(&mut db_entry, &mut out)?;
            }

            Ok((temp_db, manifest.source_data_dir))
        },
    )
    .await
    .map_err(|e| AppError::Backup(format!("Restore task failed: {e}")))?
    ?;

    // Step 4: Replace database (must happen on the main async context, not spawn_blocking,
    // because state.db is behind a State reference)
    let _ = on_progress.send(RestoreProgress::ReplacingDatabase);
    state.db.replace_and_reopen(&temp_db_path)?;
    tracing::info!("Backup: database replaced and reopened");

    // Step 5: Rewrite document paths for the new machine
    let old_docs_dir = PathBuf::from(&source_data_dir)
        .join("documents")
        .to_string_lossy()
        .to_string();
    let new_docs_dir = docs_dir.to_string_lossy().to_string();

    let rewritten = if old_docs_dir != new_docs_dir {
        state.db.rewrite_document_paths(&old_docs_dir, &new_docs_dir)?
    } else {
        0 // Same machine, no rewriting needed
    };
    let _ = on_progress.send(RestoreProgress::RewritingPaths { rewritten });
    tracing::info!("Backup: rewrote {} document paths", rewritten);

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_db_path);

    let _ = on_progress.send(RestoreProgress::Done);
    tracing::info!("Backup: restore complete");

    Ok(())
}
