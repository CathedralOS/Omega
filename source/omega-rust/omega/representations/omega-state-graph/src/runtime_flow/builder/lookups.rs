use super::RuntimeFlowBuilder;
use omega_control_flow::{MachineFlow, StateFlow, StateKey};
use psi_diagnostics::Diagnostic;
use std::fmt;

pub(super) struct StateKeyDisplay<'builder, 'plan> {
    builder: &'builder RuntimeFlowBuilder<'plan>,
    key: StateKey,
}

impl fmt::Display for StateKeyDisplay<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let machine = self.builder.machine_flow_by_symbol(self.key.machine).ok();

        match machine {
            Some(machine) => write!(formatter, "{}", machine.name)?,
            None => write!(formatter, "symbol{}", self.key.machine.arena_index())?,
        }

        formatter.write_str(".")?;

        let state = machine
            .and_then(|machine| self.builder.control_flow.states.span(machine.states))
            .and_then(|states| states.iter().find(|state| state.key == self.key));

        match state {
            Some(state) => write!(formatter, "{}", state.name)?,
            None => write!(formatter, "symbol{}", self.key.state.arena_index())?,
        }

        if self.key.segment_index != 0 {
            write!(formatter, "#{}", self.key.segment_index)?;
        }

        Ok(())
    }
}

impl<'plan> RuntimeFlowBuilder<'plan> {
    pub(super) fn machine_flow_by_symbol(
        &self,
        machine_symbol: psi_symbols::SymbolHandle,
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

    pub(super) fn state_key_display(&self, key: StateKey) -> StateKeyDisplay<'_, 'plan> {
        StateKeyDisplay { builder: self, key }
    }
}
