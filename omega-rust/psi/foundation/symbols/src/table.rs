//! Start here. The symbol table: the hierarchy arena scoped lookup walks,
//! its builder, appender and extension.

use std::sync::Arc;

use arena::{Arena, HandleSpan, HierarchyArena, SpanHandles};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceFile, SourceId, SourceMap, SourceOrigin, SourceSpan};

use super::builtin::BUILTIN_TYPE_COUNT;
mod child_batches;
mod indices;
use indices::{
    BindingSourceIndex, BindingSourceIndexCache, ModulePathIndex, ModulePathIndexCache,
    ModuleScopeIndex, ModuleScopeIndexCache, RootNameIndex, RootNameIndexCache,
};
mod modules;

pub use modules::{JoinedPath, same_semantic_name};
mod source_queries;
#[cfg(test)]
mod tests;
use super::{
    BuiltinFunction, BuiltinType, BuiltinTypeAtom, BuiltinTypeMember, FixedCoreDeclaration, Symbol,
    SymbolHandle, SymbolKind, SymbolName, SymbolNameRef, SymbolPath,
};
use child_batches::{SupplementalChildHandles, SupplementalChildren};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTable {
    symbols: HierarchyArena<Symbol>,
    names: Arena<SymbolName>,
    path_members: Arena<SymbolHandle>,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    root: SymbolHandle,
    supplemental_children: SupplementalChildren,
    module_symbols: Arena<SymbolHandle>,
    /// Dense `SourceId`-indexed map from a source to its declared module:
    /// a linear scan per lookup would cost `O(#sources)` on every
    /// module-qualified name walk.
    source_module_index: Vec<SymbolHandle>,
    /// Declaration membership at the last activation frontier. Later allocations
    /// belong to the new cohort. Selecting a source again does not resurrect
    /// an older declaration that already left its predecessor's view.
    inherited_symbols: Vec<bool>,
    root_names: RootNameIndexCache,
    /// `source_scoped_top_level_bindings` bucketed by reference source,
    /// because three lookup paths scanned the whole binding list per query.
    /// Cleared wherever bindings are appended after `finish`.
    binding_source_index: BindingSourceIndexCache,
    /// `display_path("::")` snapshot for the top-level and module roster,
    /// because module-qualified lookups walked every candidate's parent
    /// chain -- allocating a `String` per candidate per reference. Cleared
    /// alongside the other roster indexes.
    module_path_index: ModulePathIndexCache,
    /// The top-level + module roster bucketed by full name, display path,
    /// and (for Domain kinds) declared leaf, so `find_module_qualified_reference`
    /// consults only name- or path-bearing candidates instead of walking the
    /// roster per reference. Cleared alongside `module_path_index`.
    module_scope_index: ModuleScopeIndexCache,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTableBuilder {
    symbols: HierarchyArena<Symbol>,
    names: Arena<SymbolName>,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    root: SymbolHandle,
}

/// One compiler-owned top-level vocabulary binding for an exact source.
///
/// The authored spelling remains unchanged. Only references originating in
/// `reference_source` select the same-spelled declaration authored in
/// `declaration_source`; every other source retains ordinary lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceScopedTopLevelBinding {
    reference_source: SourceId,
    declaration_source: SourceId,
    name: Arc<str>,
    module_import: Option<modules::ModuleImport>,
    producer: BindingProducer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingProducer {
    Loader,
    AuthoredImport,
}

impl SourceScopedTopLevelBinding {
    pub fn new(
        reference_source: SourceId,
        declaration_source: SourceId,
        name: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            reference_source,
            declaration_source,
            name: name.into(),
            module_import: None,
            producer: BindingProducer::Loader,
        }
    }

    /// The source whose top-level references this binding scopes.
    pub const fn reference_source(&self) -> SourceId {
        self.reference_source
    }
}

/// Outcome after declaration eligibility and source lookup precedence.
/// Ambiguity must remain distinct from absence when composing static names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolLookup {
    NotFound,
    Unique(SymbolHandle),
    Ambiguous {
        first: SymbolHandle,
        second: SymbolHandle,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TopLevelSelection {
    Ordinary,
    SignatureFree,
}

impl SymbolLookup {
    pub fn unique(self) -> Option<SymbolHandle> {
        match self {
            Self::Unique(symbol) => Some(symbol),
            Self::NotFound | Self::Ambiguous { .. } => None,
        }
    }

    fn from_candidates(mut candidates: impl Iterator<Item = SymbolHandle>) -> Self {
        let Some(first) = candidates.next() else {
            return Self::NotFound;
        };
        if let Some(second) = candidates.find(|candidate| *candidate != first) {
            Self::Ambiguous { first, second }
        } else {
            Self::Unique(first)
        }
    }
}

impl SymbolTableBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sources(sources: Option<Arc<SourceMap>>) -> Self {
        Self {
            sources,
            ..Self::default()
        }
    }

    pub fn with_sources_and_top_level_bindings(
        sources: Option<Arc<SourceMap>>,
        source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    ) -> Self {
        Self {
            sources,
            source_scoped_top_level_bindings,
            ..Self::default()
        }
    }

    pub fn insert_root(&mut self, kind: SymbolKind, name: SymbolNameRef<'_>) -> SymbolHandle {
        let symbol = self.symbol_from_parts(SymbolHandle::invalid(), kind, name);
        let root = self.symbols.insert_root(symbol);
        self.root = root;

        root
    }

    pub fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        let children = children
            .into_iter()
            .map(|(kind, name)| self.symbol_from_parts(parent, kind, name))
            .collect::<Vec<_>>();
        self.symbols.insert_children(parent, children)
    }

    pub fn child_handles(span: HandleSpan<Symbol>) -> impl Iterator<Item = SymbolHandle> {
        span.handles()
    }

    pub fn finish(self) -> SymbolTable {
        SymbolTable {
            symbols: self.symbols,
            names: self.names,
            path_members: Arena::new(),
            sources: self.sources,
            source_scoped_top_level_bindings: self.source_scoped_top_level_bindings,
            root: self.root,
            supplemental_children: SupplementalChildren::default(),
            module_symbols: Arena::new(),
            source_module_index: Vec::new(),
            inherited_symbols: Vec::new(),
            root_names: RootNameIndexCache::default(),
            binding_source_index: BindingSourceIndexCache::default(),
            module_path_index: ModulePathIndexCache::default(),
            module_scope_index: ModuleScopeIndexCache::default(),
        }
    }

    fn symbol_from_parts(
        &mut self,
        parent: SymbolHandle,
        kind: SymbolKind,
        name: SymbolNameRef<'_>,
    ) -> Symbol {
        child_symbol(&mut self.names, self.sources.as_deref(), parent, kind, name)
    }
}

