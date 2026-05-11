use super::RuntimeFlowBuilder;
use crate::{RuntimeState, RuntimeTransitionTarget};
use omega_control_flow::{MachineFlow, PlannedTransitionTarget};
use omega_core::diagnostics::Diagnostic;

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
            PlannedTransitionTarget::State { key, .. } => {
                RuntimeTransitionTarget::State { key: *key }
            }
            PlannedTransitionTarget::Nested {
                receiver_symbol,
                state_symbol,
                receiver,
                state,
                ..
            } => machine
                .contains
                .iter()
                .find(|contained| {
                    (receiver_symbol.is_valid() && contained.symbol == *receiver_symbol)
                        || contained.name == *receiver
                })
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
                        .and_then(|states| {
                            states.iter().find(|candidate| {
                                (state_symbol.is_valid() && candidate.key.state == *state_symbol)
                                    || candidate.name == *state
                            })
                        })
                })
                .map(|state| RuntimeTransitionTarget::State { key: state.key })
                .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                    name: format!("{receiver}.{state}"),
                }),
            PlannedTransitionTarget::SelfTarget => RuntimeTransitionTarget::State {
                key: self
                    .active_states
                    .last()
                    .map(|active_state| active_state.key)
                    .unwrap_or_default(),
            },
            PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
        }
    }
}
