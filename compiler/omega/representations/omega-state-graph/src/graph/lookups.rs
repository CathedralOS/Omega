use psi_symbols::SymbolHandle;
use psi_typed_trees::name::Identifier;

use crate::{
    ContainedGraph, MachineGraph, MachineOwnedDataGraph, StateGraph, StateKey, StateNode,
    StateParameterNode,
};

impl StateGraph {
    pub fn state_key_by_symbols(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<StateKey> {
        let machine = self.machine_by_symbol(machine_symbol)?;

        self.states
            .span(machine.states)?
            .iter()
            .find(|state| state.key.machine == machine_symbol && state.key.state == state_symbol)
            .map(|state| state.key)
    }

    pub fn machine_by_symbol(&self, machine_symbol: SymbolHandle) -> Option<&MachineGraph> {
        self.machines
            .iter()
            .find(|(_, machine)| machine.symbol == machine_symbol)
            .map(|(_, machine)| machine)
    }

    pub fn machine_contains(&self, machine: &MachineGraph) -> &[ContainedGraph] {
        self.contained_machines.span_or_empty(machine.contains)
    }

    pub fn machine_owned_data(&self, machine: &MachineGraph) -> &[MachineOwnedDataGraph] {
        self.machine_owned_data.span_or_empty(machine.owned_data)
    }

    pub fn state_by_key(&self, key: StateKey) -> Option<&StateNode> {
        let machine = self.machine_by_symbol(key.machine)?;

        self.states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == key)
    }

    pub fn state_parameters(&self, state: &StateNode) -> &[StateParameterNode] {
        self.state_parameters.span_or_empty(state.parameters)
    }

    pub fn state_names_by_key(&self, key: StateKey) -> Option<(&Identifier, &Identifier)> {
        let machine = self.machine_by_symbol(key.machine)?;
        let state = self
            .states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == key)?;

        Some((&machine.name, &state.name))
    }

    pub fn state_names_by_key_cloned(&self, key: StateKey) -> (Identifier, Identifier) {
        self.state_names_by_key(key)
            .map(|(machine, state)| (machine.clone(), state.clone()))
            .unwrap_or_default()
    }

    pub fn state_machine_name_by_key_cloned(&self, key: StateKey) -> Identifier {
        self.state_names_by_key(key)
            .map(|(machine, _)| machine.clone())
            .unwrap_or_default()
    }

    pub fn state_name_by_key_cloned(&self, key: StateKey) -> Identifier {
        self.state_names_by_key(key)
            .map(|(_, state)| state.clone())
            .unwrap_or_default()
    }
}
