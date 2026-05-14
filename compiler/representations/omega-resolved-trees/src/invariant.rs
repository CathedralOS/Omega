use crate::name::ProgramName;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantDefinition {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
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
