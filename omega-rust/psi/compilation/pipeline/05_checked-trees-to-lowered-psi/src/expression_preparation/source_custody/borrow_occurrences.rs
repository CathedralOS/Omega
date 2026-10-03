//! Independently rejoin a primitive local borrow to its exact checked occurrence.

use checked_trees::{
    BorrowAccessKind, BorrowCallFact, CheckedStructuralAccess, CheckedTrees,
    CheckedUnitCallCoordinate,
};
use symbols::SymbolHandle;

use crate::lowering_error::LoweringError;

pub(crate) fn validate(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
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
    let call = call(checked, machine, state, coordinate, source_target)?;
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
    validate_place_argument_at(checked, call, position, place, BorrowAccessKind::Read)
}

/// Rejoin one authored access at its exact position after the complete ordered
/// roster has been replayed. The expected kind distinguishes a reference-carrier
/// read from an explicit mutable borrow of its referent.
pub(crate) fn validate_place_argument_at(
    checked: &CheckedTrees,
    call: &BorrowCallFact,
    position: usize,
    place: &checked_trees::CapturedPlace,
    expected_kind: BorrowAccessKind,
) -> Result<(), LoweringError> {
    let borrow = &checked.facts.borrow;
    let accesses =
        borrow
            .argument_accesses
            .span(call.accesses)
            .ok_or(LoweringError::Unsupported(
                "borrowed place has a stale access span",
            ))?;
    let argument = accesses.get(position).ok_or(LoweringError::Unsupported(
        "borrowed place lost its argument occurrence",
    ))?;
    if !place.root_symbol.is_valid()
        || argument.root_symbol != place.root_symbol
        || argument.kind != expected_kind
        || borrow
            .access_segments
            .span(argument.segments)
            .is_none_or(|path| path != place.segments)
        || accesses
            .iter()
            .enumerate()
            .any(|(other_position, candidate)| {
                other_position != position
                    && candidate.root_symbol == place.root_symbol
                    && (candidate.kind.is_exclusive() || expected_kind.is_exclusive())
                    && observations_coexist(accesses, position, other_position)
            })
    {
        return Err(LoweringError::Unsupported(
            "borrowed place substituted its occurrence or access",
        ));
    }
    Ok(())
}

/// Whether two rows of one ordered roster are live at the same time. The
/// earlier row stays live until its recorded end: a copied read ends at once,
/// and a loan formed inside a nested scalar call ends when that call returns
/// (stage 04 `borrow/accesses/collection.rs`). A row without an end is held
/// through the invocation, and so is one whose recorded end does not fall
/// after the row and inside the roster: a malformed end never separates rows.
fn observations_coexist(
    accesses: &[checked_trees::BorrowArgumentAccessFact],
    first: usize,
    second: usize,
) -> bool {
    let (earlier, later) = (first.min(second), first.max(second));
    accesses.get(earlier).is_none_or(|access| {
        access
            .end_before_access
            .is_none_or(|end| end > later || end <= earlier || end > accesses.len())
    })
}
