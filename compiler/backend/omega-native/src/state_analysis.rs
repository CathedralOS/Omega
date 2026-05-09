use crate::plan::NativePlan;
use omega_control_flow::{ControlFlowPlan, OperationKind, StateKey};
use omega_platform_interface::HostCallPlan;
use omega_state_calls::StateCallPlan;
use omega_state_graph::RuntimeFlowPlan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateAnalysisContext {
    pub control_flow: ControlFlowPlan,
    pub host_calls: HostCallPlan,
    pub runtime_flow: RuntimeFlowPlan,
    pub state_calls: StateCallPlan,
}

impl StateAnalysisContext {
    pub fn from_native_plan(native_plan: &NativePlan) -> Self {
        Self {
            control_flow: native_plan.control_flow.clone(),
            host_calls: native_plan.host_calls.clone(),
            runtime_flow: native_plan.runtime_flow.clone(),
            state_calls: native_plan.state_calls.clone(),
        }
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
        let Some(machine) = self
            .control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.symbol == state_key.machine)
            .map(|(_, machine)| machine)
        else {
            return false;
        };
        let Some(state) = self
            .control_flow
            .states
            .span(machine.states)
            .and_then(|states| states.iter().find(|state| state.key == state_key))
        else {
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

    pub fn runtime_state_is_reachable_by_key(&self, state_key: StateKey) -> bool {
        self.runtime_flow
            .states
            .iter()
            .any(|(_, state)| state.key == state_key)
    }

    pub fn state_statement_has_host_call_by_key(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> bool {
        self.host_calls.calls.iter().any(|(_, host_call)| {
            host_call.source_key == source_key && host_call.statement_index == statement_index
        })
    }
}
