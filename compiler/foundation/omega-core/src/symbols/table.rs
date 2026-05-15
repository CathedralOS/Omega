use std::sync::Arc;

use crate::arena::{
    Arena, HandleSpan, HierarchyArena, HierarchyArenaBuilder, HierarchyChildHandles,
};
use crate::source::{SourceMap, SourceSpan};

use super::{
    Symbol, SymbolHandle, SymbolKind, SymbolName, SymbolNameRef, SymbolNameStorageKind, SymbolPath,
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

    pub fn root(&self) -> SymbolHandle {
        self.root
    }

    pub fn child_handles(&self, parent: SymbolHandle) -> Option<HierarchyChildHandles<Symbol>> {
        self.symbols.child_handles(parent)
    }

    pub fn find_child_by_name(&self, parent: SymbolHandle, name: &str) -> Option<SymbolHandle> {
        self.symbols
            .find_child(parent, |symbol, _| self.name(symbol) == name)
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
