//! Context assembly for RAG: deduplicates chunks, applies token budget,
//! labels sources for the LLM, and builds the system prompt.

use std::collections::HashMap;

use serde::Serialize;

use super::retrieval::RetrievedChunk;

/// Maximum approximate tokens for the context window.
const DEFAULT_MAX_CONTEXT_TOKENS: usize = 3000;

/// Rough multiplier: 1 word ≈ 1.3 tokens.
const WORD_TO_TOKEN_RATIO: f32 = 1.3;

// ─── Types ───────────────────────────────────────────────────

/// A reference to a source note, used for citation mapping.
#[derive(Debug, Clone, Serialize)]
pub struct SourceRef {
    pub label: String,
    pub note_id: String,
    pub note_title: String,
    pub heading_path: String,
    pub char_start: i32,
    pub char_end: i32,
}

/// Short info emitted to the frontend before generation starts.
#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    pub label: String,
    pub note_id: String,
    pub note_title: String,
    pub heading_path: String,
    pub rrf_score: f32,
}

/// The assembled context ready for LLM consumption.
#[derive(Debug, Clone)]
pub struct AssembledContext {
    /// The system prompt instructing the LLM how to behave.
    pub system_prompt: String,
    /// The formatted context block (chunks with [NOTE-N] labels).
    #[allow(dead_code)]
    pub context_block: String,
    /// Map from label (e.g. "NOTE-1") to source reference.
    pub source_map: HashMap<String, SourceRef>,
    /// Ordered list of sources for the frontend.
    pub sources: Vec<SourceInfo>,
}

pub fn assemble_context(
    chunks: Vec<RetrievedChunk>,
    max_tokens: Option<usize>,
) -> AssembledContext {
    let max_tokens = max_tokens.unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS);

    // 1. Deduplicate overlapping chunks
    let deduped = dedup_overlapping(chunks);

    // 2. Pack within token budget
    let mut packed: Vec<RetrievedChunk> = Vec::new();
    let mut token_count: usize = 0;

    for chunk in deduped {
        let chunk_tokens = estimate_tokens(&chunk.text);
        if token_count + chunk_tokens > max_tokens && !packed.is_empty() {
            break;
        }
        token_count += chunk_tokens;
        packed.push(chunk);
    }

    // 3. Build labeled context block and source map
    let mut context_parts: Vec<String> = Vec::new();
    let mut source_map: HashMap<String, SourceRef> = HashMap::new();
    let mut sources: Vec<SourceInfo> = Vec::new();

    for (i, chunk) in packed.iter().enumerate() {
        let label = format!("NOTE-{}", i + 1);

        context_parts.push(format!(
            "[{}] (from \"{}\"{})\n{}",
            label,
            chunk.note_title,
            if chunk.heading_path.is_empty() {
                String::new()
            } else {
                format!(" > {}", chunk.heading_path)
            },
            chunk.text
        ));

        source_map.insert(
            label.clone(),
            SourceRef {
                label: label.clone(),
                note_id: chunk.note_id.clone(),
                note_title: chunk.note_title.clone(),
                heading_path: chunk.heading_path.clone(),
                char_start: chunk.char_start,
                char_end: chunk.char_end,
            },
        );

        sources.push(SourceInfo {
            label,
            note_id: chunk.note_id.clone(),
            note_title: chunk.note_title.clone(),
            heading_path: chunk.heading_path.clone(),
            rrf_score: chunk.rrf_score,
        });
    }

    let context_block = context_parts.join("\n\n---\n\n");

    // 4. System prompt
    let system_prompt = format!(
        "You are a helpful AI assistant for a personal knowledge management app called Smart Notes. \
You answer questions based ONLY on the user's notes provided below as context. \
\n\n\
RULES:\n\
1. Answer using ONLY information from the provided context. If the context doesn't contain \
enough information, say so clearly.\n\
2. ALWAYS cite your sources using [NOTE-N] labels inline in your response (e.g. \"According to \
your notes [NOTE-1], ...\").\n\
3. Be concise and helpful. Use markdown formatting where appropriate.\n\
4. If multiple sources relate to the topic, synthesize them into a coherent answer.\n\
5. Never invent information not present in the context.\n\n\
CONTEXT ({} sources, ~{} tokens):\n\n{}\n\n\
END OF CONTEXT. Answer the user's question based on the above.",
        sources.len(),
        token_count,
        context_block
    );

    AssembledContext {
        system_prompt,
        context_block,
        source_map,
        sources,
    }
}

// ─── Helpers ─────────────────────────────────────────────────

