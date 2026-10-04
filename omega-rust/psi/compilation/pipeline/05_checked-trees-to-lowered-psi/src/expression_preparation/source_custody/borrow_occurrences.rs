//! Independently rejoin a primitive local borrow to its exact checked occurrence.

use checked_trees::{BorrowAccessKind, BorrowCallFact, CheckedTrees, CheckedUnitCallCoordinate};
use symbols::SymbolHandle;

use crate::lowering_error::LoweringError;

pub(crate) fn call(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    target: SymbolHandle,
) -> Result<&BorrowCallFact, LoweringError> {
    let borrow = &checked.facts.borrow;
    let mut states = borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state);
    let state = states.next().ok_or(LoweringError::Unsupported(
        "primitive borrow lost its state custody",
    ))?;
    if states.next().is_some() {
        return Err(LoweringError::Unsupported(
            "primitive borrow state custody is duplicated",
        ));
    }
    let mut calls = borrow
        .calls
        .span(state.calls)
        .ok_or(LoweringError::Unsupported(
            "primitive borrow has a stale call span",
        ))?
        .iter()
        .filter(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
        });
    let call = calls.next().ok_or(LoweringError::Unsupported(
        "primitive borrow lost its exact call",
    ))?;
    if calls.next().is_some() || call.target_symbol != target {
        return Err(LoweringError::Unsupported(
            "primitive borrow call custody is duplicated or substituted",
        ));
    }
    Ok(call)
}

/// Returns the exact access-row position, so mixed callers can retain order.
pub(crate) fn validate_argument(
    checked: &CheckedTrees,
    call: &BorrowCallFact,
    symbol: SymbolHandle,
    expected_kind: BorrowAccessKind,
) -> Result<usize, LoweringError> {
    let borrow = &checked.facts.borrow;
    // Select the root before checking path/access; otherwise a conflicting
    // row could disappear behind the expected path or permission filter.
    let mut accesses = borrow
        .argument_accesses
        .span(call.accesses)
        .ok_or(LoweringError::Unsupported(
            "primitive borrow has a stale access span",
        ))?
        .iter()
        .enumerate()
        .filter(|(_, argument)| argument.root_symbol == symbol);
    let (position, argument) = accesses.next().ok_or(LoweringError::Unsupported(
        "primitive borrow lost its exact referent",
    ))?;
    if !symbol.is_valid()
        || accesses.next().is_some()
        || !argument.segments.is_empty()
        || borrow
            .access_segments
            .span(argument.segments)
            .is_none_or(|path| !path.is_empty())
        || argument.kind != expected_kind
    {
        return Err(LoweringError::Unsupported(
            "primitive borrow substituted its access or path",
        ));
    }
    Ok(position)
}
