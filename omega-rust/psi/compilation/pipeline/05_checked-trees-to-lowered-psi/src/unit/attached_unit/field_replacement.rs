//! `place = value` over a structural field plans one roster: the producer of
//! the replacing value (the statement's call, or the establishment of its
//! construction), its store, and displacement/cleanup only when the destination
//! was occupied. An already-open hole has no old value to move or dispose.
//! Every member after the producer continues the producer's
//! statement; the destination and value are rejoined against the authored
//! assignment by the store's own source custody.

use super::{CheckedUnitEffectOperationPlan, Multiplicity};

/// Whether this store (or its optional displacement) continues the same
/// statement's producer. Hole state is independently reconstructed by the
/// borrowed-window ledger; this checks value identity and operation order.
pub(super) fn continues(operations: &[CheckedUnitEffectOperationPlan], index: usize) -> bool {
    let store_index = match operations.get(index) {
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField { .. }) => index + 1,
        Some(CheckedUnitEffectOperationPlan::StoreStructuralField { .. }) => index,
        _ => return false,
    };
    let Some(CheckedUnitEffectOperationPlan::StoreStructuralField {
        statement_index,
        destination,
        value,
    }) = operations.get(store_index)
    else {
        return false;
    };
    let Some(previous_index) = store_index.checked_sub(1) else {
        return false;
    };
    let (displaced, producer_index) = match operations.get(previous_index) {
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField { result, source }) => {
            if result.statement_index != *statement_index || source != destination {
                return false;
            }
            let Some(producer_index) = previous_index.checked_sub(1) else {
                return false;
            };
            (Some(result), producer_index)
        }
        _ => (None, previous_index),
    };
    let Some(producer) = operations.get(producer_index) else {
        return false;
    };
    let produced = match producer {
        // The statement's own call with a structural result, ordinary or boundary.
        CheckedUnitEffectOperationPlan::Call(call)
            if let Some(result) = call.structural_result()
                && call.coordinate.call_ordinal == 0
                && call.coordinate.statement_index == *statement_index =>
        {
            result
        }
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            operand_source: None,
            ..
        } if result.statement_index == *statement_index => result,
        _ => return false,
    };
    value.path.is_empty()
        && value.access == checked_trees::CheckedStructuralAccess::Owned
        && value.source_structural_result_binding_ordinal() == Some(produced.binding_ordinal)
        && value.type_identity == produced.type_identity
        && displaced.is_none_or(|moved| {
            moved.type_identity == produced.type_identity
                && moved.multiplicity == produced.multiplicity
        })
}

/// Whether the operation at `index` is the move-out or store of a
/// replacement whose value is storage the caller already holds — a
/// parameter, an owned local, or a binding produced at an earlier
/// statement — so no producer operation precedes the pair. The pair has no
/// coordinate of its own; the destination and value are rejoined by the
/// store's source custody exactly as in `continues`.
pub(super) fn continues_bound_value(
    operations: &[CheckedUnitEffectOperationPlan],
    index: usize,
) -> bool {
    let move_index = match operations.get(index) {
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField { .. }) => index,
        Some(CheckedUnitEffectOperationPlan::StoreStructuralField { .. }) => {
            let Some(previous) = index.checked_sub(1) else {
                return false;
            };
            previous
        }
        _ => return false,
    };
    let (
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField {
            result: moved,
            source,
        }),
        Some(CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index,
            destination,
            value,
        }),
    ) = (operations.get(move_index), operations.get(move_index + 1))
    else {
        return false;
    };
    // The value is bound storage, not this statement's fresh result: the
    // moved-out hole's declared type is the value's own.
    matches!(
        value.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
    ) && moved.statement_index == *statement_index
        && source == destination
        && value.path.is_empty()
        && value.access == checked_trees::CheckedStructuralAccess::Owned
        && moved.type_identity == value.type_identity
}

/// Whether the cleanup at `index` disposes exactly the displaced value of
/// the replacement whose store immediately precedes it.
pub(super) fn continues_with_cleanup(
    operations: &[CheckedUnitEffectOperationPlan],
    index: usize,
) -> bool {
    let Some(store_index) = index.checked_sub(1) else {
        return false;
    };
    let (
        Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate,
            affine_discards,
        }),
        Some(CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index, ..
        }),
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField { result: moved, .. }),
    ) = (
        operations.get(index),
        operations.get(store_index),
        store_index
            .checked_sub(1)
            .and_then(|move_index| operations.get(move_index)),
    )
    else {
        return false;
    };
    (continues(operations, store_index) || continues_bound_value(operations, store_index))
        && coordinate.statement_index == *statement_index
        && coordinate.call_ordinal == 0
        && moved.multiplicity == Multiplicity::Affine
        && matches!(affine_discards.as_slice(), [discard]
        if discard.path.is_empty()
            && discard.type_identity == moved.type_identity
            && discard.source
                == checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: moved.binding_ordinal,
                })
}