/// Estimate token count from text (words × 1.3).
fn estimate_tokens(text: &str) -> usize {
    let words = text.split_whitespace().count();
    (words as f32 * WORD_TO_TOKEN_RATIO).ceil() as usize
}

/// Deduplicate overlapping chunks from the same note.
///
/// If two chunks from the same note have overlapping `char_start..char_end`
/// ranges, merge them into one chunk with the combined text.
fn dedup_overlapping(mut chunks: Vec<RetrievedChunk>) -> Vec<RetrievedChunk> {
    if chunks.len() <= 1 {
        return chunks;
    }

    // Group by note_id
    let mut by_note: HashMap<String, Vec<RetrievedChunk>> = HashMap::new();
    for chunk in chunks.drain(..) {
        by_note
            .entry(chunk.note_id.clone())
            .or_default()
            .push(chunk);
    }

    let mut result: Vec<RetrievedChunk> = Vec::new();

    for (_note_id, mut note_chunks) in by_note {
        // Sort by char_start
        note_chunks.sort_by_key(|c| c.char_start);

        let mut merged: Vec<RetrievedChunk> = vec![note_chunks.remove(0)];

        for chunk in note_chunks {
            let last = merged.last_mut().unwrap();
            // Check overlap: if this chunk starts before the last one ends
            if chunk.char_start <= last.char_end {
                // Merge: extend the range and combine text
                if chunk.char_end > last.char_end {
                    last.char_end = chunk.char_end;
                    last.text = format!("{}\n{}", last.text, chunk.text);
                }
                // Keep the best scores
                last.rrf_score = last.rrf_score.max(chunk.rrf_score);
                last.vector_score = last.vector_score.max(chunk.vector_score);
                last.bm25_score = last.bm25_score.max(chunk.bm25_score);
            } else {
                merged.push(chunk);
            }
        }

        result.extend(merged);
    }

    // Re-sort by rrf_score descending (best first)
    result.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(
        note_id: &str,
        title: &str,
        text: &str,
        start: i32,
        end: i32,
        score: f32,
    ) -> RetrievedChunk {
        RetrievedChunk {
            chunk_id: format!("c-{start}"),
            note_id: note_id.into(),
            note_title: title.into(),
            text: text.into(),
            heading_path: String::new(),
            char_start: start,
            char_end: end,
            rrf_score: score,
            vector_score: score,
            bm25_score: 0.0,
        }
    }

    #[test]
    fn test_dedup_no_overlap() {
        let chunks = vec![
            make_chunk("n1", "Note 1", "chunk A", 0, 100, 0.9),
            make_chunk("n1", "Note 1", "chunk B", 200, 300, 0.8),
        ];
        let result = dedup_overlapping(chunks);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_dedup_with_overlap() {
        let chunks = vec![
            make_chunk("n1", "Note 1", "chunk A", 0, 150, 0.9),
            make_chunk("n1", "Note 1", "chunk B", 100, 250, 0.8),
        ];
        let result = dedup_overlapping(chunks);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].char_start, 0);
        assert_eq!(result[0].char_end, 250);
    }

    #[test]
    fn test_dedup_different_notes() {
        let chunks = vec![
            make_chunk("n1", "Note 1", "chunk A", 0, 150, 0.9),
            make_chunk("n2", "Note 2", "chunk B", 0, 150, 0.8),
        ];
        let result = dedup_overlapping(chunks);
        assert_eq!(result.len(), 2); // different notes, no merge
    }

    #[test]
    fn test_assemble_context_labeling() {
        let chunks = vec![make_chunk("n1", "My Note", "The sky is blue.", 0, 16, 0.9)];
        let ctx = assemble_context(chunks, Some(5000));
        assert!(ctx.context_block.contains("[NOTE-1]"));
        assert!(ctx.source_map.contains_key("NOTE-1"));
        assert_eq!(ctx.sources.len(), 1);
        assert_eq!(ctx.sources[0].note_title, "My Note");
    }

    #[test]
    fn test_assemble_context_token_budget() {
        // Create chunks that collectively exceed budget
        let big_text = "word ".repeat(500); // ~500 words ≈ 650 tokens
        let chunks = vec![
            make_chunk("n1", "A", &big_text, 0, 2500, 0.9),
            make_chunk("n2", "B", &big_text, 0, 2500, 0.8),
            make_chunk("n3", "C", &big_text, 0, 2500, 0.7),
        ];
        let ctx = assemble_context(chunks, Some(1400)); // ~1400 token budget
                                                        // Should fit 2 chunks (~1300 tokens) but not 3 (~1950)
        assert!(ctx.sources.len() <= 2);
    }
}
