mod lookups;

use psi_arena::Arena;
use psi_typed_trees::expression::ExpressionTable;

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

impl ControlFlowCode {
    pub fn with_roots(
        expressions: ExpressionTable,
        machines: Arena<MachineFlow>,
        contained_machines: Arena<ContainedFlow>,
        machine_owned_data: Arena<MachineOwnedDataFlow>,
        states: Arena<StateFlow>,
        state_parameters: Arena<StateParameterFlow>,
        operations: Arena<Operation>,
        transitions: Arena<TransitionFlow>,
    ) -> Self {
        Self {
            expressions,
            machines,
            contained_machines,
            machine_owned_data,
            states,
            state_parameters,
            operations,
            transitions,
        }
    }
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
    use psi_arena::Arena;

    #[test]
    fn plan_constructor_keeps_code_and_semantic_roots_explicit() {
        let code = control_flow_code_fixture();
        let semantics = ControlFlowSemanticRoots::default();

        let plan = ControlFlowPlan::with_roots(code.clone(), semantics.clone());

        assert_eq!(plan.code, code);
        assert_eq!(plan.semantics, semantics);
    }

    #[test]
    fn code_constructor_keeps_executable_roots_explicit() {
        let expressions = Default::default();
        let machines = Arena::with_capacity(1);
        let contained_machines = Arena::with_capacity(2);
        let machine_owned_data = Arena::with_capacity(3);
        let states = Arena::with_capacity(4);
        let state_parameters = Arena::with_capacity(5);
        let operations = Arena::with_capacity(6);
        let transitions = Arena::with_capacity(7);

        let code = ControlFlowCode::with_roots(
            expressions,
            machines.clone(),
            contained_machines.clone(),
            machine_owned_data.clone(),
            states.clone(),
            state_parameters.clone(),
            operations.clone(),
            transitions.clone(),
        );

        assert_eq!(code.machines, machines);
        assert_eq!(code.contained_machines, contained_machines);
        assert_eq!(code.machine_owned_data, machine_owned_data);
        assert_eq!(code.states, states);
        assert_eq!(code.state_parameters, state_parameters);
        assert_eq!(code.operations, operations);
        assert_eq!(code.transitions, transitions);
    }

    fn control_flow_code_fixture() -> ControlFlowCode {
        ControlFlowCode::with_roots(
            Default::default(),
            Arena::with_capacity(1),
            Arena::with_capacity(2),
            Arena::with_capacity(3),
            Arena::with_capacity(4),
            Arena::with_capacity(5),
            Arena::with_capacity(6),
            Arena::with_capacity(7),
        )
    }
}
