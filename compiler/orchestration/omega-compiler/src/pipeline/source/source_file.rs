use std::path::PathBuf;

use crate::source::SourceId;
use omega_syntax_trees::SyntaxTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub syntax_trees: SyntaxTrees,
}

impl Default for SourceFile {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            syntax_trees: SyntaxTrees::default(),
        }
    }
}
