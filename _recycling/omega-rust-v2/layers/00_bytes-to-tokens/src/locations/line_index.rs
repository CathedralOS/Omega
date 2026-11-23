//! Turns a byte offset into a `SourcePosition` by remembering where every line starts.

use crate::locations::source_position::SourcePosition;

/// The byte offset at which each line of a source file starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineIndex {
    line_start_offsets: Vec<usize>, // Always begins with 0, so a file has at least one line.
}

impl LineIndex {
    /// Records where every line begins. A line ends at a line feed; a carriage return is an ordinary byte.
    pub fn new(source_bytes: &[u8]) -> Self {
        let mut line_start_offsets = vec![0];

        for (byte_offset, &source_byte) in source_bytes.iter().enumerate() {
            if source_byte == b'\n' {
                line_start_offsets.push(byte_offset + 1);
            }
        }

        Self { line_start_offsets }
    }

    /// Finds the line containing `byte_offset`. An offset at the end of the file is on the last line.
    pub fn get_position(
        &self,
        byte_offset: usize,
    ) -> SourcePosition {
        // The first line start is 0, so there is always at least one start at or before any offset.
        let line_number_zero_based = self
            .line_start_offsets
            .partition_point(|&line_start_offset| line_start_offset <= byte_offset)
            .saturating_sub(1);
        let line_start_offset = self
            .line_start_offsets
            .get(line_number_zero_based)
            .copied()
            .unwrap_or(0);

        SourcePosition::new(line_number_zero_based + 1, byte_offset - line_start_offset + 1)
    }
}

#[cfg(test)]
mod tests {
    use crate::locations::line_index::LineIndex;
    use crate::locations::source_position::SourcePosition;

    #[test]
    fn positions_are_one_based_and_lines_break_after_line_feeds() {
        let line_index = LineIndex::new(b"ab\ncd\n");

        assert_eq!(line_index.get_position(0), SourcePosition::new(1, 1));
        assert_eq!(line_index.get_position(2), SourcePosition::new(1, 3));
        assert_eq!(line_index.get_position(3), SourcePosition::new(2, 1));
        assert_eq!(line_index.get_position(6), SourcePosition::new(3, 1));
    }

    #[test]
    fn an_empty_file_has_one_line() {
        assert_eq!(LineIndex::new(b"").get_position(0), SourcePosition::new(1, 1));
    }
}
