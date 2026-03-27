use crate::notes::parser::Heading;
use crate::rag::lang::{split_sentences_multilingual, token_count_multilingual};
use serde::{Deserialize, Serialize};

/// A chunk of text from a document, ready for embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
    pub heading_path: String,
    pub chunk_index: usize,
}

const MAX_CHUNK_TOKENS: usize = 512;
const OVERLAP_TOKENS: usize = 64;

pub fn chunk_document(body: &str, headings: &[Heading]) -> Vec<Chunk> {
    let sections = split_by_headings(body, headings);
    let mut chunks = Vec::new();
    let mut chunk_index = 0;

    for section in sections {
        let section_chunks =
            chunk_section(&section.text, &section.heading_path, section.char_start);
        for mut chunk in section_chunks {
            chunk.chunk_index = chunk_index;
            chunks.push(chunk);
            chunk_index += 1;
        }
    }

    if chunks.is_empty() && !body.trim().is_empty() {
        chunks.push(Chunk {
            text: body.to_string(),
            char_start: 0,
            char_end: body.len(),
            heading_path: String::new(),
            chunk_index: 0,
        });
    }

    chunks
}

struct Section {
    text: String,
    heading_path: String,
    char_start: usize,
}

fn split_by_headings(body: &str, headings: &[Heading]) -> Vec<Section> {
    if headings.is_empty() {
        return vec![Section {
            text: body.to_string(),
            heading_path: String::new(),
            char_start: 0,
        }];
    }

    let mut sections = Vec::new();
    let lines: Vec<&str> = body.lines().collect();

    let mut heading_positions: Vec<(usize, &Heading)> = Vec::new();
    for heading in headings {
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with('#') && trimmed.contains(&heading.text) {
                heading_positions.push((i, heading));
                break;
            }
        }
    }

    let mut heading_path_stack: Vec<(u8, String)> = Vec::new();

    for (idx, (line_num, heading)) in heading_positions.iter().enumerate() {
        let end_line = if idx + 1 < heading_positions.len() {
            heading_positions[idx + 1].0
        } else {
            lines.len()
        };

        while heading_path_stack
            .last()
            .is_some_and(|(level, _)| *level >= heading.level)
        {
            heading_path_stack.pop();
        }
        heading_path_stack.push((heading.level, heading.text.clone()));

        let heading_path = heading_path_stack
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join(" > ");

        let section_text: String = lines[*line_num..end_line].join("\n");
        let char_start = lines[..*line_num]
            .iter()
            .map(|l| l.len() + 1) // +1 for newline
            .sum::<usize>();

        if !section_text.trim().is_empty() {
            sections.push(Section {
                text: section_text,
                heading_path,
                char_start,
            });
        }
    }

    if let Some(first_pos) = heading_positions.first() {
        if first_pos.0 > 0 {
            let preamble: String = lines[..first_pos.0].join("\n");
            if !preamble.trim().is_empty() {
                let preamble_section = Section {
                    text: preamble,
                    heading_path: String::new(),
                    char_start: 0,
                };
                sections.insert(0, preamble_section);
            }
        }
    }

    if sections.is_empty() {
        sections.push(Section {
            text: body.to_string(),
            heading_path: String::new(),
            char_start: 0,
        });
    }

    sections
}

