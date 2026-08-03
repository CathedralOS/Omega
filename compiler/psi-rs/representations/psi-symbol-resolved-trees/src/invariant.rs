use crate::name::DiagnosticName;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantDefinition {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: InvariantDefinitionStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantDefinitionStorage {
    pub constraints: HandleSpan<crate::types::TypeConstraint>,
}

impl Deref for InvariantDefinition {
    type Target = InvariantDefinitionStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for InvariantDefinition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}