fn child_symbol(
    names: &mut Arena<SymbolName>,
    sources: Option<&SourceMap>,
    parent: SymbolHandle,
    kind: SymbolKind,
    name: SymbolNameRef<'_>,
) -> Symbol {
    Symbol {
        parent,
        children: HandleSpan::empty(),
        kind,
        name: names.insert(SymbolName::from_ref_with_sources(name, sources)),
        generated_from: SymbolHandle::invalid(),
    }
}

/// Common append surface used while first constructing a symbol hierarchy and
/// while extending one retained hierarchy. Both routes preserve contiguous
/// child ranges; only top-level insertion differs.
pub trait SymbolTableAppender {
    fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol>;
}

impl SymbolTableAppender for SymbolTableBuilder {
    fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        SymbolTableBuilder::insert_children(self, parent, children)
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this table retains `base` exactly and appends only generated
    /// declarations with earlier derivation origins. A later frontend may
    /// adopt this allocation frontier before minting authored body symbols;
    /// this does not turn generated declarations into authored source roots.
    pub fn is_generated_extension_of(&self, base: &Self) -> bool {
        // Lookup caches are derived from the retained semantic fields. Their
        // initialization state is not source or allocation custody.
        let Self {
            symbols,
            names,
            path_members,
            sources,
            source_scoped_top_level_bindings,
            root,
            supplemental_children,
            module_symbols,
            source_module_index,
            inherited_symbols,
            root_names: _,
            binding_source_index: _,
            module_path_index: _,
            module_scope_index: _,
        } = self;
        if symbols.len() < base.symbols.len()
            || !symbols
                .nodes()
                .iter()
                .take(base.symbols.len())
                .eq(base.symbols.nodes().iter())
            || names.len() < base.names.len()
            || !names.iter().take(base.names.len()).eq(base.names.iter())
            || path_members.len() < base.path_members.len()
            || !path_members
                .iter()
                .take(base.path_members.len())
                .eq(base.path_members.iter())
            || sources != &base.sources
            || source_scoped_top_level_bindings != &base.source_scoped_top_level_bindings
            || root != &base.root
            || module_symbols != &base.module_symbols
            || source_module_index != &base.source_module_index
            || inherited_symbols != &base.inherited_symbols
        {
            return false;
        }
        // Late generated locals extend an existing lexical scope through the
        // supplemental index. Retain its exact old prefix; only newly allocated
        // children with the same parent may follow it. The loop below verifies
        // every new child's derivation origin independently.
        for (parent, _) in symbols.nodes().iter() {
            let mut children = supplemental_children.children(parent);
            if base
                .supplemental_children
                .children(parent)
                .any(|retained| children.next() != Some(retained))
                || children.any(|child| {
                    child.arena_index() as usize <= base.symbols.len()
                        || symbols.get(child).parent != parent
                })
            {
                return false;
            }
        }
        for (handle, symbol) in symbols.nodes().iter().skip(base.symbols.len()) {
            // Origins precede their products, so this also rejects cycles and
            // foreign generations without following an untrusted parent chain.
            let origin = symbol.generated_from;
            if !origin.is_valid()
                || origin.generation() != handle.generation()
                || origin.arena_index() >= handle.arena_index()
                || symbols.get(origin).kind == SymbolKind::Unknown
                || symbol.kind == SymbolKind::Unknown
                || !names.is_valid(symbol.name)
                || names.get(symbol.name).source_span().is_some()
            {
                return false;
            }
            if symbol.parent.is_valid() {
                if symbol.parent.generation() != handle.generation()
                    || symbol.parent.arena_index() >= handle.arena_index()
                {
                    return false;
                }
                let siblings = symbols.get(symbol.parent).children;
                let in_primary = siblings.start().generation() == handle.generation()
                    && handle
                        .arena_index()
                        .checked_sub(siblings.start().arena_index())
                        .is_some_and(|offset| offset < siblings.count());
                // Primary membership remains constant time for large cloned
                // scopes. Only late supplemental locals need an index walk.
                let supplemental_count = supplemental_children
                    .children(symbol.parent)
                    .filter(|&child| child == handle)
                    .count();
                if usize::from(in_primary) + supplemental_count != 1 {
                    return false;
                }
            }
            let Some(mut children) = self.child_handles(handle) else {
                return false;
            };
            if children.any(|child| {
                child.arena_index() <= handle.arena_index() || symbols.get(child).parent != handle
            }) {
                return false;
            }
        }
        path_members
            .iter()
            .skip(base.path_members.len())
            .all(|(_, symbol)| {
                symbol.is_valid() && symbols.get(*symbol).kind != SymbolKind::Unknown
            })
    }

