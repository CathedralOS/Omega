use crate::arena::{
    Arena, Handle, HandleSpan, HierarchyArena, HierarchyArenaBuilder, HierarchyChildHandles,
    HierarchyNode,
};

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolName {
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolDebugName {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDefinition<'name> {
    pub kind: SymbolKind,
    pub name: &'name str,
    pub debug_name: &'name str,
    pub children: Vec<SymbolDefinition<'name>>,
}

impl<'name> SymbolDefinition<'name> {
    pub fn named(kind: SymbolKind, name: &'name str) -> Self {
        Self {
            kind,
            name,
            debug_name: name,
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
            name,
            debug_name: name,
            children: children.into_iter().collect(),
        }
    }

    pub fn with_debug_name(mut self, debug_name: &'name str) -> Self {
        self.debug_name = debug_name;
        self
    }
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
    root: SymbolHandle,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_definition(root: SymbolDefinition<'_>) -> Self {
        let mut builder = HierarchyArenaBuilder::new();
        let mut names = Arena::new();
        let mut debug_names = Arena::new();
        let root = insert_root_definition(&mut builder, &mut names, &mut debug_names, &root);

        Self {
            symbols: builder.finish(),
            names,
            debug_names,
            path_members: Arena::new(),
            root,
        }
    }

    pub fn get(&self, symbol: SymbolHandle) -> &Symbol {
        self.symbols.get(symbol)
    }

    pub fn name(&self, symbol: SymbolHandle) -> &str {
        let symbol = self.get(symbol);

        self.names.get(symbol.name).value.as_str()
    }

    pub fn debug_name(&self, symbol: SymbolHandle) -> &str {
        let symbol = self.get(symbol);

        self.debug_names.get(symbol.debug_name).value.as_str()
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
    Symbol {
        parent,
        children: HandleSpan::empty(),
        kind: definition.kind,
        name: names.insert(SymbolName {
            value: definition.name.to_owned(),
        }),
        debug_name: debug_names.insert(SymbolDebugName {
            value: definition.debug_name.to_owned(),
        }),
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
        assert_eq!(symbols.debug_name(root), "");
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
