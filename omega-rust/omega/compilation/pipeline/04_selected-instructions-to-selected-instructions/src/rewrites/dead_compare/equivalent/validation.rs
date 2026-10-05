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
use std::sync::Arc;

use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{EquivalentCompareError, EquivalentCompareReceipt, ValidatedEquivalentCompare};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{edge_surface, terminator_successors};

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
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let elements = function_scan
        .checked_add(1)
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(EquivalentCompareError::IdentityOverflow)?,
        )
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .and_then(|total| total.checked_add(elements.checked_mul(function_scan)?.checked_mul(4)?))
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(EquivalentCompareError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or(EquivalentCompareError::IdentityOverflow)?;
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
    Ok(ValidatedEquivalentCompare {
        receipt: EquivalentCompareReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
            function_index,
            compare,
        },
        transformed: Arc::new(proposed),
    })
}
