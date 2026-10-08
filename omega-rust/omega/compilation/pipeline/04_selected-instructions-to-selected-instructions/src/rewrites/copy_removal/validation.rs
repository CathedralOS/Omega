//! Independent validation of copy removal.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! `CopyI64` shape against the bound constraint row, the destination's
//! copy-claimed roster origin, every destination mention across body,
//! terminator, transport, storage, access, call, and roster surfaces, and
//! the source register's freedom from intervening redefinition — then
//! rebuilds the function the contract demands and requires the proposal to
//! equal it. Restoring the removed copy, the roster row, and the source
//! settlements must reproduce the complete source by content. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.

use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};

use super::{CopyRemovalError, CopyRemovalReceipt, ValidatedCopyRemoval};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

/// The validation work this audit performs, in the measured-step contract the
/// family publishes: one step per block plus one per instruction across the
/// plan, a second scan of the reconstructed function's blocks counting each
/// successor transport, and one step per roster, storage, access, and call
/// row.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
) -> Result<u64, CopyRemovalError> {
    let steps = super::copy_scan(plan, function).ok_or(CopyRemovalError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| CopyRemovalError::IdentityOverflow)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_copy_removal(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    copy: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCopyRemoval, CopyRemovalError> {
    Ok(
        sealed!(ValidatedCopyRemoval, CopyRemovalReceipt, source, proposed, function_index: function_index, copy: copy),
    )
}
