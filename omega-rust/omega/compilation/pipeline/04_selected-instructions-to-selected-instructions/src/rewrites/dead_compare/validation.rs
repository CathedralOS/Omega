//! Independent validation of dead-compare removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! compare kind and canonical flag-publisher shape against the bound
//! constraint row, the call and memory-access rosters that must not name
//! it, and the forward walk proving every condition-state unit it publishes
//! dies unread on every path — then rebuilds the function the contract
//! demands and requires the proposal to equal it. Restoring the removed
//! compare and the source settlements must reproduce the complete source
//! by content. A producer admission error therefore fails validation even
//! when the proposal is exactly what that producer emitted.
use std::sync::Arc;

use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};

use super::{DeadCompareError, DeadCompareReceipt, ValidatedDeadCompare};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::terminator_successors;
use crate::rewrites::receipt;

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction
/// across the plan, a second scan of the reconstructed function's blocks
/// together with its roster, call, and access rows, then the dead-unit
/// audit at two block traversals plus the successor edges per published
/// flag unit. `units` is caller-supplied so discovery can charge the
/// contract before admission runs: the candidate's published
/// `implicit_defs`/`clobbers` union bounds it exactly.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    units: usize,
) -> Result<u64, DeadCompareError> {
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(DeadCompareError::IdentityOverflow)?;
    let edge_count = function
        .blocks
        .iter()
        .map(|block| terminator_successors(&block.terminator).len())
        .sum::<usize>();
    let dead_audit = units
        .checked_mul(
            function_scan
                .checked_mul(2)
                .and_then(|scan| scan.checked_add(edge_count))
                .ok_or(DeadCompareError::IdentityOverflow)?,
        )
        .ok_or(DeadCompareError::IdentityOverflow)?;
    let steps = crate::rewrites::plan_instruction_scan(plan)
        .and_then(|total| {
            total
                .checked_add(function_scan)?
                .checked_add(function.virtual_registers.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.memory_accesses.len())
        })
        .and_then(|total| total.checked_add(dead_audit))
        .ok_or(DeadCompareError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| DeadCompareError::IdentityOverflow)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_dead_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedDeadCompare, DeadCompareError> {
    Ok(ValidatedDeadCompare {
        receipt: receipt!(DeadCompareReceipt, source, proposed, function_index: function_index, compare: compare),
        transformed: Arc::new(proposed),
    })
}
