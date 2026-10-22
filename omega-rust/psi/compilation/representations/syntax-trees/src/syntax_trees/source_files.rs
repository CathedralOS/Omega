//! The per-source roots appended to a syntax arena by one parsing batch.
//!
//! These are source-to-root associations, not a second syntax program. Root
//! handles refer to the caller's SyntaxTrees; source scope and origin are
//! retained for later import and source-map custody.

use crate::item::ItemHandle;
use arena::{Arena, HandleSpan};
use source::{DependencyScope, SourceId, SourceOrigin};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

/// Source-local item order and the smallest range containing every import.
/// Interleaved declarations remain in the range; import consumers still inspect
/// their kind. Mathematical definitions live outside the item ordinal space.
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
    /// Bounds in `root_items`, published while parsing to avoid rediscovery.
    pub import_root_range: Range<usize>,
    /// Which dependency context checked this instance. An identical path may
    /// have a second [`ParsedSource`] under the other scope.
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
