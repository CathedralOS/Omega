use crate::expression::Expression;
use crate::name::DiagnosticName;
use crate::state::State;
use crate::types::TypeReference;
use omega_core::arena::{Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: MachineStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineStorage {
    pub contains: Vec<ContainedObject>,
    pub owned_data: Vec<OwnedData>,
    pub states: HandleSpan<Handle<State>>,
}

impl Deref for Machine {
    type Target = MachineStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for Machine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainedObject {
    pub symbol: SymbolHandle,
    pub type_symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_name: DiagnosticName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_reference: TypeReference,
    pub initial_value: Option<Expression>,
}
