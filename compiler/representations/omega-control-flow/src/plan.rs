mod lookups;

use omega_core::arena::Arena;
use omega_typed_trees::expression::ExpressionTable;

use crate::{
    ContainedFlow, ControlFlowSemanticRoots, MachineFlow, MachineOwnedDataFlow, Operation,
    StateFlow, StateParameterFlow, TransitionFlow,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowCode {
    pub expressions: ExpressionTable,
    pub machines: Arena<MachineFlow>,
    pub contained_machines: Arena<ContainedFlow>,
    pub machine_owned_data: Arena<MachineOwnedDataFlow>,
    pub states: Arena<StateFlow>,
    pub state_parameters: Arena<StateParameterFlow>,
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionFlow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowPlan {
    pub code: ControlFlowCode,
    pub semantics: ControlFlowSemanticRoots,
}

impl ControlFlowPlan {
    pub fn with_roots(code: ControlFlowCode, semantics: ControlFlowSemanticRoots) -> Self {
        Self { code, semantics }
    }
}

impl std::ops::Deref for ControlFlowPlan {
    type Target = ControlFlowCode;

    fn deref(&self) -> &Self::Target {
        &self.code
    }
}

impl std::ops::DerefMut for ControlFlowPlan {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.code
    }
}

#[cfg(test)]
mod tests {
    use crate::{ControlFlowCode, ControlFlowPlan, ControlFlowSemanticRoots};
    use omega_core::arena::Arena;

    #[test]
    fn plan_constructor_keeps_code_and_semantic_roots_explicit() {
        let code = ControlFlowCode {
            expressions: Default::default(),
            machines: Arena::with_capacity(1),
            contained_machines: Arena::with_capacity(2),
            machine_owned_data: Arena::with_capacity(3),
            states: Arena::with_capacity(4),
            state_parameters: Arena::with_capacity(5),
            operations: Arena::with_capacity(6),
            transitions: Arena::with_capacity(7),
        };
        let semantics = ControlFlowSemanticRoots::default();

        let plan = ControlFlowPlan::with_roots(code.clone(), semantics.clone());

        assert_eq!(plan.code, code);
        assert_eq!(plan.semantics, semantics);
    }
}
