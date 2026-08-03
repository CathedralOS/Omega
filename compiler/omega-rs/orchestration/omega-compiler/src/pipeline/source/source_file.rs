use std::path::PathBuf;

use crate::source::SourceId;
use psi_syntax_trees::item::ItemHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub root_items: Vec<ItemHandle>,
}

impl Default for SourceFile {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            root_items: Vec::new(),
        }
    }
}
