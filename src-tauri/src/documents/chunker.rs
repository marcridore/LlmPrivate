//! Text chunking for document retrieval.

/// A text chunk with positional information.
pub struct TextChunk {
    pub index: usize,
    pub content: String,
    pub char_offset: usize,
    pub char_length: usize,
}

/// Split text into overlapping chunks for retrieval.
///
/// Uses ~500 character chunks with ~100 character overlap by default.
/// Tries to break at sentence/paragraph boundaries to avoid splitting mid-sentence.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<TextChunk> {
    let chunk_size = chunk_size.max(100);
    let overlap = overlap.min(chunk_size / 2);

    // Trim whitespace
    let text = text.trim();
    if text.is_empty() {
        return vec![];
    }

    if text.len() <= chunk_size {
        return vec![TextChunk {
            index: 0,
            content: text.to_string(),
            char_offset: 0,
            char_length: text.len(),
        }];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = clamp_to_char_boundary(text, (start + chunk_size).min(text.len()));

        // Try to find a good break point near the end
        let actual_end = if end < text.len() {
            find_break_point(text, start + chunk_size / 2, end)
        } else {
            end
        };

        let chunk_content = &text[start..actual_end];
        if !chunk_content.trim().is_empty() {
            chunks.push(TextChunk {
                index: chunks.len(),
                content: chunk_content.to_string(),
                char_offset: start,
                char_length: actual_end - start,
            });
        }

        if actual_end >= text.len() {
            break;
        }

        // Next chunk starts `overlap` chars before the end of this one.
        // Must clamp to a valid UTF-8 char boundary to avoid panics on
        // multi-byte characters (e.g. accented French text from PDFs).
        start = if actual_end > overlap {
            clamp_to_char_boundary(text, actual_end - overlap)
        } else {
            actual_end
        };
    }

    chunks
}

/// Find the best break point in [min_pos, max_pos) range.
/// Prefers paragraph breaks > sentence boundaries > word boundaries.
fn find_break_point(text: &str, min_pos: usize, max_pos: usize) -> usize {
    // Clamp to valid char boundaries
    let min_pos = clamp_to_char_boundary(text, min_pos);
    let max_pos = clamp_to_char_boundary(text, max_pos);

    if min_pos >= max_pos {
        return max_pos;
    }

    let search_range = &text[min_pos..max_pos];

    // Prefer paragraph break
    if let Some(pos) = search_range.rfind("\n\n") {
        return min_pos + pos + 2;
    }
    // Sentence boundary
    for pattern in &[". ", "? ", "! ", ".\n"] {
        if let Some(pos) = search_range.rfind(pattern) {
            return min_pos + pos + pattern.len();
        }
    }
    // Word boundary
    if let Some(pos) = search_range.rfind(' ') {
        return min_pos + pos + 1;
    }
    // No good break point found
    max_pos
}

/// Ensure position falls on a valid UTF-8 character boundary.
fn clamp_to_char_boundary(text: &str, pos: usize) -> usize {
    if pos >= text.len() {
        return text.len();
    }
    // Move forward to the next char boundary
    let mut p = pos;
    while p < text.len() && !text.is_char_boundary(p) {
        p += 1;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_text_single_chunk() {
        let chunks = chunk_text("Hello world", 500, 100);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Hello world");
    }

    #[test]
    fn test_empty_text() {
        let chunks = chunk_text("", 500, 100);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunking_produces_overlap() {
        let text = "A".repeat(1000);
        let chunks = chunk_text(&text, 500, 100);
        assert!(chunks.len() >= 2);
        // The second chunk should start before the end of the first
        assert!(chunks[1].char_offset < chunks[0].char_offset + chunks[0].char_length);
    }

    #[test]
    fn test_multibyte_utf8_no_panic() {
        // French text with accented characters (multi-byte UTF-8)
        let text = "Résumé du cours. Évaluation des étudiants. ".repeat(100);
        let chunks = chunk_text(&text, 500, 100);
        assert!(!chunks.is_empty());
        // Every chunk should be valid UTF-8 (would panic otherwise)
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn test_sentence_boundary_breaking() {
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence. Fifth sentence.";
        let chunks = chunk_text(text, 40, 10);
        // Should break at sentence boundaries, not mid-word
        for chunk in &chunks {
            // Each chunk should end at a sentence boundary or be the last chunk
            let trimmed = chunk.content.trim();
            assert!(
                trimmed.ends_with('.')
                    || trimmed.ends_with('?')
                    || trimmed.ends_with('!')
                    || chunk.char_offset + chunk.char_length >= text.len(),
                "Chunk should end at sentence boundary: {:?}",
                trimmed
            );
        }
    }
}
