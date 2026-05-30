use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFact {
    pub symbol: SymbolHandle,
    pub name: omega_typed_trees::name::Identifier,
    pub constraint_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFacts {
    pub definitions: Arena<InvariantFact>,
}

impl InvariantFacts {
    pub fn with_roots(definitions: Arena<InvariantFact>) -> Self {
        Self { definitions }
    }
}
