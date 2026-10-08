//! The one relocation admission: locate the contiguous member run the two
//! named members bound inside one block's body, resolve the destination
//! instruction to a landing position in its block, take the direction whose
//! acyclic paths exist — downstream to a reachable destination, upstream
//! from one that reaches the run's block — derive the window the move
//! crosses over every such path, prove the move changes no traversal —
//! every predecessor edge into the arrival block lies on a crossed path and
//! every exit of every crossed block stays on one — then apply the shared
//! hazard, transport, memory-roster and settlement audit once.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::MemberRunRelocationError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission {
    /// The run's own block, as an index into `function.blocks`.
    pub run_block: usize,
    /// First body index the run occupies.
    pub run_start: usize,
    /// Last body index the run occupies.
    pub run_end: usize,
    /// Index into `function.blocks` of the block the run lands in —
    /// `run_block` itself for the in-block case.
    pub destination_block: usize,
    /// The body index the run lands at, in the destination block's
    /// source-order positions.
    pub landing_index: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, MemberRunRelocationError> {
    let reconstructed = super::reconstruct(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    Ok(Admission {
        run_block: reconstructed.run_block,
        run_start: reconstructed.run_start,
        run_end: reconstructed.run_end,
        destination_block: reconstructed.destination_block,
        landing_index: reconstructed.landing_index,
    })
}
