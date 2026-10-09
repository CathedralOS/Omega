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

use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};

use super::{CopiedCallOperandError, CopiedCallOperandReceipt, ValidatedCopiedCallOperand};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

/// The bounded-work charge of one copied-call-operand audit — the
/// whole-function consumer scan, the forward producer scan, the backward
/// last-definition walk, and the interval audit — shared by admission,
/// replay, and the executor's per-candidate budget accounting.
pub(crate) fn copied_call_operand_measured_steps(
    plan: &SelectedInstructionPlan,
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
) -> Result<u64, CopiedCallOperandError> {
    let steps = crate::rewrites::rewrite_scan(plan, function)
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
    Ok(sealed!(
        ValidatedCopiedCallOperand,
        CopiedCallOperandReceipt,
        source,
        proposed,
        function_index: function_index,
        producer: producer,
        call: call
    ))
}
