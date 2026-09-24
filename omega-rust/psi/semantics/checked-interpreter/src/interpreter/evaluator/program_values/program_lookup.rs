use crate::interpreter::evaluator::{DataDefinition, Evaluator, Machine, State};
/// Exact typed-program lookup shared by evaluator responsibilities. These
/// helpers preserve declaration identity and do not perform leaf fallback.
impl<'program> Evaluator<'program> {
    pub(in crate::interpreter::evaluator) fn find_machine_by_name(
        &self,
        name: &str,
    ) -> Option<&'program Machine> {
        self.program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
    }

    pub(in crate::interpreter::evaluator) fn find_machine_by_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&'program Machine> {
        self.program
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
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

    pub(in crate::interpreter::evaluator) fn find_data_by_name(
        &self,
        name: &str,
    ) -> Option<&'program DataDefinition> {
        self.program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)
    }
}
