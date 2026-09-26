use std::path::PathBuf;

use source::SourceId;
use tokens_to_syntax_trees::syntax_trees::item::ItemHandle;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceFile {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub root_items: Vec<ItemHandle>,
}
