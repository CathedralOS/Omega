use std::path::PathBuf;

use crate::source::{FileId, SourceFile};

#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn add(&mut self, path: PathBuf, source: String) -> SourceFile {
        let file = SourceFile {
            id: FileId(self.files.len()),
            path,
            source,
        };

        self.files.push(file.clone());
        file
    }
}
