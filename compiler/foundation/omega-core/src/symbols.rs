use crate::arena::{Arena, Handle, HandleSpan};

pub type SymbolHandle = Handle<Symbol>;
pub type SymbolSpan = HandleSpan<SymbolHandle>;
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
    pub kind: SymbolKind,
    pub debug_name: SymbolDebugNameHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolDebugName {
    pub value: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SymbolPath {
    pub root: SymbolHandle,
    pub members: SymbolSpan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolTable {
    symbols: Arena<Symbol>,
    debug_names: Arena<SymbolDebugName>,
    path_members: Arena<SymbolHandle>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, parent: SymbolHandle, kind: SymbolKind) -> SymbolHandle {
        self.symbols.insert(Symbol {
            parent,
            kind,
            debug_name: SymbolDebugNameHandle::invalid(),
        })
    }

    pub fn insert_named(
        &mut self,
        parent: SymbolHandle,
        kind: SymbolKind,
        debug_name: impl Into<String>,
    ) -> SymbolHandle {
        let debug_name = self.debug_names.insert(SymbolDebugName {
            value: debug_name.into(),
        });

        self.symbols.insert(Symbol {
            parent,
            kind,
            debug_name,
        })
    }

    pub fn get(&self, symbol: SymbolHandle) -> &Symbol {
        self.symbols.get(symbol)
    }

    pub fn debug_name(&self, symbol: SymbolHandle) -> &str {
        let symbol = self.get(symbol);

        self.debug_names.get(symbol.debug_name).value.as_str()
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

    pub fn symbols(&self) -> &Arena<Symbol> {
        &self.symbols
    }

    pub fn debug_names(&self) -> &Arena<SymbolDebugName> {
        &self.debug_names
    }

    pub fn path_member_arena(&self) -> &Arena<SymbolHandle> {
        &self.path_members
    }

    pub fn clear_debug_names(&mut self) {
        self.debug_names.clear();

        self.symbols
            .for_each_mut(|_, symbol| symbol.debug_name = SymbolDebugNameHandle::invalid());
    }
}

#[cfg(test)]
mod tests {
    use super::{SymbolKind, SymbolTable};
    use crate::arena::Handle;

    #[test]
    fn invalid_symbol_resolves_to_dummy() {
        let symbols = SymbolTable::new();
        let invalid = Handle::invalid();

        assert_eq!(symbols.get(invalid).kind, SymbolKind::Unknown);
        assert_eq!(symbols.debug_name(invalid), "");
    }

    #[test]
    fn stores_symbols_with_parent_handles() {
        let mut symbols = SymbolTable::new();
        let root = symbols.insert_named(Handle::invalid(), SymbolKind::Root, "root");
        let machine = symbols.insert_named(root, SymbolKind::Machine, "main");
        let state = symbols.insert_named(machine, SymbolKind::State, "entry");

        assert_eq!(symbols.get(machine).parent, root);
        assert_eq!(symbols.get(state).parent, machine);
        assert_eq!(symbols.debug_name(state), "entry");
    }

    #[test]
    fn stores_paths_as_handle_spans() {
        let mut symbols = SymbolTable::new();
        let root = symbols.insert_named(Handle::invalid(), SymbolKind::Root, "root");
        let machine = symbols.insert_named(root, SymbolKind::Machine, "main");
        let state = symbols.insert_named(machine, SymbolKind::State, "entry");
        let path = symbols.path_from_members(root, [machine, state]);

        assert_eq!(path.root, root);
        assert_eq!(symbols.path_members(path), &[machine, state]);
    }

    #[test]
    fn debug_names_can_be_purged_without_invalidating_symbols() {
        let mut symbols = SymbolTable::new();
        let root = symbols.insert_named(Handle::invalid(), SymbolKind::Root, "root");

        assert_eq!(symbols.debug_name(root), "root");
        symbols.clear_debug_names();
        assert!(symbols.symbols().is_valid(root));
        assert_eq!(symbols.debug_name(root), "");
    }
}
