use crate::expression::Expression;
use crate::name::ProgramName;
use crate::state::State;
use crate::types::TypeReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: ProgramName,
    pub contains: Vec<ContainedObject>,
    pub owned_data: Vec<OwnedData>,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainedObject {
    pub name: ProgramName,
    pub type_name: ProgramName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub name: ProgramName,
    pub type_reference: TypeReference,
    pub initial_value: Option<Expression>,
}
