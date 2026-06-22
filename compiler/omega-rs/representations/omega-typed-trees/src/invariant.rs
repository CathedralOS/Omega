use crate::name::Identifier;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub constraints: HandleSpan<crate::types::TypeConstraintNode>,
}

impl Default for InvariantDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            constraints: HandleSpan::empty(),
        }
    }
}
