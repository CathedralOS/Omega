use super::RuntimeFlowBuilder;
use crate::control_flow::{MachineFlow, StateFlow, StateKey};
use omega_core::diagnostics::Diagnostic;

impl<'plan> RuntimeFlowBuilder<'plan> {
    pub(super) fn machine_flow_by_symbol(
        &self,
        machine_symbol: omega_core::symbols::SymbolHandle,
    ) -> Result<&MachineFlow, Diagnostic> {
        self.control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.symbol == machine_symbol)
            .map(|(_, machine)| machine)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "unknown runtime machine symbol `{}`",
                    machine_symbol.arena_index()
                ))
            })
    }

    pub(super) fn state_flow_by_key(&self, key: StateKey) -> Result<&StateFlow, Diagnostic> {
        let machine = self.machine_flow_by_symbol(key.machine)?;
        self.control_flow
            .states
            .span(machine.states)
            .and_then(|states| states.iter().find(|state| state.key == key))
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "unknown runtime state `{}`",
                    self.state_key_display(key)
                ))
            })
    }

    pub(super) fn state_key_display(&self, key: StateKey) -> String {
        let machine = self
            .machine_flow_by_symbol(key.machine)
            .map(|machine| machine.name.to_string())
            .unwrap_or_else(|_| format!("symbol{}", key.machine.arena_index()));
        let state = self
            .state_flow_by_key(key)
            .map(|state| state.name.to_string())
            .unwrap_or_else(|_| format!("symbol{}", key.state.arena_index()));

        if key.segment_index == 0 {
            format!("{machine}.{state}")
        } else {
            format!("{machine}.{state}#{}", key.segment_index)
        }
    }
}
