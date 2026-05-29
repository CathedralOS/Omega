mod capacity;
mod lookups;

use omega_core::arena::Arena;
use omega_typed_trees::expression::ExpressionTable;

use crate::{
    ContainedGraph, MachineGraph, MachineOwnedDataGraph, Operation, StateGraphSemanticRoots,
    StateNode, StateParameterNode, TransitionEdge,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphCode {
    pub expressions: ExpressionTable,
    pub machines: Arena<MachineGraph>,
    pub contained_machines: Arena<ContainedGraph>,
    pub machine_owned_data: Arena<MachineOwnedDataGraph>,
    pub states: Arena<StateNode>,
    pub state_parameters: Arena<StateParameterNode>,
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionEdge>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraph {
    pub code: StateGraphCode,
    pub semantics: StateGraphSemanticRoots,
}

impl std::ops::Deref for StateGraph {
    type Target = StateGraphCode;

    fn deref(&self) -> &Self::Target {
        &self.code
    }
}

impl std::ops::DerefMut for StateGraph {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.code
    }
}