fn chunk_section(text: &str, heading_path: &str, base_offset: usize) -> Vec<Chunk> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let token_count = estimate_tokens(text);

    if token_count <= MAX_CHUNK_TOKENS {
        return vec![Chunk {
            text: text.to_string(),
            char_start: base_offset,
            char_end: base_offset + text.len(),
            heading_path: heading_path.to_string(),
            chunk_index: 0,
        }];
    }

    let paragraphs = split_paragraphs(text);
    let mut chunks = Vec::new();
    let mut current_text = String::new();
    let mut current_start = base_offset;

    for para in &paragraphs {
        let combined_tokens = estimate_tokens(&format!("{}\n\n{}", current_text, para));

        if combined_tokens > MAX_CHUNK_TOKENS && !current_text.is_empty() {
            // Flush current chunk
            let chunk_text = current_text.trim().to_string();
            if !chunk_text.is_empty() {
                chunks.push(Chunk {
                    text: chunk_text.clone(),
                    char_start: current_start,
                    char_end: current_start + chunk_text.len(),
                    heading_path: heading_path.to_string(),
                    chunk_index: 0,
                });
            }
            current_start += current_text.len();
            current_text = para.to_string();
        } else {
            if current_text.is_empty() {
                current_text = para.to_string();
            } else {
                current_text = format!("{}\n\n{}", current_text, para);
            }
        }
    }

    // Flush remaining
    let chunk_text = current_text.trim().to_string();
    if !chunk_text.is_empty() {
        // If still too long, use sliding window
        if estimate_tokens(&chunk_text) > MAX_CHUNK_TOKENS {
            let window_chunks = sliding_window_chunks(&chunk_text, heading_path, current_start);
            chunks.extend(window_chunks);
        } else {
            chunks.push(Chunk {
                text: chunk_text.clone(),
                char_start: current_start,
                char_end: current_start + chunk_text.len(),
                heading_path: heading_path.to_string(),
                chunk_index: 0,
            });
        }
    }

    chunks
}

/// Sliding window chunking for text that exceeds token limits.
fn sliding_window_chunks(text: &str, heading_path: &str, base_offset: usize) -> Vec<Chunk> {
    // Split text into multilingual sentences instead of simple whitespace arrays
    let sentences = split_sentences_multilingual(text);

    let mut chunks = Vec::new();

    // Fallback if sentences are impossibly long
    if sentences.is_empty() {
        return vec![Chunk {
            text: text.to_string(),
            char_start: base_offset,
            char_end: base_offset + text.len(),
            heading_path: heading_path.to_string(),
            chunk_index: 0,
        }];
    }

    let mut i = 0;
    while i < sentences.len() {
        let mut chunk_text = String::new();
        let mut tokens = 0;
        let mut j = i;

        // Pack sentences until we hit MAX_CHUNK_TOKENS
        while j < sentences.len() {
            let sentence_tokens = estimate_tokens(sentences[j]);
            if tokens + sentence_tokens > MAX_CHUNK_TOKENS && !chunk_text.is_empty() {
                break;
            }
            if !chunk_text.is_empty() {
                chunk_text.push(' ');
            }
            chunk_text.push_str(sentences[j]);
            tokens += sentence_tokens;
            j += 1;
        }

        let char_start = base_offset + text.find(chunk_text.trim()).unwrap_or(0);
        let char_end = char_start + chunk_text.trim().len();

        chunks.push(Chunk {
            text: chunk_text.trim().to_string(),
            char_start,
            char_end,
            heading_path: heading_path.to_string(),
            chunk_index: 0,
        });

        // Determine next start index based on overlap.
        // We step forward by advancing at least 1 sentence, leaving up to OVERLAP_TOKENS behind.
        let mut overlap_tokens_count = 0;
        let mut rewind = 0;
        for k in (i..j).rev() {
            overlap_tokens_count += estimate_tokens(sentences[k]);
            if overlap_tokens_count > OVERLAP_TOKENS && rewind > 0 {
                break;
            }
            rewind += 1;
        }

        let step = (j - i).saturating_sub(rewind).max(1);
        i += step;
    }

    chunks
}

/// Split text into paragraphs (separated by double newlines).
fn split_paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Estimate token count from text. Approximation: word_count * 1.3
pub fn estimate_tokens(text: &str) -> usize {
    token_count_multilingual(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_short_document() {
        let body = "# Hello\n\nThis is a short document.";
        let headings = vec![Heading {
            level: 1,
            text: "Hello".to_string(),
            line: 0,
        }];
        let chunks = chunk_document(body, &headings);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_index, 0);
    }

    #[test]
    fn test_chunk_empty_document() {
        let chunks = chunk_document("", &[]);
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello world"), 3); // 2 words * 1.3 = 2.6 → ceil = 3
    }
}
