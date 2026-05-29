mod capacity;
mod lookups;

use omega_core::arena::Arena;
use omega_typed_trees::expression::ExpressionTable;

use crate::{
    ContainedGraph, MachineGraph, MachineOwnedDataGraph, Operation, StateGraphSemanticRoots,
    StateNode, StateParameterNode, TransitionEdge,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraph {
    pub expressions: ExpressionTable,
    pub machines: Arena<MachineGraph>,
    pub contained_machines: Arena<ContainedGraph>,
    pub machine_owned_data: Arena<MachineOwnedDataGraph>,
    pub states: Arena<StateNode>,
    pub state_parameters: Arena<StateParameterNode>,
    pub semantics: StateGraphSemanticRoots,
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionEdge>,
}
