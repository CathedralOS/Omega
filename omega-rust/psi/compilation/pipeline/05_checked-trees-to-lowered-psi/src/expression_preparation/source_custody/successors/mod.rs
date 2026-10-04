//! Rejoin the complete authored successor roster before partitioning its namespaces.

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;

pub(crate) fn normalize_machine_state_target(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    target: SymbolHandle,
) -> Result<SymbolHandle, LoweringError> {
    let states = checked.machine_states(machine);
    if target.is_valid() && states.iter().any(|state| state.symbol == target) {
        return Ok(target);
    }
    // Free names retain a machine head; qualified calls may retain its
    // entry child instead. Normalize the exact owner to its first state,
    // independently of its name or how many interior states it declares.
    if target.is_valid()
        && let Some(owner) = checked.typed.machine_by_symbol(target)
        && let Some(entry) = checked.machine_states(owner).first()
    {
        return Ok(entry.symbol);
    }
    // A target spelling another machine's entry names that machine's first
    // state outright — the same spelling the scalar call gates normalize.
    if target.is_valid()
        && checked
            .typed
            .machines()
            .iter()
            .filter_map(|owner| checked.machine_states(owner).first())
            .any(|entry| entry.symbol == target)
    {
        return Ok(target);
    }
    unsupported("scalar successor has no exact machine/state target normalization")
}
