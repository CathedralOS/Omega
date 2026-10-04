//! Declaration lookups every plan family shares.
//!
//! A checked plan names machines and states by symbol. Lowering needs their
//! declarations — signatures, parameters and return types — to emit
//! operations, and this module is the one place that maps a state symbol to
//! its declaring machine and state. It reads declarations only; authored
//! statement and expression bodies belong to stage 04, which records what
//! lowering needs in the plan.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;

/// The one declared machine and state that own `state`.
pub(crate) fn state_declaration(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
) -> Result<
    (
        &checked_trees::machine::Machine,
        &checked_trees::state::State,
    ),
    LoweringError,
> {
    let program = &checked.typed;
    let mut states = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter_map(move |candidate| {
                (state.is_valid() && candidate.symbol == state).then_some((machine, candidate))
            })
    });
    let (machine, state) = states.next().ok_or(LoweringError::Unsupported(
        "state symbol names no declared machine state",
    ))?;
    if states.next().is_some() {
        return unsupported("state symbol names more than one declared machine state");
    }
    Ok((machine, state))
}
