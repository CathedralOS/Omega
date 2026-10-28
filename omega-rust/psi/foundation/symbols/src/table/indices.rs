//! Deferred lookup indices: root-name, binding-source, module-path and
//! module-scope maps built on first lookup and cleared by mutation. The
//! caches ignore themselves for equality so a warmed table still compares
//! equal to a cold one.

use super::{SourceScopedTopLevelBinding, SymbolHandle};
use source::SourceId;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(super) struct RootNameIndex {
    pub(super) by_name: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
    pub(super) by_leaf: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
}

/// Selected bindings and their derived namespace environment belong to one
/// immutable table frontier. Queries borrow these rows without rebuilding paths.
#[derive(Clone, Default)]
pub(super) struct BindingSourceIndex {
    pub(super) by_source: crate::SymbolKeyMap<SourceId, SourceLookupScope>,
    pub(super) has_selected_module: bool,
}

#[derive(Clone, Default)]
pub(super) struct SourceLookupScope {
    pub(super) bindings: Vec<SourceScopedTopLevelBinding>,
    pub(super) imports: Vec<PreparedModuleImport>,
    pub(super) has_exact_import: bool,
}

/// `binding_position` addresses this scope's ordered binding batch. Derived
/// spellings describe lookup only; visibility and package authority remain queries.
#[derive(Clone)]
pub(super) struct PreparedModuleImport {
    pub(super) binding_position: usize,
    pub(super) logical: String,
    pub(super) package_prefix: String,
    pub(super) module_leaf: Option<String>,
}

#[derive(Clone, Default)]
pub(super) struct ModulePathIndex {
    pub(super) paths: crate::SymbolKeyMap<SymbolHandle, Arc<str>>,
}

/// Candidates for a module-qualified lookup, keyed by how the match arms
/// select them. `order` is the position in the original roster walk —
/// root children then module symbols — so gathering buckets and sorting
/// reproduces the scan's candidate order exactly, including a handle that
/// appears in both halves (its two positions stay distinct).
#[derive(Clone, Default)]
pub(super) struct ModuleScopeIndex {
    /// `name` → (order, handle).
    pub(super) by_name: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
    /// `display_path("::")` → (order, handle).
    pub(super) by_path: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
    /// Declared leaf name → (order, handle), for Domain-kind candidates
    /// only: the carrier-qualified arm is the sole leaf-shaped selector.
    pub(super) domain_leaves: crate::SymbolKeyMap<Box<str>, Vec<(u32, SymbolHandle)>>,
}

/// Clones share one immutable lookup frontier, including its deferred work.
/// A mutation detaches a shared cell; clearing a unique cell needs no new
/// allocation. The cache is invisible to semantic equality and debug output.
macro_rules! index_cache {
    ($(#[$cache_meta:meta])* $cache:ident, $index:ty, $label:literal) => {
        $(#[$cache_meta])*
        #[derive(Clone, Default)]
        pub(super) struct $cache(pub(super) Arc<std::sync::OnceLock<$index>>);

        impl $cache {
            pub(super) fn clear(&mut self) {
                if let Some(cell) = Arc::get_mut(&mut self.0) {
                    cell.take();
                } else {
                    self.0 = Arc::default();
                }
            }
        }

        impl PartialEq for $cache {
            fn eq(&self, _: &Self) -> bool {
                true
            }
        }

        impl Eq for $cache {}

        impl std::fmt::Debug for $cache {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str($label)
            }
        }
    };
}

index_cache!(
    /// The table's root children bucketed by full name and by leaf, built on the
    /// first top-level lookup. Every mutation that can change the root roster
    /// clears it, so it always describes this table; equality ignores it.
    RootNameIndexCache,
    RootNameIndex,
    "RootNameIndexCache"
);

index_cache!(
    /// `source_scoped_top_level_bindings` bucketed by `reference_source`, built on
    /// the first scoped lookup. A finished table still gains bindings when an
    /// extension begins or a syntax-only import is registered, and both clear
    /// the index; equality ignores it.
    BindingSourceIndexCache,
    BindingSourceIndex,
    "BindingSourceIndexCache"
);

index_cache!(
    /// `display_path("::")` per root child and module symbol, snapshotted on the
    /// first module-qualified or imported-path lookup. Handles minted after the
    /// snapshot miss the map and fall back to a walk. Every roster mutation
    /// clears it alongside `root_names`; equality ignores it.
    ModulePathIndexCache,
    ModulePathIndex,
    "ModulePathIndexCache"
);

index_cache!(
    /// The module-qualified roster index is built on the first
    /// `find_module_qualified_reference` and cleared wherever `module_path_index`
    /// or `root_names` clears; equality ignores it.
    ModuleScopeIndexCache,
    ModuleScopeIndex,
    "ModuleScopeIndexCache"
);