    /// Append an authored child batch without replacing the parent's original
    /// children or rebinding any existing handle. Lookup visits the original
    /// declaration children first, then appended batches in insertion order.
    /// This is also the route for body locals and synthesized state headers.
    pub fn append_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        self.append_children_from(
            parent,
            children
                .into_iter()
                .map(|(kind, name)| (SymbolHandle::invalid(), kind, name)),
        )
    }

    /// Append generated locals to an existing lexical scope while preserving
    /// both its frozen child span and each new child's derivation origin.
    pub fn append_generated_children_from<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolHandle, SymbolKind, &'name str)>,
    ) -> HandleSpan<Symbol> {
        let children = children.into_iter().collect::<Vec<_>>();
        assert!(
            children.iter().all(|(origin, _, _)| origin.is_valid()
                && self.symbols.get(*origin).kind != SymbolKind::Unknown),
            "compiler-generated symbols require existing derivation origins"
        );
        self.append_children_from(
            parent,
            children
                .into_iter()
                .map(|(origin, kind, name)| (origin, kind, SymbolNameRef::Borrowed(name))),
        )
    }

    fn append_children_from<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolHandle, SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        // Validate the full generational handle before indexing its dense slot.
        let _ = self.get(parent);
        let mut appended = HandleSpan::empty();
        for (generated_from, kind, name) in children {
            let name = self.names.insert(SymbolName::from_ref_with_sources(
                name,
                self.sources.as_deref(),
            ));
            let child = self.symbols.insert_supplemental_child(
                parent,
                Symbol {
                    parent,
                    children: HandleSpan::empty(),
                    kind,
                    name,
                    generated_from,
                },
            );
            appended.push_contiguous(child);
        }
        if appended.is_empty() {
            return appended;
        }
        self.supplemental_children.append(parent, appended);
        self.root_names.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        appended
    }

    #[inline]
    pub fn get(&self, symbol: SymbolHandle) -> &Symbol {
        self.symbols.get(symbol)
    }

    /// Give one symbol a new spelling. Target-family selection promotes the
    /// realized target's body to the family path and demotes the body it
    /// replaces to its `<path>::<target>` spelling; handles are unchanged.
    /// The declaration's source span stays attached to the new spelling, so
    /// provenance and diagnostics are unchanged.
    pub fn rename(&mut self, symbol: SymbolHandle, name: &str) {
        let name = match self.names.get(self.get(symbol).name).source_span() {
            Some(source_span) => {
                self.names
                    .insert(SymbolName::from_ref(SymbolNameRef::OwnedSource {
                        value: name,
                        source_span,
                    }))
            }
            None => self
                .names
                .insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name))),
        };
        self.symbols.get_mut(symbol).name = name;
        self.module_scope_index.clear();
        self.root_names.clear();
    }

    /// Start an append-only extension of an already resolved symbol table.
    /// Existing symbol handles and child ranges remain unchanged.
    pub fn begin_extension(
        mut self,
        sources: Option<Arc<SourceMap>>,
        additional_source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    ) -> SymbolTableExtension {
        self.sources = sources;
        self.source_scoped_top_level_bindings
            .extend(additional_source_scoped_top_level_bindings);
        self.binding_source_index.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        SymbolTableExtension { table: self }
    }

    /// Start a new compilation view over retained declaration coordinates.
    /// Unlike an extension within one activation, its complete loader-supplied
    /// loader import roster replaces historical loader bindings, even when both
    /// endpoints remain selected for unrelated imports. Selected declarations
    /// retain their own authored imports; backing is not loader authority.
    pub fn begin_source_activation(
        mut self,
        sources: Arc<SourceMap>,
        source_scoped_top_level_bindings: Vec<SourceScopedTopLevelBinding>,
    ) -> Result<SymbolTableExtension, String> {
        if self
            .sources
            .as_deref()
            .is_some_and(|previous| !sources.retains_source_storage(previous))
        {
            return Err(String::from(
                "source activation changed retained source coordinates",
            ));
        }
        let mut inherited = vec![false; self.symbols.len() + 1];
        for (handle, _) in self.symbols.nodes().iter() {
            inherited[handle.arena_index() as usize] = self.symbol_source_is_selected(handle)
                && self
                    .provenance_source_span(handle)
                    .is_none_or(|span| sources.source_is_selected(span.source_id));
        }
        if let Some(previous) = self.sources.as_deref() {
            for (position, module) in self.source_module_index.iter_mut().enumerate() {
                let source = SourceId(position);
                if sources.source_is_selected(source) && !previous.source_is_selected(source) {
                    // This file is a new declaration cohort, not a second module
                    // declaration in the old cohort. Its parsed module will join
                    // the existing logical namespace without reviving old members.
                    *module = SymbolHandle::invalid();
                }
            }
        }
        let authored_imports = self
            .source_scoped_top_level_bindings
            .iter()
            .filter(|binding| {
                binding.producer == BindingProducer::AuthoredImport
                    && sources.source_is_selected(binding.reference_source)
            })
            .cloned()
            .collect::<Vec<_>>();
        self.inherited_symbols = inherited;
        self.sources = Some(sources);
        self.source_scoped_top_level_bindings = source_scoped_top_level_bindings;
        self.binding_source_index.clear();
        self.root_names.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        // Syntax imports belong to retained declarations. Loader rows belong
        // to the current request and were completely replaced above. Reinsert
        // through their producer so a current certified import takes precedence.
        for binding in authored_imports {
            if let Some(import) = &binding.module_import {
                self.register_source_import(binding.reference_source, &import.path);
            }
        }
        Ok(SymbolTableExtension { table: self })
    }

    /// Mint a compiler-generated root declaration after symbol resolution.
    /// The generated name is owned by the table and authored symbol handles
    /// remain unchanged. Children must be installed once, as one batch, via
    /// [`Self::insert_generated_children`].
    pub fn insert_generated_root_from(
        &mut self,
        generated_from: SymbolHandle,
        kind: SymbolKind,
        name: &str,
    ) -> SymbolHandle {
        assert!(
            generated_from.is_valid()
                && self.symbols.get(generated_from).kind != SymbolKind::Unknown,
            "compiler-generated symbols require one existing derivation origin"
        );
        self.root_names.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        let name = self
            .names
            .insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name)));
        self.symbols.insert_root(Symbol {
            parent: SymbolHandle::invalid(),
            children: HandleSpan::empty(),
            kind,
            name,
            generated_from,
        })
    }

    /// Mint the complete child range of a freshly generated symbol. Keeping
    /// the batch contiguous preserves the hierarchy arena's compact child-span
    /// representation without allowing late mutation of authored parents.
    pub fn insert_generated_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, &'name str)>,
    ) -> HandleSpan<Symbol> {
        self.insert_generated_children_from(
            parent,
            children
                .into_iter()
                .map(|(kind, name)| (parent, kind, name)),
        )
    }

    /// Mint one contiguous child range while retaining each child's exact
    /// existing derivation origin. Lexical parentage still belongs to `parent`;
    /// specialization provenance belongs to the corresponding source child.
    pub fn insert_generated_children_from<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolHandle, SymbolKind, &'name str)>,
    ) -> HandleSpan<Symbol> {
        let children = children.into_iter().collect::<Vec<_>>();
        assert!(
            children.iter().all(|(origin, _, _)| origin.is_valid()
                && self.symbols.get(*origin).kind != SymbolKind::Unknown),
            "compiler-generated symbols require existing derivation origins"
        );
        self.root_names.clear();
        self.module_path_index.clear();
        self.module_scope_index.clear();
        let names = &mut self.names;
        self.symbols.insert_children(
            parent,
            children
                .into_iter()
                .map(|(generated_from, kind, name)| Symbol {
                    parent,
                    children: HandleSpan::empty(),
                    kind,
                    name: names.insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name))),
                    generated_from,
                }),
        )
    }

    #[inline]
    pub fn name(&self, symbol: SymbolHandle) -> &str {
        let symbol = self.get(symbol);

        self.names.get(symbol.name).as_str(self.sources.as_deref())
    }

    #[inline]
    pub fn root(&self) -> SymbolHandle {
        self.root
    }

    #[inline]
    pub fn child_handles(&self, parent: SymbolHandle) -> Option<SymbolChildHandles<'_>> {
        if self.get(parent).kind == SymbolKind::Module {
            return Some(SymbolChildHandles {
                contiguous: None,
                supplemental: self.supplemental_children.children(parent),
                namespace: self.module_children(parent).into_iter(),
            });
        }
        Some(SymbolChildHandles {
            contiguous: Some(self.symbols.child_handles(parent)?),
            supplemental: self.supplemental_children.children(parent),
            namespace: Vec::new().into_iter(),
        })
    }

    /// Root-level children whose full name is exactly `name`, in the order
    /// `child_handles(root())` yields them, probed through the root name
    /// index rather than a scan of the whole root roster.
    pub fn root_children_named(&self, name: &str) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.root_names
            .0
            .get_or_init(|| self.root_name_index())
            .by_name
            .get(name)
            .into_iter()
            .flatten()
            .map(|&(_, symbol)| symbol)
            .filter(|symbol| self.symbol_source_is_selected(*symbol))
    }

    pub fn find_child_by_name(&self, parent: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        if !parent.is_valid() {
            return None;
        }

        let mut candidates = self
            .child_handles(parent)?
            .filter(|symbol| self.name(*symbol) == name && self.symbol_source_is_selected(*symbol));
        let first = candidates.next()?;
        if self.get(parent).kind == SymbolKind::Module && candidates.next().is_some() {
            return None;
        }
        Some(first)
    }

    pub fn find_child_by_name_and_kind(
        &self,
        parent: SymbolHandle,
        name: &str,
        kind: SymbolKind,
    ) -> Option<SymbolHandle> {
        if !parent.is_valid() {
            return None;
        }

        let mut candidates = self.child_handles(parent)?.filter(|symbol| {
            self.get(*symbol).kind == kind
                && self.name(*symbol) == name
                && self.symbol_source_is_selected(*symbol)
        });
        let first = candidates.next()?;
        if self.get(parent).kind == SymbolKind::Module && candidates.next().is_some() {
            return None;
        }
        Some(first)
    }

    /// Resolve one source-backed top-level reference without turning a
    /// presentation spelling into global authority.
    pub fn find_top_level_by_name_and_kinds_from_source(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
    ) -> Option<SymbolHandle> {
        self.find_top_level_by_name_and_kinds_from_source_matching(name, kinds, reference, |_| true)
    }

    /// Apply declaration eligibility before source-scoped binding and lookup
    /// precedence. In particular, a receiver's nominal owner can exclude a
    /// same-spelled method without hiding another eligible declaration.
    pub fn find_top_level_by_name_and_kinds_from_source_matching(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        matches_candidate: impl FnMut(SymbolHandle) -> bool,
    ) -> Option<SymbolHandle> {
        self.lookup_top_level_by_name_and_kinds_from_source_matching(
            name,
            kinds,
            reference,
            matches_candidate,
        )
        .unique()
    }

    /// Retain ambiguity when a caller composes multiple declaration meanings.
    /// Eligibility still precedes the ordinary source and namespace precedence.
    pub fn lookup_top_level_by_name_and_kinds_from_source_matching(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        matches_candidate: impl FnMut(SymbolHandle) -> bool,
    ) -> SymbolLookup {
        self.lookup_top_level(
            name,
            kinds,
            reference,
            matches_candidate,
            TopLevelSelection::Ordinary,
        )
    }

    /// Apply ordinary namespace selection without choosing among declarations
    /// that require a call signature to distinguish. Flat source forests retain
    /// the occurrence's package scope, and synthetic references retain the whole
    /// eligible pool; neither may silently select its first declaration.
    pub fn lookup_signature_free_top_level_from_source_matching(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        matches_candidate: impl FnMut(SymbolHandle) -> bool,
    ) -> SymbolLookup {
        self.lookup_top_level(
            name,
            kinds,
            reference,
            matches_candidate,
            TopLevelSelection::SignatureFree,
        )
    }

    /// Root-level children whose full name or leaf name selects `name`,
    /// in declaration order. The whole-roster filter behind every top-level
    /// lookup is O(children) per call; bucketing once per table keeps the
    /// same candidate pool without rescanning.
    fn top_level_children_matching_name(
        &self,
        name: &str,
        include_leaf: bool,
    ) -> Vec<SymbolHandle> {
        let index = self.root_names.0.get_or_init(|| self.root_name_index());
        let mut merged = index.by_name.get(name).cloned().unwrap_or_default();
        if include_leaf && let Some(leaves) = index.by_leaf.get(name) {
            merged.extend_from_slice(leaves);
        }
        merged.sort_unstable_by_key(|(position, _)| *position);
        merged.dedup_by_key(|(_, symbol)| *symbol);
        merged.into_iter().map(|(_, symbol)| symbol).collect()
    }

    /// The bindings visible to `reference_source`, in binding order, via the
    /// per-source index instead of a whole-list scan per lookup. Activation
    /// is projected once here: lookup and collision consumers cannot recover
    /// authority from a historical row by forgetting their own filter.
    pub(super) fn source_scoped_bindings_for(
        &self,
        reference_source: SourceId,
    ) -> &[SourceScopedTopLevelBinding] {
        const EMPTY: &[SourceScopedTopLevelBinding] = &[];
        let index = self.binding_source_index.0.get_or_init(|| {
            let mut index = BindingSourceIndex::default();
            for binding in &self.source_scoped_top_level_bindings {
                if !self.source_binding_is_selected(binding) {
                    continue;
                }
                index
                    .by_source
                    .entry(binding.reference_source)
                    .or_default()
                    .push(binding.clone());
            }
            index
        });
        index
            .by_source
            .get(&reference_source)
            .map_or(EMPTY, Vec::as_slice)
    }

    fn root_name_index(&self) -> RootNameIndex {
        let mut index = RootNameIndex::default();
        if let Some(children) = self.child_handles(self.root) {
            for (position, symbol) in children.enumerate() {
                let full_name = self.name(symbol);
                index
                    .by_name
                    .entry(full_name.into())
                    .or_default()
                    .push((position as u32, symbol));
                if let Some((_, leaf)) = full_name.rsplit_once("::") {
                    index
                        .by_leaf
                        .entry(leaf.into())
                        .or_default()
                        .push((position as u32, symbol));
                }
            }
        }
        index
    }

    /// The `find_module_qualified_reference` candidate roster — root
    /// children then module symbols, in the same order — bucketed by every
    /// key the match arms can select on. Built once per table state.
    fn module_scope_index(&self) -> &ModuleScopeIndex {
        self.module_scope_index.0.get_or_init(|| {
            let mut index = ModuleScopeIndex::default();
            let candidates = self
                .child_handles(self.root)
                .into_iter()
                .flatten()
                .chain(self.module_symbols.iter().map(|(_, handle)| *handle));
            for (order, candidate) in (0u32..).zip(candidates) {
                let name = self.name(candidate);
                index
                    .by_name
                    .entry(name.into())
                    .or_default()
                    .push((order, candidate));
                let path = self.indexed_display_path(candidate);
                index
                    .by_path
                    .entry(path.as_ref().into())
                    .or_default()
                    .push((order, candidate));
                if self.get(candidate).kind == SymbolKind::Domain
                    && let Some((_, leaf)) = name.rsplit_once("::")
                {
                    index
                        .domain_leaves
                        .entry(leaf.into())
                        .or_default()
                        .push((order, candidate));
                }
            }
            index
        })
    }

    fn lookup_top_level(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
        mut matches_candidate: impl FnMut(SymbolHandle) -> bool,
        selection: TopLevelSelection,
    ) -> SymbolLookup {
        let reference_is_source_backed = reference.span.start != reference.span.end;
        if reference_is_source_backed
            && self
                .sources
                .as_deref()
                .is_some_and(|sources| !sources.source_is_selected(reference.source_id))
        {
            return SymbolLookup::NotFound;
        }
        // A dummy span may retain a real SourceId, but it has no lexical
        // occurrence. Do not let that id authorize a relative module path.
        if (reference_is_source_backed || selection == TopLevelSelection::Ordinary)
            && let Some(result) =
                self.find_module_qualified_reference(name, kinds, reference, &mut matches_candidate)
        {
            return result;
        }
        if self.child_handles(self.root).is_none() {
            return SymbolLookup::NotFound;
        }
        let candidates = self
            .top_level_children_matching_name(
                name,
                reference_is_source_backed && self.has_namespace_context(reference),
            )
            .into_iter()
            .filter(|symbol| {
                kinds.contains(&self.get(*symbol).kind)
                    && matches_candidate(*symbol)
                    && self.source_reference_can_see_symbol(reference, *symbol)
            })
            .collect::<Vec<_>>();

        if !reference_is_source_backed {
            if selection == TopLevelSelection::SignatureFree {
                return SymbolLookup::from_candidates(candidates.into_iter());
            }
            return candidates
                .first()
                .copied()
                .map_or(SymbolLookup::NotFound, SymbolLookup::Unique);
        }

        if let Some(binding) = self
            .source_scoped_bindings_for(reference.source_id)
            .iter()
            .find(|binding| binding.module_import.is_none() && binding.name.as_ref() == name)
        {
            let targets = candidates.iter().copied().filter(|symbol| {
                self.name(*symbol) == name
                    && self
                        .symbol_source_span(*symbol)
                        .is_some_and(|span| span.source_id == binding.declaration_source)
            });
            return SymbolLookup::from_candidates(targets);
        }

        if self.has_namespace_context(reference) {
            return self.select_namespace_candidate(&candidates, name, reference);
        }
        if selection == TopLevelSelection::SignatureFree {
            let local =
                SymbolLookup::from_candidates(candidates.iter().copied().filter(|symbol| {
                    self.symbol_provenance_source_span(*symbol)
                        .is_some_and(|declaration| self.same_source_package(reference, declaration))
                }));
            return match local {
                SymbolLookup::NotFound => SymbolLookup::from_candidates(candidates.into_iter()),
                _ => local,
            };
        }
        candidates
            .iter()
            .copied()
            .find(|symbol| {
                self.symbol_source_span(*symbol)
                    .is_some_and(|span| span.source_id == reference.source_id)
            })
            // Two checked instances of one path each carry the same spelled
            // candidates; a reference selects only its own instance — but
            // only among candidates its stratum can already see.
            .or_else(|| {
                self.sources.as_deref().and_then(|sources| {
                    candidates.iter().copied().find(|symbol| {
                        self.symbol_source_span(*symbol).is_some_and(|declaration| {
                            !sources.resolution_strata_separate(reference, declaration)
                                && sources.same_checked_instance(reference, declaration)
                        })
                    })
                })
            })
            .or_else(|| {
                self.sources.as_deref().and_then(|sources| {
                    candidates.iter().copied().find(|symbol| {
                        self.symbol_source_span(*symbol).is_some_and(|declaration| {
                            !sources.resolution_strata_separate(reference, declaration)
                        })
                    })
                })
            })
            .or_else(|| candidates.first().copied())
            .map_or(SymbolLookup::NotFound, SymbolLookup::Unique)
    }

    /// Resolve either one exact declaration or one source-scoped overloaded
    /// operator family. The returned operator is only a representative;
    /// typed consumers must reconstruct and validate the complete same-path
    /// family before granting it authority.
    pub fn find_top_level_declaration_or_operator_family_from_source(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        reference: SourceSpan,
    ) -> Option<SymbolHandle> {
        if let Some(exact) =
            self.find_top_level_by_name_and_kinds_from_source(name, kinds, reference)
        {
            return Some(exact);
        }
        if reference.span.start == reference.span.end {
            return None;
        }
        let binding = self
            .source_scoped_bindings_for(reference.source_id)
            .iter()
            .find(|binding| binding.module_import.is_none() && binding.name.as_ref() == name)?;
        let mut family = self.child_handles(self.root)?.filter(|symbol| {
            self.get(*symbol).kind == SymbolKind::Operator
                && kinds.contains(&SymbolKind::Operator)
                && self.name(*symbol) == name
                && self.source_reference_can_see_symbol(reference, *symbol)
                && self
                    .symbol_source_span(*symbol)
                    .is_some_and(|span| span.source_id == binding.declaration_source)
        });
        let representative = family.next()?;
        family.next().map(|_| representative)
    }

    /// Whether two declarations occupy separate source-resolution contexts
    /// through the activation stratum or an explicit same-name binding.
    pub fn source_scopes_separate(&self, left: SymbolHandle, right: SymbolHandle) -> bool {
        // Specialization names are diagnostic payload, not a new source
        // namespace. Apply the authored declarations' existing scope rules.
        let left = self.authored_origin(left);
        let right = self.authored_origin(right);
        let left_parent = self.get(left).parent;
        let right_parent = self.get(right).parent;
        if left_parent != right_parent
            && (self.get(left_parent).kind == SymbolKind::Machine
                || self.get(right_parent).kind == SymbolKind::Machine)
        {
            // Lexical machine scopes stay private even inside the same file.
            // Package imports never expose another invocation's declarations.
            return true;
        }
        let (Some(left_span), Some(right_span)) = (
            self.symbol_source_span(left),
            self.symbol_source_span(right),
        ) else {
            return false;
        };
        if left_span.source_id == right_span.source_id {
            return false;
        }
        // Two checked instances of one package — including two instances of
        // the exact same file — never share a resolution scope, module-
        // declared or not.
        if self.sources.as_deref().is_some_and(|sources| {
            sources.same_package(left_span, right_span)
                && !sources.same_checked_instance(left_span, right_span)
        }) {
            return true;
        }
        // A package boundary is a resolution-scope boundary: declarations
        // in distinct packages collide only where an authored import edge
        // exposes the name into the other file's scope, since imports do
        // not shadow authored declarations.
        if let Some(sources) = self.sources.as_deref()
            && sources.file_at(left_span).is_some()
            && sources.file_at(right_span).is_some()
            && !sources.same_package(left_span, right_span)
        {
            return !(self.import_exposes_symbol(left_span.source_id, right)
                || self.import_exposes_symbol(right_span.source_id, left));
        }
        if self.source_module(left_span.source_id) != self.source_module(right_span.source_id) {
            return true;
        }
        if self
            .sources
            .as_deref()
            .is_some_and(|sources| sources.resolution_strata_separate(left_span, right_span))
        {
            return true;
        }
        let name = self.name(left);
        if name != self.name(right) {
            return false;
        }
        self.source_scoped_top_level_bindings.iter().any(|binding| {
            self.source_binding_is_selected(binding)
                && binding.module_import.is_none()
                && binding.name.as_ref() == name
                && (binding.declaration_source == left_span.source_id
                    || binding.declaration_source == right_span.source_id)
        })
    }

    /// Whether a source-backed reference may use this exact declaration under
    /// checked-purpose and current activation resolution-stratum boundaries.
    ///
    /// Source-free consumers retain their existing behavior. Compiler-generated
    /// symbols inherit the visibility of their exact authored derivation.
    pub fn source_reference_can_see_symbol(
        &self,
        reference: SourceSpan,
        symbol: SymbolHandle,
    ) -> bool {
        let Some(sources) = self.sources.as_deref() else {
            return true;
        };
        if self.get(symbol).kind != SymbolKind::Module && !self.inherited_symbol_is_selected(symbol)
        {
            return false;
        }
        let Some(declaration) = self.provenance_source_span(symbol) else {
            return true;
        };
        if self.get(symbol).kind == SymbolKind::Module {
            // A logical namespace can have active contributors even when its
            // first declaration belongs to retained, inactive backing.
            return self.module_source_ids(symbol).any(|source| {
                sources.reference_can_see_declaration(
                    reference,
                    SourceSpan::new(source, source::Span::default()),
                )
            });
        }
        sources.reference_can_see_declaration(reference, declaration)
    }

    /// Current declaration membership; source-free vocabulary remains present.
    /// Logical modules are present while any selected source contributes to them.
    fn symbol_source_is_selected(&self, symbol: SymbolHandle) -> bool {
        let Some(sources) = self.sources.as_deref() else {
            return true;
        };
        if self.get(symbol).kind == SymbolKind::Module {
            return self.provenance_source_span(symbol).is_none()
                || self.module_source_ids(symbol).next().is_some();
        }
        if !self.inherited_symbol_is_selected(symbol) {
            return false;
        }
        let Some(declaration) = self.provenance_source_span(symbol) else {
            return true;
        };
        sources.source_is_selected(declaration.source_id)
    }

    fn inherited_symbol_is_selected(&self, symbol: SymbolHandle) -> bool {
        self.inherited_symbols
            .get(symbol.arena_index() as usize)
            .copied()
            .unwrap_or(true)
    }

    /// Whether a source-backed reference may select this declaration as a
    /// candidate: the declaration is authored in the reference's own source,
    /// in the same logical module of its checked package instance, or in a
    /// source the reference's file imported. A declaration is never a candidate
    /// merely because its source was loaded (modules spec, "Import scope and
    /// exposure"): every target's entry contract joins every compilation, and
    /// the bundled spellings and names it pulls in must not change the
    /// resolution of a program that never imported them. Source-free
    /// references and symbols without authored provenance keep the whole
    /// program, as `source_reference_can_see_symbol` does.
    pub fn source_reference_may_select_symbol(
        &self,
        reference: SourceSpan,
        symbol: SymbolHandle,
    ) -> bool {
        if !self.source_reference_can_see_symbol(reference, symbol) {
            return false;
        }
        let Some(sources) = self.sources.as_deref() else {
            return true;
        };
        let Some(declaration) = self.provenance_source_span(symbol) else {
            return true;
        };
        declaration.source_id == reference.source_id
            || (sources.same_checked_instance(reference, declaration)
                && self.source_module(reference.source_id)
                    == self.source_module(declaration.source_id))
            || self
                .source_scoped_bindings_for(reference.source_id)
                .iter()
                .any(|binding| binding.declaration_source == declaration.source_id)
    }

    pub fn find_descendant_by_path<'name>(
        &self,
        root: SymbolHandle,
        names: impl IntoIterator<Item = &'name str>,
    ) -> Option<SymbolHandle> {
        if !root.is_valid() {
            return None;
        }

        let mut current = root;

        for name in names {
            current = self.find_child_by_name(current, name)?;
        }

        Some(current)
    }

    pub fn builtin_function_symbol(&self, function: BuiltinFunction) -> Option<SymbolHandle> {
        let builtin_offset = BUILTIN_TYPE_COUNT.checked_add(function.ordinal())?;
        self.child_handles(self.root)?
            .nth(builtin_offset)
            .filter(|symbol| self.get(*symbol).kind == SymbolKind::BuiltinFunction)
    }

    /// Classify only an exact compiler-installed root builtin-function slot.
    /// Names are deliberately irrelevant: same-spelled package declarations
    /// and generated source-free symbols are not compiler functions.
    pub fn builtin_function_for_symbol(&self, symbol: SymbolHandle) -> Option<BuiltinFunction> {
        if !symbol.is_valid() || self.get(symbol).kind != SymbolKind::BuiltinFunction {
            return None;
        }
        self.root_slot_ordinal(symbol, BUILTIN_TYPE_COUNT, BuiltinFunction::COUNT)
            .and_then(BuiltinFunction::from_ordinal)
    }

    /// The root child installed for `declaration`, if this table carries the
    /// fixed `core::*` vocabulary at its canonical slots.
    pub fn fixed_core_declaration_symbol(
        &self,
        declaration: FixedCoreDeclaration,
    ) -> Option<SymbolHandle> {
        let offset = BUILTIN_TYPE_COUNT
            .checked_add(BuiltinFunction::COUNT)?
            .checked_add(declaration.ordinal())?;
        self.child_handles(self.root)?
            .nth(offset)
            .filter(|symbol| self.get(*symbol).kind == SymbolKind::MathematicalDefinition)
    }

    /// The member's compiler-installed slot beneath its exact builtin owner.
    /// A package member with the same spelling never supplies this identity.
    pub fn builtin_type_member_symbol(&self, member: BuiltinTypeMember) -> Option<SymbolHandle> {
        self.child_handles(
            self.child_handles(self.root)?
                .nth(member.owner().ordinal())?,
        )?
        .nth(member.ordinal())
        .filter(|symbol| self.get(*symbol).kind == SymbolKind::MathematicalDefinition)
    }

    pub fn builtin_type_member(&self, symbol: SymbolHandle) -> Option<BuiltinTypeMember> {
        symbol.is_valid().then_some(())?;
        let member = BuiltinTypeMember::AddressBound;
        (self.builtin_type_member_symbol(member) == Some(symbol)).then_some(member)
    }

    pub fn builtin_type_symbol(&self, builtin_type: BuiltinType) -> Option<SymbolHandle> {
        self.child_handles(self.root)?
            .nth(builtin_type.ordinal())
            .filter(|symbol| self.get(*symbol).kind == SymbolKind::BuiltinType)
    }

    /// Classify only an exact compiler-installed root builtin slot. A package
    /// declaration with the same spelling, or a generated source-free symbol,
    /// is deliberately not a compiler atom.
    pub fn builtin_type_atom(&self, symbol: SymbolHandle) -> Option<BuiltinTypeAtom> {
        if !symbol.is_valid() || self.get(symbol).kind != SymbolKind::BuiltinType {
            return None;
        }
        self.root_slot_ordinal(symbol, 0, BUILTIN_TYPE_COUNT)
            .and_then(BuiltinTypeAtom::from_ordinal)
    }

    /// Ordinal of `symbol` among the `count` compiler-installed root slots
    /// starting at root child `first`. The builder installs those slots as
    /// the root's first contiguous children, so an ordinal is an arena
    /// distance from the first child, confirmed by `nth` rather than a scan.
    fn root_slot_ordinal(&self, symbol: SymbolHandle, first: usize, count: usize) -> Option<usize> {
        let start = self.child_handles(self.root)?.next()?;
        let ordinal = symbol.arena_index().checked_sub(start.arena_index())? as usize;
        let slot = ordinal.checked_sub(first).filter(|slot| *slot < count)?;
        (self.child_handles(self.root)?.nth(ordinal) == Some(symbol)).then_some(slot)
    }

    /// `display_path` for a roster member through the per-table snapshot:
    /// zero allocation on a hit, and handles minted after the snapshot
    /// (generated roots, late module registrations, extension children)
    /// miss the map and fall back to the parent walk.
    pub(super) fn indexed_display_path(&self, symbol: SymbolHandle) -> std::borrow::Cow<'_, str> {
        let index = self.module_path_index.0.get_or_init(|| {
            let mut paths = crate::SymbolKeyMap::default();
            for candidate in self
                .child_handles(self.root)
                .into_iter()
                .flatten()
                .chain(self.module_symbols.iter().map(|(_, handle)| *handle))
            {
                paths.insert(candidate, Arc::from(self.display_path(candidate, "::")));
            }
            ModulePathIndex { paths }
        });
        match index.paths.get(&symbol) {
            Some(path) => std::borrow::Cow::Borrowed(path.as_ref()),
            None => std::borrow::Cow::Owned(self.display_path(symbol, "::")),
        }
    }

    pub fn display_path(&self, symbol: SymbolHandle, separator: &str) -> String {
        if !symbol.is_valid() {
            return String::new();
        }

        let mut names = Vec::new();
        let mut current = symbol;

        while current.is_valid() && current != self.root {
            names.push(self.name(current));
            current = self.namespace_parent(current);
        }

        let byte_count = names.iter().map(|name| name.len()).sum::<usize>()
            + separator
                .len()
                .saturating_mul(names.len().saturating_sub(1));
        let mut path = String::with_capacity(byte_count);

        for (index, name) in names.iter().rev().enumerate() {
            if index > 0 {
                path.push_str(separator);
            }

            path.push_str(name);
        }

        path
    }

    pub fn path_from_members(
        &mut self,
        root: SymbolHandle,
        members: impl IntoIterator<Item = SymbolHandle>,
    ) -> SymbolPath {
        SymbolPath {
            root,
            members: self.path_members.insert_many(members),
        }
    }

    pub fn resolve_child_path<'name>(
        &mut self,
        root: SymbolHandle,
        names: impl IntoIterator<Item = &'name str>,
    ) -> SymbolPath {
        let mut current = root;
        let mut members = Vec::new();

        if !root.is_valid() {
            return SymbolPath::default();
        }

        for name in names {
            let Some(child) = self.find_child_by_name(current, name) else {
                return SymbolPath::default();
            };

            members.push(child);
            current = child;
        }

        if members.is_empty() {
            SymbolPath::default()
        } else {
            self.path_from_members(root, members)
        }
    }

    pub fn path_members(&self, path: SymbolPath) -> &[SymbolHandle] {
        self.path_members.span_or_empty(path.members)
    }

    pub fn symbols(&self) -> &HierarchyArena<Symbol> {
        &self.symbols
    }

    pub fn names(&self) -> &Arena<SymbolName> {
        &self.names
    }

    pub fn path_member_arena(&self) -> &Arena<SymbolHandle> {
        &self.path_members
    }
}

