//! Independent validation of store mutation motion.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! motion's legality from the source records — the named instruction's
//! store kind against its single write row, the byte extent the row's
//! place and range spell (or the fixed byte a resolved sequence index
//! lands it on), the operand surface the route declares, the forward walk
//! that lands the store immediately before the first position that must
//! stay ordered after it, the crossed-edge audits that keep the carried
//! registers and the moved storage untouched, and the boundary-settlement
//! remap the landing demands — then rebuilds the function the contract
//! demands and requires the proposal to equal it. Moving the store back
//! must reproduce the complete source by content. A producer admission
//! error therefore fails validation even when the proposal is exactly
//! what that producer emitted.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};

use super::{StoreMutationMotionError, StoreMutationMotionReceipt, ValidatedStoreMutationMotion};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

/// Build the function the contract demands from the validator's own
/// record: the named store moved from its source position to the proven
/// landing, the roster retained unchanged — its rows name instructions
/// by identity, so the move keeps them — and the target block's
/// settlements shifted over the inserted ordinal. The producer's
/// `rewrite` is not consulted; both sides derive the same function from
/// the source alone.
fn expect(
    reconstructed: &super::Reconstructed<'_>,
) -> Result<SelectedFunction, StoreMutationMotionError> {
    let mut expected = reconstructed.function.clone();
    expected.boundary_settlements = super::moved_boundary_settlements(
        reconstructed.function,
        reconstructed.block,
        reconstructed.store_index,
        expected.blocks[reconstructed.target_block_index].id,
        reconstructed.insert_index,
    )?;
    let moved = expected.blocks[reconstructed.block_index]
        .instructions
        .remove(reconstructed.store_index);
    expected.blocks[reconstructed.target_block_index]
        .instructions
        .insert(reconstructed.insert_index, moved);
    Ok(expected)
}

/// Move the store back and restore the source settlements: undoing the
/// validator's expected edit must restore the complete source by
/// content — every other function, block, instruction, register, call,
/// and access retained bit-identical.
fn restore(
    reconstructed: &super::Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, StoreMutationMotionError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(StoreMutationMotionError::ReplayMismatch)?;
    let target = function
        .blocks
        .get_mut(reconstructed.target_block_index)
        .ok_or(StoreMutationMotionError::ReplayMismatch)?;
    if reconstructed.insert_index >= target.instructions.len() {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    let moved_back = target.instructions.remove(reconstructed.insert_index);
    let source_block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(StoreMutationMotionError::ReplayMismatch)?;
    if reconstructed.store_index > source_block.instructions.len() {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    source_block
        .instructions
        .insert(reconstructed.store_index, moved_back);
    // The proposed settlements already matched the shifted source, so
    // restoring the source rows restores the source function.
    function.boundary_settlements = reconstructed.function.boundary_settlements.clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator
/// reconstructs the motion's preconditions and landing from the source,
/// requires the proposal to equal the function its own record produces,
/// and restores the complete source by content. The producer's admission
/// routine is never consulted, so a wrong legality decision fails here
/// even when the proposal matches the edit the producer emitted.
pub(crate) fn validate_store_mutation_motion(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedStoreMutationMotion, StoreMutationMotionError> {
    let reconstructed = super::reconstruct(source, function_index, store, environment)?;
    if super::measured_steps(
        source.selected_plan(),
        reconstructed.function,
        &reconstructed,
    )? > budget.validation_steps()
    {
        return Err(StoreMutationMotionError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)?) {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedStoreMutationMotion,
        StoreMutationMotionReceipt,
        source,
        proposed
    ))
}
