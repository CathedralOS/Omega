use crate::control_flow::{ControlFlowPlan, OperationKind, StateKey};
use crate::host_calls::HostCallPlan;
use crate::plan::NativePlan;
use crate::runtime_flow::RuntimeFlowPlan;
use crate::state_calls::StateCallPlan;

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

    pub fn state_is_required(&self, machine_name: &str, state_name: &str) -> bool {
        let state_key = self.state_key(machine_name, state_name);

        self.runtime_flow
            .states
            .iter()
            .any(|(_, state)| Some(state.key) == state_key)
            || self.state_calls.calls.iter().any(|(_, state_call)| {
                state_call.required
                    && ((state_call.source_machine == machine_name
                        && state_call.source_state == state_name)
                        || (state_call.target_machine == machine_name
                            && state_call.target_state == state_name))
            })
    }

    pub fn state_mutation_is_already_lowered(
        &self,
        machine_name: &str,
        state_name: &str,
        statement_index: usize,
    ) -> bool {
        let Some(machine) = self
            .control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name == machine_name)
            .map(|(_, machine)| machine)
        else {
            return false;
        };
        let Some(state) = self
            .control_flow
            .states
            .span(machine.states)
            .and_then(|states| states.iter().find(|state| state.name == state_name))
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

    pub fn runtime_state_is_reachable(&self, machine_name: &str, state_name: &str) -> bool {
        let state_key = self.state_key(machine_name, state_name);

        self.runtime_flow
            .states
            .iter()
            .any(|(_, state)| Some(state.key) == state_key)
    }

    pub fn state_statement_has_host_call(
        &self,
        machine_name: &str,
        state_name: &str,
        statement_index: usize,
    ) -> bool {
        self.host_calls.calls.iter().any(|(_, host_call)| {
            host_call.machine == machine_name
                && host_call.state == state_name
                && host_call.statement_index == statement_index
        })
    }

    fn state_key(&self, machine_name: &str, state_name: &str) -> Option<StateKey> {
        let machine = self
            .control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name == machine_name)
            .map(|(_, machine)| machine)?;

        self.control_flow
            .states
            .span(machine.states)?
            .iter()
            .find(|state| state.name == state_name)
            .map(|state| state.key)
    }
}
