use psi_arena::{Handle, HandleSpan, HierarchyNode};

use super::{SymbolKind, SymbolName};

pub type SymbolHandle = Handle<Symbol>;
pub type SymbolSpan = HandleSpan<SymbolHandle>;
pub type SymbolNameHandle = Handle<SymbolName>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Symbol {
    pub parent: SymbolHandle,
    pub children: HandleSpan<Symbol>,
    pub kind: SymbolKind,
    pub name: SymbolNameHandle,
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
