use crate::expression::Expression;
use crate::name::ProgramName;
use crate::state::State;
use crate::types::TypeReference;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub contains: Vec<ContainedObject>,
    pub owned_data: Vec<OwnedData>,
    pub states: Vec<State>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            contains: Vec::new(),
            owned_data: Vec::new(),
            states: Vec::new(),
        }
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
