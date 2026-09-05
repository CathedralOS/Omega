use arena::{Handle, HandleSpan, HierarchyNode};

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
    /// Exact earlier symbol whose declaration provenance this compiler-created
    /// symbol derives from. Authored symbols use the invalid handle.
    pub generated_from: SymbolHandle,
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
