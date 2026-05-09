use super::RuntimeFlowBuilder;
use crate::control_flow::{MachineFlow, PlannedTransitionTarget};
use crate::runtime_flow::{RuntimeState, RuntimeTransitionTarget};
use omega_core::diagnostics::Diagnostic;
use omega_typed_program::name::ProgramName;

impl RuntimeFlowBuilder<'_> {
    pub(super) fn visit_target(
        &mut self,
        target: RuntimeTransitionTarget,
    ) -> Result<(), Diagnostic> {
        if let RuntimeTransitionTarget::State { key, .. } = target {
            self.visit_state(RuntimeState { key })?;
        }

        Ok(())
    }

    pub(super) fn runtime_target(
        &self,
        machine: &MachineFlow,
        target: &PlannedTransitionTarget,
    ) -> RuntimeTransitionTarget {
        match target {
            PlannedTransitionTarget::State { key, name, .. } => RuntimeTransitionTarget::State {
                key: *key,
                machine: machine.name.clone(),
                state: name.clone(),
            },
            PlannedTransitionTarget::Nested {
                receiver, state, ..
            } => machine
                .contains
                .iter()
                .find(|contained| contained.name == *receiver)
                .and_then(|contained| {
                    self.control_flow
                        .machines
                        .iter()
                        .find(|(_, machine)| machine.symbol == contained.type_symbol)
                        .map(|(_, machine)| machine)
                })
                .and_then(|target_machine| {
                    self.control_flow
                        .states
                        .span(target_machine.states)
                        .and_then(|states| states.iter().find(|candidate| candidate.name == *state))
                })
                .map(|state| RuntimeTransitionTarget::State {
                    key: state.key,
                    machine: target_machine_name(machine, receiver).unwrap_or_default(),
                    state: state.name.clone(),
                })
                .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                    name: format!("{receiver}.{state}"),
                }),
            PlannedTransitionTarget::SelfTarget => RuntimeTransitionTarget::State {
                key: self
                    .active_states
                    .last()
                    .map(|active_state| active_state.key)
                    .unwrap_or_default(),
                machine: machine.name.clone(),
                state: self
                    .active_states
                    .last()
                    .and_then(|active_state| {
                        self.control_flow
                            .state_by_key(active_state.key)
                            .map(|state| state.name.clone())
                    })
                    .unwrap_or_default(),
            },
            PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
        }
    }
}

fn target_machine_name(machine: &MachineFlow, receiver: &ProgramName) -> Option<ProgramName> {
    machine
        .contains
        .iter()
        .find(|contained| contained.name == *receiver)
        .map(|contained| contained.type_name.clone())
}
