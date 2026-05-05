use crate::ir::state::State;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub contains: Vec<ContainedObject>,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainedObject {
    pub name: String,
    pub type_name: String,
}
