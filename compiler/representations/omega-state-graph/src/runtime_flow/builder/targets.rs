use super::RuntimeFlowBuilder;
use crate::RuntimeTransitionTarget;
use omega_control_flow::PlannedTransitionTarget;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;

impl RuntimeFlowBuilder<'_> {
    pub(super) fn visit_target(
        &mut self,
        target: RuntimeTransitionTarget,
    ) -> Result<(), Diagnostic> {
        if let RuntimeTransitionTarget::State { key, .. } = target {
            self.visit_state(key)?;
        }

        Ok(())
    }

    pub(super) fn runtime_target(
        &self,
        machine_symbol: SymbolHandle,
        target: &PlannedTransitionTarget,
    ) -> RuntimeTransitionTarget {
        match target {
            PlannedTransitionTarget::None => RuntimeTransitionTarget::None,
            PlannedTransitionTarget::State { key, .. } => {
                RuntimeTransitionTarget::State { key: *key }
            }
            PlannedTransitionTarget::Nested {
                receiver_symbol,
                state_symbol,
                receiver,
                state,
                ..
            } => self
                .control_flow
                .machine_by_symbol(machine_symbol)
                .and_then(|machine| {
                    self.control_flow
                        .machine_contains(machine)
                        .iter()
                        .find(|contained| {
                            (receiver_symbol.is_valid() && contained.symbol == *receiver_symbol)
                                || contained.name == *receiver
                        })
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
                .unwrap_or(RuntimeTransitionTarget::Unknown),
            PlannedTransitionTarget::SelfTarget => RuntimeTransitionTarget::State {
                key: self.active_states.last().unwrap_or_default(),
            },
            PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
        }
    }
}
