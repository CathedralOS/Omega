use crate::ir::state::State;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub states: Vec<State>,
}
