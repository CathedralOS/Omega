use crate::item::ItemHandle;
use arena::{Arena, HandleSpan};
use source::{DependencyScope, SourceId, SourceOrigin};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedRootItems {
    pub items: Vec<ItemHandle>,
    pub import_range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub origin: Option<SourceOrigin>,
    pub root_items: Vec<ItemHandle>,
    pub import_root_range: Range<usize>,
    pub scope: DependencyScope,
}

impl Default for ParsedSource {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            origin: None,
            root_items: Vec::new(),
            import_root_range: 0..0,
            scope: DependencyScope::Product,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedSources {
    pub sources: Arena<ParsedSource>,
    pub batch: HandleSpan<ParsedSource>,
}

/// One source parsed into its own standalone page under private
/// coordinates, retained so a later admission of the same bytes can adopt
/// it instead of parsing again (`SyntaxTrees::adopt_unit`).
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ParsedUnit {
    pub syntax: crate::SyntaxTrees,
    pub source: Arc<str>,
    pub root_items: Vec<ItemHandle>,
    pub import_root_range: Range<usize>,
}

impl ParsedUnit {
    pub fn root_items(&self) -> &[ItemHandle] {
        &self.root_items
    }
}
