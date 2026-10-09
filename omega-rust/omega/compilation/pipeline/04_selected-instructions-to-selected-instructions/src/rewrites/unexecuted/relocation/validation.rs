//! Independent replay of member-run relocation: the validator
//! reconstructs the contiguous run the named members bound inside one
//! block's body, the destination block and landing index the destination
//! instruction names, and the window the move crosses on its own audit —
//! sharing no state with the producer's `admission` record — deriving
//! every acyclic path between the run block and the destination in the
//! direction that has them, re-proving that the move changes no traversal
//! (every predecessor edge into the arrival block lies on a crossed path
//! and every exit of every crossed block stays on one), and applying the shared
//! hazard, transport, memory-roster, and settlement audit once. It then
//! requires the proposed program to place exactly the run's members at
//! the landing index in their original order and restores the complete
//! source by content — every crossed instruction, every other block and
//! instruction, register, roster row, call, settlement, and edge
//! included.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};

use super::{MemberRunRelocationError, MemberRunRelocationReceipt, ValidatedMemberRunRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

/// Independently consume the proposed program: the validator re-derives
/// the admitted run, window and landing index from the source without
/// the producer's admission routine, the proposal must place exactly the
/// run's members on the landing index in the destination block in their
/// original order, and moving the run back must restore the complete
/// source by content — every crossed instruction, every other block and
/// instruction, register, roster row, call, settlement, and edge
/// included.
pub(crate) fn validate_member_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedMemberRunRelocation, MemberRunRelocationError> {
    let reconstructed = super::reconstruct(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let run_len = reconstructed.run_end - reconstructed.run_start + 1;
    // A forward in-block move counts its landing index in source order
    // and puts the run's last member on it; the run's transformed start
    // sits one width less the named slot behind.
    let insert_index = if reconstructed.run_block == reconstructed.destination_block
        && reconstructed.landing_index > reconstructed.run_end
    {
        reconstructed.landing_index - run_len + 1
    } else {
        reconstructed.landing_index
    };
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.destination_block))
        .and_then(|block| block.instructions.get(insert_index..insert_index + run_len))
        != Some(
            &reconstructed.function.blocks[reconstructed.run_block].instructions
                [reconstructed.run_start..=reconstructed.run_end],
        )
    {
        return Err(MemberRunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let run: Vec<_> = restored.functions[function_index].blocks[reconstructed.destination_block]
        .instructions
        .drain(insert_index..insert_index + run_len)
        .collect();
    restored.functions[function_index].blocks[reconstructed.run_block]
        .instructions
        .splice(reconstructed.run_start..reconstructed.run_start, run);
    if restored != *source.selected_plan() {
        return Err(MemberRunRelocationError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedMemberRunRelocation,
        MemberRunRelocationReceipt,
        source,
        proposed
    ))
}
