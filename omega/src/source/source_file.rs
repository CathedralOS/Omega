use std::path::PathBuf;

use crate::source::FileId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub source: String,
}
