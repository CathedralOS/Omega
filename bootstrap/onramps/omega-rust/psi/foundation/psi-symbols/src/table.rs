use std::sync::Arc;

use psi_arena::{Arena, HandleSpan, HierarchyArena, HierarchyArenaBuilder, HierarchyChildHandles};
use psi_core::PackageKeyIdentity;
use psi_source::{SourceFile, SourceMap, SourceOrigin, SourceSpan};

use super::builtin::BUILTIN_TYPE_COUNT;
use super::{
    BuiltinFunction, BuiltinType, Symbol, SymbolHandle, SymbolKind, SymbolName, SymbolNameRef,
    SymbolNameStorageKind, SymbolPath,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTable {
    symbols: HierarchyArena<Symbol>,
    names: Arena<SymbolName>,
    path_members: Arena<SymbolHandle>,
    sources: Option<Arc<SourceMap>>,
    root: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTableBuilder {
    symbols: HierarchyArenaBuilder<Symbol>,
    names: Arena<SymbolName>,
    sources: Option<Arc<SourceMap>>,
    root: SymbolHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SymbolNameStorageCounts {
    pub missing: usize,
    pub source_names: usize,
    pub static_names: usize,
    pub owned_names: usize,
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
        let names = &mut self.names;
        self.symbols.insert_children(
            parent,
            children.into_iter().map(|(kind, name)| Symbol {
                parent,
                children: HandleSpan::empty(),
                kind,
                name: names.insert(SymbolName::from_ref(name)),
            }),
        )
    }

    pub fn child_handles(span: HandleSpan<Symbol>) -> impl Iterator<Item = SymbolHandle> {
        let start = span.start();

        (0..span.count()).map(move |offset| {
            SymbolHandle::from_parts(
                start
                    .arena_index()
                    .checked_add(offset)
                    .expect("symbol child handle overflow"),
                start.generation(),
            )
        })
    }

    pub fn finish(self) -> SymbolTable {
        SymbolTable {
            symbols: self.symbols.finish(),
            names: self.names,
            path_members: Arena::new(),
            sources: self.sources,
            root: self.root,
        }
    }

    fn symbol_from_parts(
        &mut self,
        parent: SymbolHandle,
        kind: SymbolKind,
        name: SymbolNameRef<'_>,
    ) -> Symbol {
        Symbol {
            parent,
            children: HandleSpan::empty(),
            kind,
            name: self.names.insert(SymbolName::from_ref(name)),
        }
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, symbol: SymbolHandle) -> &Symbol {
        self.symbols.get(symbol)
    }

    /// Mint a compiler-generated root declaration after symbol resolution.
    /// The generated name is owned by the table and authored symbol handles
    /// remain unchanged. Children must be installed once, as one batch, via
    /// [`Self::insert_generated_children`].
    pub fn insert_generated_root(&mut self, kind: SymbolKind, name: &str) -> SymbolHandle {
        let name = self
            .names
            .insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name)));
        self.symbols.insert_generated_root(Symbol {
            parent: SymbolHandle::invalid(),
            children: HandleSpan::empty(),
            kind,
            name,
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
        let names = &mut self.names;
        self.symbols.insert_generated_children(
            parent,
            children.into_iter().map(|(kind, name)| Symbol {
                parent,
                children: HandleSpan::empty(),
                kind,
                name: names.insert(SymbolName::from_ref(SymbolNameRef::Borrowed(name))),
            }),
        )
    }

    pub fn name(&self, symbol: SymbolHandle) -> &str {
        let symbol = self.get(symbol);

        self.names.get(symbol.name).as_str(self.sources.as_deref())
    }

    pub fn source_text(&self, source_span: SourceSpan) -> &str {
        self.sources
            .as_deref()
            .map(|sources| sources.text_at(source_span))
            .unwrap_or("")
    }

    /// Source declaration span retained for an authored symbol.
    pub fn symbol_source_span(&self, symbol: SymbolHandle) -> Option<SourceSpan> {
        self.names.get(self.get(symbol).name).source_span()
    }

    /// File metadata for one retained authored span. Generated/source-free
    /// trees return `None` instead of inventing a presentation path.
    pub fn source_file(&self, source_span: SourceSpan) -> Option<&SourceFile> {
        self.sources.as_deref()?.file_at(source_span)
    }

    /// Compare declaration provenance at the package boundary. Source-free
    /// lowering is used by focused representation tests; those trees model
    /// one package and retain the historical all-local behavior.
    pub fn same_source_package(&self, left: SourceSpan, right: SourceSpan) -> bool {
        self.sources
            .as_deref()
            .is_none_or(|sources| sources.same_package(left, right))
    }

    /// Compare the declaration packages of two authored symbols. Generated
    /// symbols carry no source span and therefore cannot accidentally inherit
    /// the first source file's package through `SourceSpan::default()`.
    /// Source-free focused trees continue to model one package.
    pub fn same_symbol_source_package(&self, left: SymbolHandle, right: SymbolHandle) -> bool {
        let Some(sources) = self.sources.as_deref() else {
            return true;
        };
        let Some(left) = self.names.get(self.get(left).name).source_span() else {
            return false;
        };
        let Some(right) = self.names.get(self.get(right).name).source_span() else {
            return false;
        };
        sources.same_package(left, right)
    }

    /// Authored declaration provenance for one symbol. Generated symbols and
    /// focused source-free trees return `None`; semantic consumers that admit
    /// source-free fixtures must make that fallback explicit rather than
    /// mistaking a spelling for toolchain ownership.
    pub fn symbol_source_origin(&self, symbol: SymbolHandle) -> Option<SourceOrigin> {
        let sources = self.sources.as_deref()?;
        let source_span = self.names.get(self.get(symbol).name).source_span()?;
        sources.file_at(source_span).map(|file| file.origin)
    }

    /// Reconciled package identity for one user-authored declaration.
    /// Generated, source-free, toolchain, and unmanaged symbols return `None`.
    pub fn symbol_package_identity(&self, symbol: SymbolHandle) -> Option<PackageKeyIdentity> {
        let source_span = self.symbol_source_span(symbol)?;
        let source_file = self.source_file(source_span)?;

        (source_file.origin == SourceOrigin::User)
            .then_some(source_file.package_identity)
            .flatten()
    }

    pub fn has_source_metadata(&self) -> bool {
        self.sources.is_some()
    }

    pub fn root(&self) -> SymbolHandle {
        self.root
    }

    pub fn child_handles(&self, parent: SymbolHandle) -> Option<HierarchyChildHandles<Symbol>> {
        self.symbols.child_handles(parent)
    }

    pub fn find_child_by_name(&self, parent: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        if !parent.is_valid() {
            return None;
        }

        self.symbols
            .find_child(parent, |symbol, _| self.name(symbol) == name)
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

        self.symbols.find_child(parent, |symbol, symbol_data| {
            symbol_data.kind == kind && self.name(symbol) == name
        })
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

    pub fn builtin_type_symbol(&self, builtin_type: BuiltinType) -> Option<SymbolHandle> {
        self.child_handles(self.root)?
            .nth(builtin_type.ordinal())
            .filter(|symbol| self.get(*symbol).kind == SymbolKind::BuiltinType)
    }

    pub fn display_path(&self, symbol: SymbolHandle, separator: &str) -> String {
        if !symbol.is_valid() {
            return String::new();
        }

        let mut names = Vec::new();
        let mut current = symbol;

        while current.is_valid() && current != self.root {
            names.push(self.name(current));
            current = self.get(current).parent;
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

    pub fn name_storage_counts(&self) -> SymbolNameStorageCounts {
        let mut counts = SymbolNameStorageCounts::default();

        for (_, name) in self.names.iter() {
            match name.storage_kind() {
                SymbolNameStorageKind::Missing => counts.missing += 1,
                SymbolNameStorageKind::Source => counts.source_names += 1,
                SymbolNameStorageKind::Static => counts.static_names += 1,
                SymbolNameStorageKind::Owned => counts.owned_names += 1,
            }
        }

        counts
    }

    pub fn path_member_arena(&self) -> &Arena<SymbolHandle> {
        &self.path_members
    }
}
