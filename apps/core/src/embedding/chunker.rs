//! Text chunker - Split conversations into overlapping text chunks for embedding.
//!
//! Strategy: Split on paragraph boundaries with target chunk size and overlap.
//! Each chunk preserves context by including overlapping text from neighbors.
//!
//! All offsets use **byte** positions that are validated to land on UTF-8 char
//! boundaries, so multi-byte characters (e.g. Vietnamese, CJK) are safe.

use crate::parsers::{ParsedConversation, Role};

/// Configuration for text chunking.
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Target bytes per chunk (approx – actual split will snap to nearest
    /// paragraph/sentence/word boundary).
    pub chunk_size: usize,
    /// Overlap bytes between consecutive chunks
    pub chunk_overlap: usize,
    /// Minimum chunk size to keep (discard smaller)
    pub min_chunk_size: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            chunk_overlap: 200,
            min_chunk_size: 50,
        }
    }
}

/// A text chunk with metadata.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Index of this chunk within the source
    pub index: usize,
    /// The chunk text content
    pub content: String,
    /// Byte offset in the original text where this chunk starts
    pub start_offset: usize,
    /// Byte offset in the original text where this chunk ends
    pub end_offset: usize,
}

/// Round a byte offset DOWN to the nearest UTF-8 char boundary.
#[inline]
fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    // Walk backwards until we hit a char boundary
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Round a byte offset UP to the nearest UTF-8 char boundary.
#[inline]
fn ceil_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Split a parsed conversation into chunks suitable for embedding.
///
/// Each message is formatted as `[role]: content` and the conversation
/// is split into overlapping chunks of approximately `chunk_size` characters.
pub fn chunk_conversation(conv: &ParsedConversation, config: &ChunkConfig) -> Vec<Chunk> {
    // Build full text from conversation messages
    let mut text = String::new();

    if let Some(title) = &conv.title {
        text.push_str(&format!("# {}\n\n", title));
    }

    for msg in &conv.messages {
        // Skip system/info messages that don't add semantic value
        if msg.role == Role::System || msg.role == Role::Info {
            continue;
        }

        let role_label = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::Tool => "Tool",
            _ => continue,
        };

        text.push_str(&format!("[{}]: {}\n\n", role_label, msg.content));
    }

    chunk_text(&text, config)
}

