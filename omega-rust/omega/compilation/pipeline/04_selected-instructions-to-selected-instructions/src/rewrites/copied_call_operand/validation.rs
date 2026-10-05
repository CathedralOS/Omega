//! Independent replay of a copied-call-operand reroute.
//!
//! The validator never consults
//! [`CopiedCallOperandRule`](super::pair::CopiedCallOperandRule) or the
//! declared table: it re-derives the whole grammar from the instruction
//! records — the consumer kind and its leading-`Use`, trailing-`Def`
//! operand roster, the named producer as the destination register's last
//! in-block definition under a clean `CopyI64` record, the source
//! register's freedom from intervening redefinition, the operand classes
//! against the roster, and the catalog surface the call row and the
//! isolated copy row must carry — then rebuilds the expected rerouted
//! record, requires the proposal to equal it, and restores the complete
//! source by content.

use std::sync::Arc;

use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{CopiedCallOperandError, CopiedCallOperandReceipt, ValidatedCopiedCallOperand};
use crate::ValidatedSelectedAnalysis;

/// The bounded-work charge of one copied-call-operand audit — the
/// whole-function consumer scan, the forward producer scan, the backward
/// last-definition walk, and the interval audit — shared by admission,
/// replay, and the executor's per-candidate budget accounting.
pub fn copied_call_operand_measured_steps(
    plan: &SelectedInstructionPlan,
    block: &selected_instructions::SelectedBlock,
) -> Result<u64, CopiedCallOperandError> {
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            total
                .checked_add(block.instructions.len())?
                .checked_add(block.instructions.len())
        })
        .ok_or(CopiedCallOperandError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| CopiedCallOperandError::IdentityOverflow)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_copied_call_operand_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    producer: SelectedInstructionId,
    call: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCopiedCallOperand, CopiedCallOperandError> {
    Ok(ValidatedCopiedCallOperand {
        receipt: CopiedCallOperandReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            function_index,
            producer,
            call,
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
