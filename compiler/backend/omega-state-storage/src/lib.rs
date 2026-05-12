mod collection;
mod model;
mod mutation_kind;

pub use collection::{build_state_storage_plan, build_state_storage_plan_with_workers};
pub use model::{
    StateLocalStorage, StateMutation, StateMutationKind, StateMutationLowering, StateStoragePlan,
};
use omega_control_flow::{ControlFlowPlan, OperationKind, StateKey};
use omega_state_calls::StateCallPlan;
use omega_state_graph::RuntimeFlowPlan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateStoragePlanningContext {
    pub control_flow: ControlFlowPlan,
    pub runtime_flow: RuntimeFlowPlan,
    pub state_calls: StateCallPlan,
}

impl StateStoragePlanningContext {
    pub fn state_flow_by_key(&self, state_key: StateKey) -> Option<&omega_control_flow::StateFlow> {
        let machine = self
            .control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.symbol == state_key.machine)
            .map(|(_, machine)| machine)?;

        self.control_flow
            .states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == state_key)
    }

    pub fn state_is_required_by_key(&self, state_key: StateKey) -> bool {
        self.runtime_flow
            .states
            .iter()
            .any(|(_, state)| state.key == state_key)
            || self.state_calls.calls.iter().any(|(_, state_call)| {
                state_call.required
                    && (state_call.source_key == state_key || state_call.target_key == state_key)
            })
    }

    pub fn state_mutation_is_already_lowered_by_key(
        &self,
        state_key: StateKey,
        statement_index: usize,
    ) -> bool {
        let Some(state) = self.state_flow_by_key(state_key) else {
            return false;
        };
        let Some(operations) = self.control_flow.operations.span(state.operations) else {
            return false;
        };

        operations.iter().any(|operation| {
            operation.statement_index == statement_index
                && matches!(
                    operation.kind,
                    OperationKind::ConstantIntegerAssignment
                        | OperationKind::StaticAssignment { .. }
                )
        })
    }
}
