use omega_core::symbols::SymbolHandle;
use omega_typed_trees::name::Identifier;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFact {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub constraint_count: usize,
}
