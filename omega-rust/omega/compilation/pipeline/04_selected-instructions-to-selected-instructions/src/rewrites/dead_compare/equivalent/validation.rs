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

use super::super::admission::compare_removal_steps;
use super::{EquivalentCompareError, EquivalentCompareReceipt, ValidatedEquivalentCompare};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: the shared compare-removal cone audit — the rewrite
/// scan, the reconstructed function's roster, call, and access rows, the
/// reaching walk's setup, the backward operand audit over stream positions
/// and crossed edge surfaces, and the shadow-transport rows per published
/// unit — plus the per-site equivalence checks' unique-producer scans.
/// `units` is caller-supplied so discovery can charge the contract before
/// admission runs: the candidate's published `implicit_defs`/`clobbers`
/// union bounds it exactly.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    units: usize,
) -> Result<u64, EquivalentCompareError> {
    let steps = compare_removal_steps(
        plan,
        function,
        successors,
        units,
        |cone| {
            cone.per_unit_base
                .checked_add(
                    cone.elements
                        .checked_mul(cone.function_scan)
                        .and_then(|product| product.checked_mul(4))
                        .ok_or(EquivalentCompareError::IdentityOverflow)?,
                )
                .ok_or(EquivalentCompareError::IdentityOverflow)
        },
        || EquivalentCompareError::IdentityOverflow,
    )?;
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
