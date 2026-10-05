//! Independent validation of redundant-extension removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the removal's
//! legality from the source records — the named instruction's extension kind
//! and `[use, def]` shape against the bound constraint row, the result
//! register's origin naming this instruction, the input's unique producer and
//! defining operand, the normalization that producer's contract promises, the
//! identity table over extension form and promise, and the target's own copy
//! row — then rebuilds the function the contract demands and requires the
//! proposal to equal it. Restoring the extension instruction must reproduce
//! the complete source by content. A producer admission error therefore fails
//! validation even when the proposal is exactly what that producer emitted.

use std::sync::Arc;

use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{RedundantExtensionError, RedundantExtensionReceipt, ValidatedRedundantExtension};
use crate::ValidatedSelectedAnalysis;

/// The validation work this audit performs, in the measured-step contract the
/// family publishes: one step per block plus one per instruction across the
/// plan, then a second scan of the reconstructed function's instructions that
/// locates and counts the input's producers.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
) -> Result<u64, RedundantExtensionError> {
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())
            })
        })
        .ok_or(RedundantExtensionError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| RedundantExtensionError::IdentityOverflow)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_redundant_extension_removal(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    extension: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRedundantExtension, RedundantExtensionError> {
    Ok(ValidatedRedundantExtension {
        receipt: RedundantExtensionReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
            function_index,
            extension,
        },
        transformed: Arc::new(proposed),
    })
}
