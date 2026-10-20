use crate::interpreter::evaluator::{DataDefinition, Evaluator, Machine, State};
/// Exact typed-program lookup shared by evaluator responsibilities. These
/// helpers preserve declaration identity and do not perform leaf fallback.
impl<'program> Evaluator<'program> {
    pub(in crate::interpreter::evaluator) fn find_machine_by_name(
        &self,
        name: &str,
    ) -> Option<&'program Machine> {
        let machine = self.program.realized_machine_named(name)?;
        self.find_machine_by_symbol(machine.symbol)
    }

    pub(in crate::interpreter::evaluator) fn find_machine_by_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&'program Machine> {
        let selected = self
            .selected_build_time_machine_bodies
            .iter()
            .find(|selected| selected.family == symbol)
            .map_or(symbol, |selected| selected.body);
        self.program.machine(selected)
    }

    pub(in crate::interpreter::evaluator) fn find_state(
        &self,
        machine: &Machine,
        name: &str,
    ) -> Option<&'program State> {
        self.program
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == name)
    }

    /// Resolved-identity counterpart of `find_state`: the check stage records
    /// each named transition target's state symbol, so execution matches on it
    /// instead of a debug spelling that may collide with a sibling's.
    pub(in crate::interpreter::evaluator) fn find_state_by_symbol(
        &self,
        machine: &Machine,
        symbol: symbols::SymbolHandle,
    ) -> Option<&'program State> {
        if symbol == symbols::SymbolHandle::invalid() {
            return None;
        }
        self.program.state_in_machine(machine, symbol)
    }

    pub(in crate::interpreter::evaluator) fn find_data_by_name(
        &self,
        name: &str,
    ) -> Option<&'program DataDefinition> {
        self.program.data_definition_by_name_str(name)
    }
}
