use crate::expression::Expression;
use crate::state::State;
use crate::types::TypeReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub contains: Vec<ContainedObject>,
    pub owned_data: Vec<OwnedData>,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainedObject {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub name: String,
    pub type_reference: TypeReference,
    pub initial_value: Option<Expression>,
}
