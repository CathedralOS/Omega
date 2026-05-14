use crate::expression::Expression;
use crate::name::ProgramName;
use crate::state::State;
use crate::types::TypeReference;
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub storage: MachineStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineStorage {
    pub contains: Vec<ContainedObject>,
    pub owned_data: Vec<OwnedData>,
    pub states: Vec<State>,
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
    pub name: ProgramName,
    pub type_name: ProgramName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_reference: TypeReference,
    pub initial_value: Option<Expression>,
}
