//! What the authored statement does with a call's result, as stage 04's
//! source-custody binding pass recorded it on the call's flow row.

use super::CheckedTrees;
use checked_trees::FlowCallResult;

fn planned_result(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> FlowCallResult {
    checked
        .facts
        .flow
        .control
        .call_at(
            state,
            coordinate.statement_index as usize,
            coordinate.call_ordinal as usize,
        )
        .map_or_else(Default::default, |call| call.authored_result)
}

/// A discarded invocation produces a value but no source-local binding.
pub(crate) fn discards_result(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> bool {
    planned_result(checked, state, coordinate) == FlowCallResult::Discarded
}

/// Whether the call at `coordinate` is the whole initializer of a declared
/// immutable local, so its retained result is a source-named value. A result
/// an assignment stores has no source name of its own.
pub(crate) fn initializes_immutable_local(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> bool {
    planned_result(checked, state, coordinate) == FlowCallResult::ImmutableLocal
}
