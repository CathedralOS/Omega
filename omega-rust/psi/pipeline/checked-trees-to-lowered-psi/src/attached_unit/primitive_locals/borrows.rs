//! Independently rejoin a primitive local borrow to its exact checked occurrence.

use checked_trees::{
    BorrowAccessKind, BorrowCallFact, CheckedStructuralAccess, CheckedTrees,
    CheckedUnitCallCoordinate, CheckedUnitEffectMachinePlan,
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
    let call = call(checked, plan.machine, plan.state, coordinate, source_target)?;
    validate_argument(checked, call, symbol, expected_kind)?;
    Ok(())
}

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

/// Matches a shared occurrence after its caller has replayed the full ordered
/// access roster, including scalar observations between structural operands.
pub(crate) fn validate_shared_argument_at(
    checked: &CheckedTrees,
    call: &BorrowCallFact,
    position: usize,
    symbol: SymbolHandle,
) -> Result<(), LoweringError> {
    validate_shared_place_argument_at(
        checked,
        call,
        position,
        &checked_trees::CapturedPlace {
            root_symbol: symbol,
            segments: Vec::new(),
        },
    )
}

/// Rejoin one shared occurrence after the complete ordered access roster was
/// independently replayed. A projected argument retains its original referent,
/// not the similarly typed root or a sibling field.
pub(crate) fn validate_shared_place_argument_at(
    checked: &CheckedTrees,
    call: &BorrowCallFact,
    position: usize,
    place: &checked_trees::CapturedPlace,
) -> Result<(), LoweringError> {
    let borrow = &checked.facts.borrow;
    let accesses =
        borrow
            .argument_accesses
            .span(call.accesses)
            .ok_or(LoweringError::Unsupported(
                "shared primitive borrow has a stale access span",
            ))?;
    let argument = accesses.get(position).ok_or(LoweringError::Unsupported(
        "shared primitive borrow lost its argument occurrence",
    ))?;
    if !place.root_symbol.is_valid()
        || argument.root_symbol != place.root_symbol
        || argument.kind != BorrowAccessKind::Read
        || borrow
            .access_segments
            .span(argument.segments)
            .is_none_or(|path| path != place.segments)
        || accesses.iter().any(|candidate| {
            candidate.root_symbol == place.root_symbol && candidate.kind.is_exclusive()
        })
    {
        return Err(LoweringError::Unsupported(
            "shared primitive borrow substituted its occurrence or access",
        ));
    }
    Ok(())
}
