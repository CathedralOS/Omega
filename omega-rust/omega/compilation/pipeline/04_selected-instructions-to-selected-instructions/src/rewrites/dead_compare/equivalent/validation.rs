//! Independent validation of equivalent-compare removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! compare kind and canonical flag-publisher shape against the bound
//! constraint row, the call and memory-access rosters that must not name
//! it, the shared reaching walk resolving every published unit's last
//! observed condition-state events, and the side-by-side proof that each
//! shadow site computes the victim's own subtraction — then the backward
//! walk refusing any write to an operand register the two compares share
//! by identity. From that record it rebuilds the function the contract
//! demands and requires the proposal to equal it, and restoring the
//! removed compare with the source settlements must reproduce the
//! complete source by content. A producer admission error therefore
//! fails validation even when the proposal is exactly what that producer
//! emitted.

use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};

use super::{EquivalentCompareError, EquivalentCompareReceipt, ValidatedEquivalentCompare};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{edge_surface, terminator_successors};
use crate::rewrites::sealed;

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction
/// across the plan, a second scan of the reconstructed function together
/// with its roster, call, and access rows, then the reaching walk's setup
/// — every edge's target resolution, the predecessor fill, and the cone
/// mark — plus per published unit the in-block prefix scan, the per-block
/// last-event table, the bounded fixpoint propagation, and the per-site
/// equivalence checks' unique-producer scans, and finally the backward
/// operand audit once per unit over stream positions and crossed edge
/// surfaces. `units` is caller-supplied so discovery can charge the
/// contract before admission runs: the candidate's published
/// `implicit_defs`/`clobbers` union bounds it exactly.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    units: usize,
) -> Result<u64, EquivalentCompareError> {
    let cone = crate::rewrites::cone_propagation(function, successors, || {
        EquivalentCompareError::IdentityOverflow
    })?;
    let function_scan = cone.function_scan;
    let edge_count = cone.edge_count;
    let per_unit = Some(cone.per_unit_base)
        .and_then(|total| {
            total.checked_add(
                cone.elements
                    .checked_mul(cone.function_scan)?
                    .checked_mul(4)?,
            )
        })
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
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
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let audit = units
        .checked_mul(
            function_scan
                .checked_add(edge_count)
                .and_then(|total| total.checked_add(transport_rows))
                .ok_or(EquivalentCompareError::IdentityOverflow)?,
        )
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let steps = crate::rewrites::plan_instruction_scan(plan)
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
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| EquivalentCompareError::IdentityOverflow)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_equivalent_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedEquivalentCompare, EquivalentCompareError> {
    Ok(
        sealed!(ValidatedEquivalentCompare, EquivalentCompareReceipt, source, proposed, function_index: function_index, compare: compare),
    )
}
