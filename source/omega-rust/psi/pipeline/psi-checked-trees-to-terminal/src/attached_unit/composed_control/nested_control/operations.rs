//! Independent admission and emission of control-state operation prefixes.

use super::*;

pub(super) fn validate(state: &psi_checked_trees::CheckedComposedUnitControlStatePlan) -> bool {
    match state.operations.as_slice() {
        [] => true,
        [
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                structural_arguments,
                claim_transfers,
                ..
            },
        ] => {
            coordinate.statement_index == 0
                && coordinate.call_ordinal == 0
                && structural_arguments.is_empty()
                && claim_transfers.is_empty()
        }
        _ => false,
    }
}

pub(super) fn emit(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    targets: &[super::super::catalogs::LoweredComposedInternalTarget],
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    if state.operations.is_empty() {
        return Ok(());
    }
    super::super::internal_calls::emission::emit_call_operation(state, targets, operations)
}
