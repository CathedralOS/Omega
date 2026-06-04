use crate::name::Identifier;
use crate::signature::StateSignature;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    pub name: Identifier,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
}

impl Default for TraitDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            is_boundary: false,
            name: Identifier::default(),
            type_parameters: HandleSpan::empty(),
            requires: HandleSpan::empty(),
            machines: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRequirement {
    pub symbol: SymbolHandle,
    pub name: Identifier,
}

impl Default for TraitRequirement {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
        }
    }
}
