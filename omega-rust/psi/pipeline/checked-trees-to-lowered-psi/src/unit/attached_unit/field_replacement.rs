//! `place = value` over a structural field plans one roster: the producer of
//! the replacing value (the statement's call, or the establishment of its
//! construction), the displaced value's move-out, the store of the produced
//! value into the opened hole, and -- for an affine field -- the displaced
//! value's cleanup. Every member after the producer continues the producer's
//! statement; the destination and value are rejoined against the authored
//! assignment by the store's own source custody.

use super::{CheckedUnitEffectOperationPlan, Multiplicity};

/// Whether the operation at `index` is the move-out or store of a
/// replacement whose producer immediately precedes the move-out.
pub(super) fn continues(operations: &[CheckedUnitEffectOperationPlan], index: usize) -> bool {
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
        Some(producer),
    ) = (
        operations.get(move_index),
        operations.get(move_index + 1),
        move_index
            .checked_sub(1)
            .and_then(|producer| operations.get(producer)),
    )
    else {
        return false;
    };
    let produced = match producer {
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate, result, ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate, result, ..
        } if coordinate.call_ordinal == 0 && coordinate.statement_index == *statement_index => {
            result
        }
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            operand_source: None,
            ..
        } if result.statement_index == *statement_index => result,
        _ => return false,
    };
    moved.statement_index == *statement_index
        && source == destination
        && value.path.is_empty()
        && value.access == checked_trees::CheckedStructuralAccess::Owned
        && value.source_structural_result_binding_ordinal() == Some(produced.binding_ordinal)
        && moved.type_identity == produced.type_identity
        && moved.multiplicity == produced.multiplicity
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
    continues(operations, store_index)
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
