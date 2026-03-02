//! Document chunk retrieval using SQLite FTS5.

use crate::db::connection::Database;
use crate::documents::types::ChunkSearchResult;
use crate::error::AppError;

/// Search document chunks using FTS5 with BM25 ranking.
///
/// - `query`: user's search query (will be tokenized by FTS5)
/// - `document_ids`: optional filter to specific documents
/// - `limit`: max results to return
pub fn search_chunks(
    db: &Database,
    query: &str,
    document_ids: Option<&[String]>,
    limit: u32,
) -> Result<Vec<ChunkSearchResult>, AppError> {
    let fts_query = sanitize_fts_query(query);
    if fts_query.is_empty() {
        return Ok(vec![]);
    }
    db.search_document_chunks(&fts_query, document_ids, limit)
}

/// Sanitize user input for FTS5 queries.
/// Removes special FTS5 operators to prevent syntax errors.
/// Uses OR between terms so partial matches return results.
fn sanitize_fts_query(query: &str) -> String {
    let cleaned: String = query
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '\'')
        .collect();

    let words: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|w| w.len() > 1) // skip single chars
        .collect();

    if words.is_empty() {
        return String::new();
    }

    // Use OR to match any term, so "what is the course objective"
    // becomes "what OR the OR course OR objective" (more forgiving)
    words.join(" OR ")
}

/// Build a context string from retrieved chunks for injection into the LLM prompt.
/// Respects a character budget to avoid overflowing the context window.
pub fn build_context_from_chunks(chunks: &[ChunkSearchResult], max_chars: usize) -> String {
    let mut context = String::new();
    let mut current_doc = String::new();

    for chunk in chunks {
        let entry = if chunk.document_filename != current_doc {
            current_doc.clone_from(&chunk.document_filename);
            format!(
                "\n--- From: {} ---\n{}\n",
                chunk.document_filename, chunk.chunk.content
            )
        } else {
            format!("{}\n", chunk.chunk.content)
        };

        if context.len() + entry.len() > max_chars {
            break;
        }
        context.push_str(&entry);
    }

    context
}

/// Build context from ordered chunks (for summarize/quiz where we want sequential content).
pub fn build_sequential_context(chunks: &[ChunkSearchResult], max_chars: usize) -> String {
    let mut context = String::new();

    for chunk in chunks {
        let entry = format!("{}\n", chunk.chunk.content);
        if context.len() + entry.len() > max_chars {
            break;
        }
        context.push_str(&entry);
    }

    context
}
