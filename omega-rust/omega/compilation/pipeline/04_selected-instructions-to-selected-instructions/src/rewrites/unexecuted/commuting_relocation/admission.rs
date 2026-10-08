//! Shared admission for the in-block commuting run relocation: locate the
//! named `first_member` and `last_member` bounding one contiguous run
//! inside a block's body, locate the named destination instruction outside
//! the run in the same block, and prove the window they bound independent —
//! no register or condition-state hazard between any member and any crossed
//! position, every roster row that newly trades order commuting with every
//! row of the position it crosses, no call, hosted-effect, or terminator
//! barrier anywhere in the window, and no boundary settlement inside its
//! span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::CommutingRelocationError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission {
    pub block_index: usize,
    /// The run's first member index in the block body.
    pub first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    pub last_index: usize,
    /// The index whose instruction the run displaces: the window the
    /// relocation crosses is `first_index..=destination_index` in either
    /// order, with the destination always outside the run.
    pub destination_index: usize,
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, CommutingRelocationError> {
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
        block_index: reconstructed.block_index,
        first_index: reconstructed.first_index,
        last_index: reconstructed.last_index,
        destination_index: reconstructed.destination_index,
    })
}
