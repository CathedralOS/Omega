//! Independent admission and emission of control-state operation prefixes.

use super::*;

pub(super) fn validate(state: &psi_checked_trees::CheckedComposedUnitControlStatePlan) -> bool {
    state
        .operations
        .iter()
        .enumerate()
        .all(|(statement_index, operation)| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                structural_arguments,
                claim_transfers,
                ..
                } if usize::try_from(coordinate.statement_index).ok() == Some(statement_index)
                    && coordinate.call_ordinal == 0
                    && structural_arguments.is_empty()
                    && claim_transfers.is_empty()
            )
        })
}

pub(super) fn emit(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    targets: &[super::super::catalogs::LoweredComposedInternalTarget],
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    for operation in &state.operations {
        super::super::internal_calls::emission::emit_call_operation(
            operation, targets, operations,
        )?;
    }
    Ok(())
}
