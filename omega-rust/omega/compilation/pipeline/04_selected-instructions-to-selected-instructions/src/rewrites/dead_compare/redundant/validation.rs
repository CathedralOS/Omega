//! Independent validation of redundant-compare removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! compare kind and canonical flag-publisher shape against the bound
//! constraint row, the call and memory-access rosters that must not name
//! it, the flag-equivalent shadows every published unit's reaching events
//! must resolve to, and the backward walk proving no position exposed
//! between a unit's shadow sites and the compare writes a shared operand
//! register — then rebuilds the function the contract demands and
//! requires the proposal to equal it. Restoring the removed compare and
//! the source settlements must reproduce the complete source by content.
//! A producer admission error therefore fails validation even when the
//! proposal is exactly what that producer emitted.

use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};

use super::{RedundantCompareError, RedundantCompareReceipt, ValidatedRedundantCompare};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{edge_surface, terminator_successors};
use crate::rewrites::sealed;

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: the shared rewrite scan (one step per block
/// and instruction of the function, one per plan function), a second scan of the reconstructed function's blocks
/// together with its roster, call, and access rows, the shared walk's
/// setup and fixpoint bound, and the backward operand audit at one stream
/// scan plus the crossed edges' transport surface per published unit.
/// `units` is caller-supplied so discovery can charge the contract before
/// admission runs: the candidate's published `implicit_defs`/`clobbers`
/// union bounds it exactly.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    units: usize,
) -> Result<u64, RedundantCompareError> {
    let cone = crate::rewrites::cone_propagation(function, successors, || {
        RedundantCompareError::IdentityOverflow
    })?;
    let function_scan = cone.function_scan;
    let edge_count = cone.edge_count;
    let per_unit = cone.per_unit_base;
    let walk_setup = cone.walk_setup_base;
    let transport_rows = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            terminator_successors(&block.terminator)
                .iter()
                .try_fold(total, |total, successor| {
                    total.checked_add(edge_surface(successor))
                })
        })
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let audit = units
        .checked_mul(
            function_scan
                .checked_add(edge_count)
                .and_then(|total| total.checked_add(transport_rows))
                .ok_or(RedundantCompareError::IdentityOverflow)?,
        )
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            total
                .checked_add(function_scan)?
                .checked_add(function.virtual_registers.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.memory_accesses.len())
        })
        .and_then(|total| {
            total
                .checked_add(walk_setup)?
                .checked_add(units.checked_mul(per_unit)?)?
                .checked_add(audit)
        })
        .ok_or(RedundantCompareError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| RedundantCompareError::IdentityOverflow)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_redundant_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRedundantCompare, RedundantCompareError> {
    Ok(
        sealed!(ValidatedRedundantCompare, RedundantCompareReceipt, source, proposed, function_index: function_index, compare: compare),
    )
}
