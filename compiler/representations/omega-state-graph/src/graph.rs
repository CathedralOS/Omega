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

impl StateGraph {
    pub fn with_roots(code: StateGraphCode, semantics: StateGraphSemanticRoots) -> Self {
        Self { code, semantics }
    }
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

#[cfg(test)]
mod tests {
    use crate::{StateGraph, StateGraphCode, StateGraphSemanticRoots};
    use omega_core::arena::Arena;

    #[test]
    fn graph_constructor_keeps_code_and_semantic_roots_explicit() {
        let code = StateGraphCode {
            expressions: Default::default(),
            machines: Arena::with_capacity(1),
            contained_machines: Arena::with_capacity(2),
            machine_owned_data: Arena::with_capacity(3),
            states: Arena::with_capacity(4),
            state_parameters: Arena::with_capacity(5),
            operations: Arena::with_capacity(6),
            transitions: Arena::with_capacity(7),
        };
        let semantics = StateGraphSemanticRoots::default();

        let graph = StateGraph::with_roots(code.clone(), semantics.clone());

        assert_eq!(graph.code, code);
        assert_eq!(graph.semantics, semantics);
    }
}
