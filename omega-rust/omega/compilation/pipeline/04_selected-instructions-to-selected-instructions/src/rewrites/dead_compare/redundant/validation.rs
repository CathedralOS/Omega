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

use super::super::admission::compare_removal_steps;
use super::{RedundantCompareError, RedundantCompareReceipt, ValidatedRedundantCompare};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: the shared compare-removal cone audit — the rewrite
/// scan, the reconstructed function's blocks together with its roster, call,
/// and access rows, the shared walk's setup and fixpoint bound, and the
/// backward operand audit at one stream scan plus the crossed edges'
/// transport surface per published unit. `units` is caller-supplied so
/// discovery can charge the contract before admission runs: the candidate's
/// published `implicit_defs`/`clobbers` union bounds it exactly.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    units: usize,
) -> Result<u64, RedundantCompareError> {
    let steps = compare_removal_steps(
        plan,
        function,
        successors,
        units,
        |cone| Ok(cone.per_unit_base),
        || RedundantCompareError::IdentityOverflow,
    )?;
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
