//! Independently rejoin a primitive local borrow to its exact checked occurrence.

use checked_trees::{
    BorrowAccessKind, CheckedStructuralAccess, CheckedTrees, CheckedUnitCallCoordinate,
    CheckedUnitEffectMachinePlan,
};
use symbols::SymbolHandle;

use crate::LoweringError;

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    coordinate: CheckedUnitCallCoordinate,
    source_target: SymbolHandle,
    symbol: SymbolHandle,
    access: CheckedStructuralAccess,
) -> Result<(), LoweringError> {
    let expected_kind = match access {
        CheckedStructuralAccess::SharedBorrow => BorrowAccessKind::Read,
        CheckedStructuralAccess::MutableBorrow => BorrowAccessKind::Mutable,
        CheckedStructuralAccess::WriteOnlyBorrow => BorrowAccessKind::WriteOnly,
        CheckedStructuralAccess::Owned => {
            return Err(LoweringError::Unsupported(
                "primitive local argument requires a borrow",
            ));
        }
    };
    let mut states = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| state.machine_symbol == plan.machine && state.state_symbol == plan.state);
    let state = states.next().ok_or(LoweringError::Unsupported(
        "primitive local borrow lost its state custody",
    ))?;
    if states.next().is_some() {
        return Err(LoweringError::Unsupported(
            "primitive local borrow state custody is duplicated",
        ));
    }
    let mut calls = checked
        .facts
        .borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
        });
    let call = calls.next().ok_or(LoweringError::Unsupported(
        "primitive local borrow lost its exact call",
    ))?;
    if calls.next().is_some() || call.target_symbol != source_target {
        return Err(LoweringError::Unsupported(
            "primitive local borrow call custody is duplicated or substituted",
        ));
    }
    // Select by root before validating the path and access: filtering by those
    // expectations first could conceal a conflicting row for the same referent.
    let mut accesses = checked
        .facts
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .filter(|argument| argument.root_symbol == symbol);
    let argument = accesses.next().ok_or(LoweringError::Unsupported(
        "primitive local borrow lost its exact referent",
    ))?;
    if accesses.next().is_some()
        || !argument.segments.is_empty()
        || !checked.facts.borrow.access_segments(argument).is_empty()
        || argument.kind != expected_kind
    {
        return Err(LoweringError::Unsupported(
            "primitive local borrow substituted its access or path",
        ));
    }
    Ok(())
}