/// Split raw text into overlapping chunks.
///
/// Tries to split on paragraph boundaries (\n\n), falling back to
/// sentence boundaries (. or \n), then word boundaries.
/// All byte offsets are snapped to UTF-8 char boundaries.
pub fn chunk_text(text: &str, config: &ChunkConfig) -> Vec<Chunk> {
    if text.len() <= config.chunk_size {
        if text.len() >= config.min_chunk_size {
            return vec![Chunk {
                index: 0,
                content: text.to_string(),
                start_offset: 0,
                end_offset: text.len(),
            }];
        }
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    let mut index = 0;

    while start < text.len() {
        // Ensure start is on a char boundary
        start = ceil_char_boundary(text, start);
        if start >= text.len() {
            break;
        }

        let remaining = text.len() - start;
        let end = if remaining <= config.chunk_size {
            text.len()
        } else {
            // Find a good split point near chunk_size
            let target_end = floor_char_boundary(text, start + config.chunk_size);
            find_split_point(text, start, target_end)
        };

        let chunk_text = &text[start..end];
        let trimmed = chunk_text.trim();

        if trimmed.len() >= config.min_chunk_size {
            chunks.push(Chunk {
                index,
                content: trimmed.to_string(),
                start_offset: start,
                end_offset: end,
            });
            index += 1;
        }

        // Move start forward, accounting for overlap
        if end >= text.len() {
            break;
        }

        let advance = if config.chunk_overlap < (end - start) {
            end - start - config.chunk_overlap
        } else {
            // Overlap is larger than chunk, just advance by minimum
            (end - start).max(config.min_chunk_size)
        };

        start += advance;
    }

    chunks
}

/// Find a good split point near `target_end`.
///
/// Preference order:
/// 1. Paragraph boundary (\n\n)
/// 2. Line boundary (\n)
/// 3. Sentence boundary (. followed by space)
/// 4. Word boundary (space)
/// 5. Nearest char boundary at target_end
fn find_split_point(text: &str, start: usize, target_end: usize) -> usize {
    let target_end = floor_char_boundary(text, target_end);
    let search_start = floor_char_boundary(
        text,
        if target_end > 100 {
            target_end - 100
        } else {
            start
        },
    );
    let search_end = floor_char_boundary(text, (target_end + 50).min(text.len()));
    let search_range = &text[search_start..search_end];

    // Try paragraph boundary
    if let Some(pos) = search_range.rfind("\n\n") {
        let split = search_start + pos + 2;
        if split > start {
            return split;
        }
    }

    // Try line boundary
    if let Some(pos) = search_range.rfind('\n') {
        let split = search_start + pos + 1;
        if split > start {
            return split;
        }
    }

    // Try sentence boundary
    if let Some(pos) = search_range.rfind(". ") {
        let split = search_start + pos + 2;
        if split > start {
            return split;
        }
    }

    // Try word boundary
    if let Some(pos) = search_range.rfind(' ') {
        let split = search_start + pos + 1;
        if split > start {
            return split;
        }
    }

    // Fallback: nearest char boundary
    floor_char_boundary(text, target_end.min(text.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_short_text() {
        let config = ChunkConfig {
            chunk_size: 1000,
            chunk_overlap: 200,
            min_chunk_size: 10,
        };
        let chunks = chunk_text("Hello world, this is a test.", &config);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Hello world, this is a test.");
    }

    #[test]
    fn test_chunk_empty_text() {
        let config = ChunkConfig::default();
        let chunks = chunk_text("", &config);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_long_text() {
        let config = ChunkConfig {
            chunk_size: 100,
            chunk_overlap: 20,
            min_chunk_size: 10,
        };

        // Create a text with several paragraphs
        let text = (0..10)
            .map(|i| format!("This is paragraph number {}. It has some content.\n\n", i))
            .collect::<String>();

        let chunks = chunk_text(&text, &config);
        assert!(chunks.len() > 1);

        // Verify chunks have content
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
            assert!(chunk.content.len() >= config.min_chunk_size);
        }
    }

    #[test]
    fn test_chunk_indices_sequential() {
        let config = ChunkConfig {
            chunk_size: 50,
            chunk_overlap: 10,
            min_chunk_size: 5,
        };
        let text = "word ".repeat(100);
        let chunks = chunk_text(&text, &config);

        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
    }

    #[test]
    fn test_chunk_unicode_vietnamese() {
        // Vietnamese text with multi-byte UTF-8 chars (ờ is 3 bytes)
        let config = ChunkConfig {
            chunk_size: 80,
            chunk_overlap: 20,
            min_chunk_size: 10,
        };
        let text = "Tổng hợp ý tưởng và phân tích giải pháp.\n\n\
                     Người dùng muốn tiếp tục cuộc hội thoại.\n\n\
                     Đây là đoạn văn bản tiếng Việt dài hơn để kiểm tra chunker.";
        let chunks = chunk_text(text, &config);
        assert!(
            !chunks.is_empty(),
            "Should produce chunks from Vietnamese text"
        );
        for chunk in &chunks {
            assert!(chunk.content.is_char_boundary(0));
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn test_chunk_unicode_cjk() {
        let config = ChunkConfig {
            chunk_size: 30,
            chunk_overlap: 5,
            min_chunk_size: 5,
        };
        // CJK chars are 3 bytes each
        let text = "日本語テキスト。\n\nこれはテストです。\n\n中文文本测试。";
        let chunks = chunk_text(text, &config);
        assert!(!chunks.is_empty(), "Should produce chunks from CJK text");
    }

    #[test]
    fn test_floor_ceil_char_boundary() {
        let text = "Tờ"; // 'T' = 1 byte, 'ờ' = 3 bytes → total 4 bytes
        assert_eq!(text.len(), 4);
        assert_eq!(floor_char_boundary(text, 0), 0); // start of 'T'
        assert_eq!(floor_char_boundary(text, 1), 1); // start of 'ờ'
        assert_eq!(floor_char_boundary(text, 2), 1); // inside 'ờ' → back to 1
        assert_eq!(floor_char_boundary(text, 3), 1); // inside 'ờ' → back to 1
        assert_eq!(floor_char_boundary(text, 4), 4); // past end → s.len()
        assert_eq!(ceil_char_boundary(text, 2), 4); // inside 'ờ' → up to end
        assert_eq!(ceil_char_boundary(text, 0), 0); // already on boundary
        assert_eq!(ceil_char_boundary(text, 1), 1); // already on boundary
    }
}
