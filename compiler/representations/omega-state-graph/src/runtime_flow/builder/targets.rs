use super::RuntimeFlowBuilder;
use crate::RuntimeTransitionTarget;
use omega_control_flow::{PlannedTransitionTarget, StateKey};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::name::ProgramName;

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
            } => {
                if *receiver_symbol == machine_symbol || receiver.as_str() == "self" {
                    return self
                        .resolve_local_or_attached_state(machine_symbol, *state_symbol, state)
                        .map(|key| RuntimeTransitionTarget::State { key })
                        .unwrap_or(RuntimeTransitionTarget::Unknown);
                }

                self.resolve_contained_attached_state(
                    machine_symbol,
                    *receiver_symbol,
                    receiver,
                    *state_symbol,
                    state,
                )
                .map(|key| RuntimeTransitionTarget::State { key })
                .unwrap_or(RuntimeTransitionTarget::Unknown)
            }
            PlannedTransitionTarget::SelfTarget => RuntimeTransitionTarget::State {
                key: self.active_states.last().unwrap_or_default(),
            },
            PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
        }
    }

    fn resolve_local_or_attached_state(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        state_name: &ProgramName,
    ) -> Option<StateKey> {
        self.resolve_state_in_machine(machine_symbol, state_symbol, state_name)
            .or_else(|| {
                let machine = self.control_flow.machine_by_symbol(machine_symbol)?;
                let attached_data = machine.attached_data.as_ref()?;
                self.resolve_attached_data_state(attached_data, state_symbol, state_name)
            })
    }

    fn resolve_contained_attached_state(
        &self,
        machine_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
        receiver_name: &ProgramName,
        state_symbol: SymbolHandle,
        state_name: &ProgramName,
    ) -> Option<StateKey> {
        let machine = self.control_flow.machine_by_symbol(machine_symbol)?;
        let contained = self
            .control_flow
            .machine_contains(machine)
            .iter()
            .find(|contained| {
                (receiver_symbol.is_valid() && contained.symbol == receiver_symbol)
                    || contained.name == *receiver_name
            })?;

        self.resolve_attached_data_state(&contained.type_name, state_symbol, state_name)
            .or_else(|| {
                self.resolve_state_in_machine(contained.type_symbol, state_symbol, state_name)
            })
    }

    fn resolve_attached_data_state(
        &self,
        attached_data: &ProgramName,
        state_symbol: SymbolHandle,
        state_name: &ProgramName,
    ) -> Option<StateKey> {
        self.control_flow
            .machines
            .iter()
            .filter_map(|(_, candidate)| {
                (candidate.attached_data.as_ref() == Some(attached_data)).then_some(candidate)
            })
            .find_map(|machine| {
                self.resolve_state_in_machine(machine.symbol, state_symbol, state_name)
            })
    }

    fn resolve_state_in_machine(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        state_name: &ProgramName,
    ) -> Option<StateKey> {
        if state_symbol.is_valid()
            && let Some(key) = self
                .control_flow
                .state_key_by_symbols(machine_symbol, state_symbol)
        {
            return Some(key);
        }

        let machine = self.control_flow.machine_by_symbol(machine_symbol)?;
        self.control_flow
            .states
            .span(machine.states)?
            .iter()
            .find(|state| state.name == *state_name)
            .map(|state| state.key)
    }
}
