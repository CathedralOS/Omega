use optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity;
use register_homes::AllocationEvidence;

use crate::{
    FunctionRelativeOptimizationRealizationError,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
    WholeFunctionExitContractError,
};
use post_allocation_machine_to_resolved_layout::selected_form_encoding::{
    OptimizedSelectedFormEncodingError, StagedOptimizedSelectedFormEncoding,
};
use post_allocation_machine_to_resolved_layout::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
};
use register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
};

use selected_instructions_to_register_homes::{AllocationReplayError, RetainedAllocation};

/// Final frameless, function-relative custody for one allocation-recovery
/// transformation. It grants no section, object, installation, or publication
/// authority.
#[derive(Debug)]
pub struct StagedAllocationRecoveryFunctionRelativeRealization {
    pub(super) allocation: RetainedAllocation,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt,
}

impl StagedAllocationRecoveryFunctionRelativeRealization {
    pub const fn allocation(&self) -> &RetainedAllocation {
        &self.allocation
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }
    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
    }
    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }
    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
    pub const fn custody(
        &self,
    ) -> &StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: AllocationEvidence,
    pub(super) machine: physical_instructions::PostAllocationMachineIdentity,
    pub(super) encoding: machine_code::SelectedFormEncodingIdentity,
    pub(super) layout: machine_code::ResolvedSelectedFormLayoutIdentity,
    pub(super) exit_contract: machine_code::WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> &AllocationEvidence {
        &self.source
    }
    pub const fn machine(&self) -> physical_instructions::PostAllocationMachineIdentity {
        self.machine
    }
    pub const fn encoding(&self) -> machine_code::SelectedFormEncodingIdentity {
        self.encoding
    }
    pub const fn layout(&self) -> machine_code::ResolvedSelectedFormLayoutIdentity {
        self.layout
    }
    pub const fn exit_contract(&self) -> machine_code::WholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationRecoveryFunctionRelativeRealizationError {
    Allocation(AllocationReplayError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Encoding(OptimizedSelectedFormEncodingError),
    Layout(OptimizedResolvedSelectedFormLayoutError),
    ExitContract(WholeFunctionExitContractError),
    Manifest(FunctionRelativeOptimizationRealizationError),
    UnsupportedSelections,
    RootMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for AllocationRecoveryFunctionRelativeRealizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "allocation-recovery function-relative realization failed: {self:?}"
        )
    }
}

impl std::error::Error for AllocationRecoveryFunctionRelativeRealizationError {}
