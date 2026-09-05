use omega_machine_optimizer::{
    PostAllocationMachineError, ValidatedPostAllocationMachinePlan,
    ValidatedPreAllocationMachineEffects,
};
use omega_physical_instructions::PostAllocationMachineIdentity;
use omega_selected_instructions_to_machine_effects::MachineEffectStageError;
use omega_selected_instructions_to_register_homes::{AllocationEvidence, AllocationReplayError};

/// Home-aware machine facts joined only through independently replayed source
/// custody. This remains non-emission and non-publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachinePlan {
    pub(super) effects: ValidatedPreAllocationMachineEffects,
    pub(super) machine: ValidatedPostAllocationMachinePlan,
    pub(super) custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
}

impl StagedOptimizedPostAllocationMachinePlan {
    pub const fn effects(&self) -> &ValidatedPreAllocationMachineEffects {
        &self.effects
    }

    pub const fn machine(&self) -> &ValidatedPostAllocationMachinePlan {
        &self.machine
    }

    pub const fn custody(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachineCustodyReceipt {
    pub(super) source: AllocationEvidence,
    pub(super) effects: omega_machine_optimizer::PreAllocationMachineEffectIdentity,
    pub(super) machine: PostAllocationMachineIdentity,
    pub(super) function_count: usize,
    pub(super) structural_unit_function_count: usize,
    pub(super) instruction_count: usize,
    pub(super) operand_count: usize,
    pub(super) unit_action_count: usize,
}

impl StagedOptimizedPostAllocationMachineCustodyReceipt {
    pub const fn source(&self) -> &AllocationEvidence {
        &self.source
    }
    pub const fn effects(&self) -> omega_machine_optimizer::PreAllocationMachineEffectIdentity {
        self.effects
    }
    pub const fn machine(&self) -> PostAllocationMachineIdentity {
        self.machine
    }
    pub const fn function_count(&self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(&self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn instruction_count(&self) -> usize {
        self.instruction_count
    }
    pub const fn operand_count(&self) -> usize {
        self.operand_count
    }
    pub const fn unit_action_count(&self) -> usize {
        self.unit_action_count
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
