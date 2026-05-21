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
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRequirement {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
}

impl Default for TraitRequirement {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
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
