//! Text extraction from various document formats.

use std::path::Path;

use crate::error::AppError;

/// Extract plain text content from a document file.
/// Dispatches to format-specific extractors based on file extension.
pub fn extract_text(path: &Path) -> Result<String, AppError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" => extract_text_file(path),
        "pdf" => extract_pdf(path),
        "docx" => extract_docx(path),
        other => Err(AppError::Document(format!(
            "Unsupported file type: .{other}"
        ))),
    }
}

/// Read a plain text or markdown file.
fn extract_text_file(path: &Path) -> Result<String, AppError> {
    std::fs::read_to_string(path)
        .map_err(|e| AppError::Document(format!("Failed to read text file: {e}")))
}

/// Extract text from a PDF using pdf-extract.
fn extract_pdf(path: &Path) -> Result<String, AppError> {
    let bytes = std::fs::read(path)
        .map_err(|e| AppError::Document(format!("Failed to read PDF file: {e}")))?;
    pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| AppError::Document(format!("PDF text extraction failed: {e}")))
}

/// Extract text from a DOCX file.
/// DOCX is a ZIP archive containing XML; text lives in word/document.xml.
fn extract_docx(path: &Path) -> Result<String, AppError> {
    let file = std::fs::File::open(path)
        .map_err(|e| AppError::Document(format!("Failed to open DOCX: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::Document(format!("Invalid DOCX archive: {e}")))?;

    let mut doc_xml = String::new();
    {
        let mut entry = archive
            .by_name("word/document.xml")
            .map_err(|_| AppError::Document("No word/document.xml found in DOCX".into()))?;
        use std::io::Read;
        entry
            .read_to_string(&mut doc_xml)
            .map_err(|e| AppError::Document(format!("Failed to read document.xml: {e}")))?;
    }

    Ok(extract_text_from_docx_xml(&doc_xml))
}

/// Parse DOCX XML and extract text from <w:t> elements.
/// Inserts newlines at paragraph boundaries (<w:p>).
fn extract_text_from_docx_xml(xml: &str) -> String {
    let mut result = String::new();
    let mut in_text = false;
    let mut chars = xml.chars().peekable();
    let mut tag_buf = String::new();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            tag_buf.clear();
            for tc in chars.by_ref() {
                if tc == '>' {
                    break;
                }
                tag_buf.push(tc);
            }
            // <w:t> or <w:t xml:space="preserve">
            if tag_buf.starts_with("w:t") && !tag_buf.starts_with("w:tbl") {
                in_text = true;
            } else if tag_buf == "/w:t" {
                in_text = false;
            } else if tag_buf == "/w:p" {
                result.push('\n');
            }
        } else if in_text {
            result.push(ch);
        }
    }

    result
}
