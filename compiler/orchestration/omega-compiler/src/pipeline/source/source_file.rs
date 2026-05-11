use std::path::PathBuf;

use crate::parser::SourceTrees;
use crate::source::SourceId;
use omega_syntax_trees::item::Item;
use omega_syntax_trees::tables::AstTables;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source_trees: SourceTrees,
}

impl Default for SourceFile {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source_trees: SourceTrees {
                source_id: SourceId::default(),
                items: Vec::new(),
                tables: AstTables::default(),
            },
        }
    }
}

impl SourceFile {
    pub fn items(&self) -> &[Item] {
        &self.source_trees.items
    }
}
