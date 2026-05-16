use crate::name::ProgramName;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantDefinition {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub constraints: HandleSpan<crate::types::TypeConstraintNode>,
}

impl Default for InvariantDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            constraints: HandleSpan::empty(),
        }
    }
}
