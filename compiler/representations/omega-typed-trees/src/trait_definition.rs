use crate::name::ProgramName;
use crate::signature::StateSignature;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    pub name: ProgramName,
    pub machines: HandleSpan<StateSignature>,
}

impl Default for TraitDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            is_boundary: false,
            name: ProgramName::default(),
            machines: HandleSpan::empty(),
        }
    }
}
