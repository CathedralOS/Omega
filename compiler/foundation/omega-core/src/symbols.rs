use crate::arena::{
    Arena, Handle, HandleSpan, HierarchyArena, HierarchyArenaBuilder, HierarchyChildHandles,
    HierarchyNode,
};
use crate::source::{SourceMap, SourceSpan};
use std::sync::Arc;

pub type SymbolHandle = Handle<Symbol>;
pub type SymbolSpan = HandleSpan<SymbolHandle>;
pub type SymbolNameHandle = Handle<SymbolName>;
pub type SymbolDebugNameHandle = Handle<SymbolDebugName>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SymbolKind {
    #[default]
    Unknown,
    Root,
    Module,
    BuiltinType,
    Invariant,
    Data,
    Field,
    Variant,
    Machine,
    State,
    Parameter,
    Local,
    Platform,
    HostCapability,
    Object,
    Function,
    Section,
    Import,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Symbol {
    pub parent: SymbolHandle,
    pub children: HandleSpan<Symbol>,
    pub kind: SymbolKind,
    pub name: SymbolNameHandle,
    pub debug_name: SymbolDebugNameHandle,
}

impl HierarchyNode for Symbol {
    fn parent(&self) -> Handle<Self> {
        self.parent
    }

    fn set_parent(&mut self, parent: Handle<Self>) {
        self.parent = parent;
    }

    fn children(&self) -> HandleSpan<Self> {
        self.children
    }

    fn set_children(&mut self, children: HandleSpan<Self>) {
        self.children = children;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SymbolName {
    #[default]
    Missing,
    Source(SourceSpan),
    Static(&'static str),
    Owned(String),
}

impl SymbolName {
    pub fn from_ref(name: SymbolNameRef<'_>) -> Self {
        match name {
            SymbolNameRef::Borrowed(value) => Self::Owned(value.to_owned()),
            SymbolNameRef::Source(source_span) => Self::Source(source_span),
            SymbolNameRef::Static(value) => Self::Static(value),
        }
    }

    pub fn as_str<'source>(&'source self, sources: Option<&'source SourceMap>) -> &'source str {
        match self {
            Self::Missing => "",
            Self::Source(source_span) => sources
                .map(|sources| sources.text_at(*source_span))
                .unwrap_or(""),
            Self::Static(value) => value,
            Self::Owned(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolDebugName {
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolNameRef<'name> {
    Borrowed(&'name str),
    Source(SourceSpan),
    Static(&'static str),
}

impl<'name> SymbolNameRef<'name> {
    pub fn as_str(self) -> &'name str {
        match self {
            Self::Borrowed(value) => value,
            Self::Source(_) => "",
            Self::Static(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDefinition<'name> {
    pub kind: SymbolKind,
    pub name: SymbolNameRef<'name>,
    pub debug_name: Option<SymbolNameRef<'name>>,
    pub children: Vec<SymbolDefinition<'name>>,
}

impl<'name> SymbolDefinition<'name> {
    pub fn named(kind: SymbolKind, name: &'name str) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Borrowed(name),
            debug_name: None,
            children: Vec::new(),
        }
    }

    pub fn static_named(kind: SymbolKind, name: &'static str) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Static(name),
            debug_name: None,
            children: Vec::new(),
        }
    }

    pub fn source_named(kind: SymbolKind, source_span: SourceSpan) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Source(source_span),
            debug_name: None,
            children: Vec::new(),
        }
    }

    pub fn with_children(
        kind: SymbolKind,
        name: &'name str,
        children: impl IntoIterator<Item = SymbolDefinition<'name>>,
    ) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Borrowed(name),
            debug_name: None,
            children: children.into_iter().collect(),
        }
    }

    pub fn static_with_children(
        kind: SymbolKind,
        name: &'static str,
        children: impl IntoIterator<Item = SymbolDefinition<'name>>,
    ) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Static(name),
            debug_name: None,
            children: children.into_iter().collect(),
        }
    }

    pub fn source_with_children(
        kind: SymbolKind,
        source_span: SourceSpan,
        children: impl IntoIterator<Item = SymbolDefinition<'name>>,
    ) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Source(source_span),
            debug_name: None,
            children: children.into_iter().collect(),
        }
    }

    pub fn with_debug_name(mut self, debug_name: &'name str) -> Self {
        self.debug_name = Some(SymbolNameRef::Borrowed(debug_name));
        self
    }

    pub fn with_static_debug_name(mut self, debug_name: &'static str) -> Self {
        self.debug_name = Some(SymbolNameRef::Static(debug_name));
        self
    }
}

pub fn builtin_type_symbol_definitions() -> [SymbolDefinition<'static>; 19] {
    [
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "bool"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "i8"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "i16"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "i32"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "i64"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "isize"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "u8"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "u16"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "u32"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "u64"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "usize"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "f32"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "f64"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "String"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "Slice"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "Result"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "SyscallResult"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "Terminal"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "Never"),
    ]
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SymbolPath {
    pub root: SymbolHandle,
    pub members: SymbolSpan,
}

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
            match name {
                SymbolName::Missing => counts.missing += 1,
                SymbolName::Source(_) => counts.source_names += 1,
                SymbolName::Static(_) => counts.static_names += 1,
                SymbolName::Owned(_) => counts.owned_names += 1,
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

#[cfg(test)]
mod tests {
    use super::{SymbolDefinition, SymbolKind, SymbolTable};

    #[test]
    fn invalid_symbol_resolves_to_dummy() {
        let symbols = SymbolTable::new();
        let invalid = super::SymbolHandle::invalid();

        assert_eq!(symbols.get(invalid).kind, SymbolKind::Unknown);
        assert_eq!(symbols.name(invalid), "");
        assert_eq!(symbols.debug_name(invalid), "");
    }

    #[test]
    fn stores_symbols_with_parent_handles() {
        let symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::named(SymbolKind::State, "entry")],
            )],
        ));
        let root = symbols.root();
        let machine = symbols
            .find_child_by_name(root, "main")
            .expect("main should resolve");
        let state = symbols
            .find_child_by_name(machine, "entry")
            .expect("entry should resolve");

        assert_eq!(symbols.get(machine).parent, root);
        assert_eq!(symbols.get(state).parent, machine);
        assert_eq!(symbols.get(root).children.count(), 1);
        assert_eq!(symbols.get(machine).children.count(), 1);
        assert_eq!(symbols.name(state), "entry");
        assert_eq!(symbols.debug_name(state), "entry");
    }

    #[test]
    fn stores_paths_as_handle_spans() {
        let mut symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::named(SymbolKind::State, "entry")],
            )],
        ));
        let root = symbols.root();
        let machine = symbols
            .find_child_by_name(root, "main")
            .expect("main should resolve");
        let state = symbols
            .find_child_by_name(machine, "entry")
            .expect("entry should resolve");
        let path = symbols.path_from_members(root, [machine, state]);

        assert_eq!(path.root, root);
        assert_eq!(symbols.path_members(path), &[machine, state]);
    }

    #[test]
    fn resolves_child_paths_by_sibling_walk() {
        let mut symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::named(SymbolKind::State, "entry")],
            )],
        ));
        let root = symbols.root();
        let path = symbols.resolve_child_path(root, ["main", "entry"]);
        let names = symbols
            .path_members(path)
            .iter()
            .map(|symbol| symbols.name(*symbol))
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["main", "entry"]);
        let missing_path = symbols.resolve_child_path(root, ["main", "missing"]);
        assert!(symbols.path_members(missing_path).is_empty());
    }

    #[test]
    fn resolves_descendant_without_storing_path_members() {
        let symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::named(SymbolKind::State, "entry")],
            )],
        ));
        let root = symbols.root();
        let entry = symbols
            .find_descendant_by_path(root, ["main", "entry"])
            .expect("entry should resolve");

        assert_eq!(symbols.name(entry), "entry");
        assert_eq!(
            symbols.find_descendant_by_path(root, ["main", "missing"]),
            None
        );
        assert_eq!(symbols.path_member_arena().len(), 0);
    }

    #[test]
    fn formats_symbol_display_path_from_parent_chain() {
        let symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [SymbolDefinition::with_children(
                SymbolKind::Machine,
                "main",
                [SymbolDefinition::with_children(
                    SymbolKind::Object,
                    "console",
                    [SymbolDefinition::named(SymbolKind::State, "write_line")],
                )],
            )],
        ));
        let write_line = symbols
            .find_descendant_by_path(symbols.root(), ["main", "console", "write_line"])
            .expect("write_line should resolve");

        assert_eq!(
            symbols.display_path(write_line, "::"),
            "main::console::write_line"
        );
        assert_eq!(
            symbols.display_path(super::SymbolHandle::invalid(), "::"),
            ""
        );
    }

    #[test]
    fn child_ranges_are_exact_per_parent() {
        let symbols = SymbolTable::from_definition(SymbolDefinition::with_children(
            SymbolKind::Root,
            "root",
            [
                SymbolDefinition::with_children(
                    SymbolKind::Machine,
                    "main",
                    [
                        SymbolDefinition::named(SymbolKind::State, "entry"),
                        SymbolDefinition::named(SymbolKind::State, "running"),
                    ],
                ),
                SymbolDefinition::named(SymbolKind::Data, "Inventory"),
            ],
        ));
        let root = symbols.root();
        let root_children = symbols
            .child_handles(root)
            .expect("root children should resolve")
            .map(|child| symbols.debug_name(child).to_owned())
            .collect::<Vec<_>>();
        let main = symbols
            .find_child_by_name(root, "main")
            .expect("main should resolve");
        let main_children = symbols
            .child_handles(main)
            .expect("main children should resolve")
            .map(|child| symbols.debug_name(child).to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            root_children,
            vec!["main".to_owned(), "Inventory".to_owned()]
        );
        assert_eq!(
            main_children,
            vec!["entry".to_owned(), "running".to_owned()]
        );
    }

    #[test]
    fn debug_names_can_be_purged_without_invalidating_symbols_or_lookup_names() {
        let mut symbols =
            SymbolTable::from_definition(SymbolDefinition::named(SymbolKind::Root, "root"));
        let root = symbols.root();

        assert_eq!(symbols.name(root), "root");
        assert_eq!(symbols.debug_name(root), "root");
        symbols.clear_debug_names();
        assert!(root.is_valid());
        assert_eq!(symbols.name(root), "root");
        assert_eq!(symbols.debug_name(root), "root");
    }

    #[test]
    fn lookup_name_can_differ_from_debug_name() {
        let symbols = SymbolTable::from_definition(
            SymbolDefinition::named(SymbolKind::Root, "root").with_debug_name("root display only"),
        );
        let root = symbols.root();

        assert_eq!(symbols.name(root), "root");
        assert_eq!(symbols.debug_name(root), "root display only");
    }
}
