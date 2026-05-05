use std::path::PathBuf;

use crate::source::{FileId, SourceFile};

#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn add(&mut self, path: PathBuf, source: String) -> &SourceFile {
        self.files.push(SourceFile {
            id: FileId(self.files.len()),
            path,
            source,
        });

        self.files
            .last()
            .expect("source map should contain added file")
    }
}
