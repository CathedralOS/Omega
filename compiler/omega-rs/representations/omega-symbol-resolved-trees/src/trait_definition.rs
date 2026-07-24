use crate::name::DiagnosticName;
use crate::signature::StateSignature;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    pub name: DiagnosticName,
    pub storage: TraitStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitStorage {
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub invariants: HandleSpan<crate::domain::ProofFact>,
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
}

/// A standalone conformance item (`Point satisfies Equatable;`, frozen
/// decision 8): a data type claims a whole, optionally generic trait;
/// validation checks its written/default attached machines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataConformance {
    pub type_name: DiagnosticName,
    pub trait_name: DiagnosticName,
    pub arguments: HandleSpan<crate::types::TypeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRequirement {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    /// Generic arguments authored on a header parent (`Policy<C>`). Empty for
    /// the body-level `requires Policy;` form.
    pub arguments: HandleSpan<crate::types::TypeReference>,
}

impl Default for TraitRequirement {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            arguments: HandleSpan::empty(),
        }
    }
}

impl Deref for TraitDefinition {
    type Target = TraitStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for TraitDefinition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}
