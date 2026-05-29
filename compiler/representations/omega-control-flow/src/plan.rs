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
