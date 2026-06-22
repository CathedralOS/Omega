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
    pub invariants: HandleSpan<crate::domain::ProofFact>,
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
            invariants: HandleSpan::empty(),
            requires: HandleSpan::empty(),
            machines: HandleSpan::empty(),
        }
    }
}

/// A standalone conformance item (`Point satisfies Equatable;`, frozen
/// decision 8): a data type claims a whole trait; validation checks the
/// type's written attached machines against the trait's requirements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataConformance {
    pub type_name: Identifier,
    pub trait_name: Identifier,
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
