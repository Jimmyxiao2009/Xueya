use super::parser::ParsedDocument;
use crate::domain::Chunk;
use crate::error::AppError;
use crate::util::hashing::sha256_text;
use crate::util::text::estimate_tokens;
use crate::util::time::utc_now;
use regex::Regex;
use uuid::Uuid;

pub trait Chunker: Send + Sync {
    fn chunk(&self, document_id: Uuid, document: &ParsedDocument) -> Result<Vec<Chunk>, AppError>;
}

#[derive(Debug, Clone, Copy)]
pub struct HeadingAwareChunker {
    pub max_chars: usize,
    pub overlap_chars: usize,
}

impl Default for HeadingAwareChunker {
    fn default() -> Self {
        Self {
            max_chars: 1800,
            overlap_chars: 160,
        }
    }
}

impl HeadingAwareChunker {
    fn heading_regex() -> Result<Regex, AppError> {
        Regex::new(r"^\s{0,3}(#{1,6})\s+(.+?)\s*$")
            .map_err(|error| AppError::Internal(error.to_string()))
    }

    fn clean_heading(value: &str) -> String {
        value.trim().trim_start_matches('#').trim().to_owned()
    }

    #[allow(clippy::too_many_arguments)]
    fn flush_segment(
        chunks: &mut Vec<Chunk>,
        document_id: Uuid,
        heading: Option<String>,
        text: String,
        char_offset: usize,
        page_number: Option<i64>,
        ordinal: &mut i64,
        max_chars: usize,
        overlap_chars: usize,
    ) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let leading = text
            .chars()
            .take_while(|character| character.is_whitespace())
            .count();
        let characters: Vec<char> = trimmed.chars().collect();
        if characters.is_empty() {
            return;
        }

        let mut start = 0usize;
        while start < characters.len() {
            let hard_end = (start + max_chars).min(characters.len());
            let end = if hard_end < characters.len() {
                Self::find_sentence_boundary(&characters, start, hard_end)
            } else {
                hard_end
            };
            let end = if end <= start { hard_end } else { end };
            let value: String = characters[start..end].iter().collect();
            let content_hash = sha256_text(&value);
            chunks.push(Chunk {
                id: Uuid::new_v4(),
                document_id,
                ordinal: *ordinal,
                text: value.clone(),
                char_start: (char_offset + leading + start) as i64,
                char_end: (char_offset + leading + end) as i64,
                page_number,
                heading: heading.clone(),
                token_estimate: estimate_tokens(&value),
                content_hash,
                created_at: utc_now(),
            });
            *ordinal += 1;

            if end >= characters.len() {
                break;
            }
            let next = end.saturating_sub(overlap_chars);
            start = if next <= start { end } else { next };
        }
    }

    fn find_sentence_boundary(characters: &[char], start: usize, hard_end: usize) -> usize {
        let minimum = start + (hard_end - start) / 2;
        for index in (minimum..hard_end).rev() {
            let character = characters[index];
            if matches!(
                character,
                '.' | '!' | '?' | '\n' | '。' | '！' | '？' | '；' | ';'
            ) {
                return (index + 1).min(characters.len());
            }
        }
        hard_end
    }
}

impl Chunker for HeadingAwareChunker {
    fn chunk(&self, document_id: Uuid, document: &ParsedDocument) -> Result<Vec<Chunk>, AppError> {
        let heading_regex = Self::heading_regex()?;
        let mut chunks = Vec::new();
        let mut ordinal = 0i64;

        for page in &document.pages {
            let text = &page.text;
            let mut current_heading: Option<String> = None;
            let mut buffer = String::new();
            let mut buffer_start = 0usize;
            let mut char_offset = 0usize;

            for line in text.lines() {
                let line_length = line.chars().count();
                if let Some(captures) = heading_regex.captures(line) {
                    Self::flush_segment(
                        &mut chunks,
                        document_id,
                        current_heading.take(),
                        std::mem::take(&mut buffer),
                        buffer_start,
                        page.page_number,
                        &mut ordinal,
                        self.max_chars,
                        self.overlap_chars,
                    );
                    current_heading = captures
                        .get(2)
                        .map(|value| Self::clean_heading(value.as_str()))
                        .filter(|value| !value.is_empty());
                    buffer_start = char_offset + line_length + 1;
                } else {
                    if buffer.is_empty() {
                        buffer_start = char_offset;
                    }
                    buffer.push_str(line);
                    buffer.push('\n');
                }
                char_offset += line_length + 1;
            }

            Self::flush_segment(
                &mut chunks,
                document_id,
                current_heading,
                buffer,
                buffer_start,
                page.page_number,
                &mut ordinal,
                self.max_chars,
                self.overlap_chars,
            );
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::parser::{ParsedDocument, ParsedPage};

    #[test]
    fn respects_headings_and_keeps_offsets() {
        let document = ParsedDocument {
            title: "Test".into(),
            pages: vec![ParsedPage {
                page_number: None,
                text: "# First\nAlpha beta gamma. Delta epsilon.\n\n# Second\nZeta eta theta."
                    .into(),
            }],
            metadata_json: "{}".into(),
        };
        let chunks = HeadingAwareChunker {
            max_chars: 30,
            overlap_chars: 5,
        }
        .chunk(Uuid::new_v4(), &document)
        .expect("chunk");

        assert!(!chunks.is_empty());
        assert!(chunks
            .iter()
            .any(|chunk| chunk.heading.as_deref() == Some("First")));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.heading.as_deref() == Some("Second")));
        assert!(chunks
            .iter()
            .all(|chunk| chunk.char_start <= chunk.char_end));
    }

    #[test]
    fn splits_long_sentences_with_overlap() {
        let document = ParsedDocument {
            title: "Long".into(),
            pages: vec![ParsedPage {
                page_number: Some(1),
                text: "One two three. Four five six. Seven eight nine. Ten eleven twelve.".into(),
            }],
            metadata_json: "{}".into(),
        };
        let chunks = HeadingAwareChunker {
            max_chars: 22,
            overlap_chars: 4,
        }
        .chunk(Uuid::new_v4(), &document)
        .expect("chunk");
        assert!(chunks.len() >= 2);
        assert!(chunks[0].text.len() <= 22);
    }
}