pub struct SymbolChildHandles<'table> {
    contiguous: Option<SpanHandles<Symbol>>,
    supplemental: SupplementalChildHandles<'table>,
    namespace: std::vec::IntoIter<SymbolHandle>,
}

impl Iterator for SymbolChildHandles<'_> {
    type Item = SymbolHandle;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.contiguous
            .as_mut()
            .and_then(Iterator::next)
            .or_else(|| self.namespace.next())
            .or_else(|| self.supplemental.next())
    }

    /// Skip whole parts by length: builtin lookups index the root's children
    /// by ordinal, and stepping through them one at a time was measurable.
    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        if let Some(contiguous) = self.contiguous.as_mut() {
            let length = contiguous.len();
            if n < length {
                return contiguous.nth(n);
            }
            n -= length;
            let _ = contiguous.nth(length);
        }
        let length = self.namespace.len();
        if n < length {
            return self.namespace.nth(n);
        }
        n -= length;
        let _ = self.namespace.nth(length);
        self.supplemental.nth(n)
    }
}

pub struct SymbolTableExtension {
    table: SymbolTable,
}

impl SymbolTableExtension {
    pub fn insert_top_level<'name>(
        &mut self,
        declarations: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> Vec<SymbolHandle> {
        SymbolTableBuilder::child_handles(self.table.append_children(self.table.root, declarations))
            .collect()
    }

    /// Append beneath an existing declaration while retaining its exact node.
    pub fn append_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        self.table.append_children(parent, children)
    }

    pub fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        let children = children
            .into_iter()
            .map(|(kind, name)| {
                child_symbol(
                    &mut self.table.names,
                    self.table.sources.as_deref(),
                    parent,
                    kind,
                    name,
                )
            })
            .collect::<Vec<_>>();
        self.table.symbols.insert_children(parent, children)
    }

    pub fn finish(mut self) -> SymbolTable {
        self.table.root_names.clear();
        self.table.module_path_index.clear();
        self.table.module_scope_index.clear();
        self.table
    }
}

impl SymbolTableAppender for SymbolTableExtension {
    fn insert_children<'name>(
        &mut self,
        parent: SymbolHandle,
        children: impl IntoIterator<Item = (SymbolKind, SymbolNameRef<'name>)>,
    ) -> HandleSpan<Symbol> {
        SymbolTableExtension::insert_children(self, parent, children)
    }
}
