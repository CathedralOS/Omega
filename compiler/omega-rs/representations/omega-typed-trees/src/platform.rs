use crate::name::Identifier;
use crate::signature::StateSignature;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub states: HandleSpan<StateSignature>,
}

impl Default for Platform {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            states: HandleSpan::empty(),
        }
    }
}
