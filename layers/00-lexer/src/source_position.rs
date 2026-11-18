//! Turns a byte offset into the line and column a person would look for.

/// A one-based line and column. The column counts bytes, not characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

/// The byte offset at which each line of a source file starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    line_start_offsets: Vec<usize>, // Always begins with 0, so a file has at least one line.
}

impl LineIndex {
    /// Records where every line begins. A line ends at a line feed; a carriage return is an ordinary byte.
    pub fn new(source_bytes: &[u8]) -> Self {
        let mut line_start_offsets = vec![0];
        for (byte_offset, &byte) in source_bytes.iter().enumerate() {
            if byte == b'\n' {
                line_start_offsets.push(byte_offset + 1);
            }
        }
        Self { line_start_offsets }
    }

    /// Finds the line containing `byte_offset`. An offset at the end of the file is on the last line.
    pub fn get_position(&self, byte_offset: usize) -> SourcePosition {
        // The first line start is 0, so there is always at least one start at or before any offset.
        let line_index = self
            .line_start_offsets
            .partition_point(|&line_start_offset| line_start_offset <= byte_offset)
            .saturating_sub(1);
        let line_start_offset = self
            .line_start_offsets
            .get(line_index)
            .copied()
            .unwrap_or(0);
        SourcePosition {
            line: line_index + 1,
            column: byte_offset - line_start_offset + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LineIndex, SourcePosition};

    #[test]
    fn positions_are_one_based_and_lines_break_after_line_feeds() {
        let line_index = LineIndex::new(b"ab\ncd\n");
        assert_eq!(
            line_index.get_position(0),
            SourcePosition { line: 1, column: 1 }
        );
        assert_eq!(
            line_index.get_position(2),
            SourcePosition { line: 1, column: 3 }
        );
        assert_eq!(
            line_index.get_position(3),
            SourcePosition { line: 2, column: 1 }
        );
        assert_eq!(
            line_index.get_position(6),
            SourcePosition { line: 3, column: 1 }
        );
    }

    #[test]
    fn an_empty_file_has_one_line() {
        assert_eq!(
            LineIndex::new(b"").get_position(0),
            SourcePosition { line: 1, column: 1 }
        );
    }
}
