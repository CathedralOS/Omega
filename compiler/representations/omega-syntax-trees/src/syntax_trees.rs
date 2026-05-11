use crate::item::Item;
use crate::tables::AstTables;
use omega_core::source::SourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTrees {
    pub source_id: SourceId,
    pub items: Vec<Item>,
    pub tables: AstTables,
}

impl SyntaxTrees {
    pub fn from_items(source_id: SourceId, items: Vec<Item>) -> Self {
        let tables = AstTables::from_items(&items);

        Self {
            source_id,
            items,
            tables,
        }
    }
}

