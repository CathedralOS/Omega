use std::sync::Arc;

use crate::arena::{
    Arena, HandleSpan, HierarchyArena, HierarchyArenaBuilder, HierarchyChildHandles,
};
use crate::source::{SourceMap, SourceSpan};

use super::{
    Symbol, SymbolDebugName, SymbolDebugNameHandle, SymbolDefinition, SymbolHandle, SymbolName,
    SymbolNameStorageKind, SymbolPath,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTable {
    symbols: HierarchyArena<Symbol>,
    names: Arena<SymbolName>,
    debug_names: Arena<SymbolDebugName>,
    path_members: Arena<SymbolHandle>,
    sources: Option<Arc<SourceMap>>,
    root: SymbolHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SymbolNameStorageCounts {
    pub missing: usize,
    pub source_names: usize,
    pub static_names: usize,
    pub owned_names: usize,
    pub explicit_debug_names: usize,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_definition(root: SymbolDefinition<'_>) -> Self {
        Self::from_definition_with_sources(root, None)
    }

    pub fn from_definition_with_sources(
        root: SymbolDefinition<'_>,
        sources: Option<Arc<SourceMap>>,
    ) -> Self {
        let mut builder = HierarchyArenaBuilder::new();
        let mut names = Arena::new();
        let mut debug_names = Arena::new();
        let root = insert_root_definition(&mut builder, &mut names, &mut debug_names, &root);

        Self {
            symbols: builder.finish(),
            names,
            debug_names,
            path_members: Arena::new(),
            sources,
            root,
        }
    }

    pub fn get(&self, symbol: SymbolHandle) -> &Symbol {
        self.symbols.get(symbol)
    }

    pub fn name(&self, symbol: SymbolHandle) -> &str {
        let symbol = self.get(symbol);

        self.names.get(symbol.name).as_str(self.sources.as_deref())
    }

    pub fn debug_name(&self, symbol: SymbolHandle) -> &str {
        let symbol = self.get(symbol);

        if self.debug_names.is_valid(symbol.debug_name) {
            self.debug_names.get(symbol.debug_name).value.as_str()
        } else {
            self.names.get(symbol.name).as_str(self.sources.as_deref())
        }
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

    pub fn find_child_by_debug_name(
        &self,
        parent: SymbolHandle,
        debug_name: &str,
    ) -> Option<SymbolHandle> {
        self.find_child_by_name(parent, debug_name)
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

    pub fn debug_names(&self) -> &Arena<SymbolDebugName> {
        &self.debug_names
    }

    pub fn name_storage_counts(&self) -> SymbolNameStorageCounts {
        let mut counts = SymbolNameStorageCounts {
            explicit_debug_names: self.debug_names.len(),
            ..SymbolNameStorageCounts::default()
        };

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

    pub fn clear_debug_names(&mut self) {
        self.debug_names.clear();
    }
}

fn insert_root_definition(
    builder: &mut HierarchyArenaBuilder<Symbol>,
    names: &mut Arena<SymbolName>,
    debug_names: &mut Arena<SymbolDebugName>,
    definition: &SymbolDefinition<'_>,
) -> SymbolHandle {
    let root = builder.insert_root(symbol_from_definition(
        SymbolHandle::invalid(),
        names,
        debug_names,
        definition,
    ));
    insert_child_definitions(builder, names, debug_names, root, &definition.children);

    root
}

fn insert_child_definitions(
    builder: &mut HierarchyArenaBuilder<Symbol>,
    names: &mut Arena<SymbolName>,
    debug_names: &mut Arena<SymbolDebugName>,
    parent: SymbolHandle,
    definitions: &[SymbolDefinition<'_>],
) {
    if definitions.is_empty() {
        return;
    }

    let children = builder.insert_children(
        parent,
        definitions
            .iter()
            .map(|definition| symbol_from_definition(parent, names, debug_names, definition)),
    );

    for (offset, definition) in definitions.iter().enumerate() {
        let offset = u32::try_from(offset).expect("symbol child offset overflow");
        let child = SymbolHandle::from_parts(
            children
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("symbol child handle overflow"),
            children.start().generation(),
        );

        insert_child_definitions(builder, names, debug_names, child, &definition.children);
    }
}

fn symbol_from_definition(
    parent: SymbolHandle,
    names: &mut Arena<SymbolName>,
    debug_names: &mut Arena<SymbolDebugName>,
    definition: &SymbolDefinition<'_>,
) -> Symbol {
    let debug_name = definition
        .debug_name
        .filter(|debug_name| debug_name.as_str() != definition.name.as_str())
        .map(|debug_name| {
            debug_names.insert(SymbolDebugName {
                value: debug_name.as_str().to_owned(),
            })
        })
        .unwrap_or_else(SymbolDebugNameHandle::invalid);

    Symbol {
        parent,
        children: HandleSpan::empty(),
        kind: definition.kind,
        name: names.insert(SymbolName::from_ref(definition.name)),
        debug_name,
    }
}
