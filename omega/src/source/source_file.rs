use std::path::PathBuf;

use crate::source::FileId;

#[derive(Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

impl SourceFile {
    pub fn position_at(&self, byte_offset: usize) -> SourcePosition {
        let mut line = 1;
        let mut column = 1;

        for (index, character) in self.source.char_indices() {
            if index >= byte_offset {
                break;
            }

            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }

        SourcePosition { line, column }
    }
}
