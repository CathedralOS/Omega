use crate::{PostAllocationMachineError, ValidatedPostAllocationMachinePlan};
pub use physical_instructions::PostAllocationMachineCustodyReceipt;
use selected_instructions_to_register_homes::AllocationReplayError;
use selected_instructions_to_register_homes::MachineEffectStageError;
use selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects;

/// Home-aware machine facts joined only through independently replayed source
/// custody. This remains non-emission and non-publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachinePlan {
    pub(super) effects: ValidatedPreAllocationMachineEffects,
    pub(super) machine: ValidatedPostAllocationMachinePlan,
    pub(super) custody: PostAllocationMachineCustodyReceipt,
}

impl StagedOptimizedPostAllocationMachinePlan {
    pub const fn effects(&self) -> &ValidatedPreAllocationMachineEffects {
        &self.effects
    }

    pub const fn machine(&self) -> &ValidatedPostAllocationMachinePlan {
        &self.machine
    }

    pub const fn custody(&self) -> &PostAllocationMachineCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachinePipelineError {
    Allocation(AllocationReplayError),
    MachineEffects(MachineEffectStageError),
    PostAllocation(PostAllocationMachineError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostAllocationMachinePipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-allocation machine staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostAllocationMachinePipelineError {}
