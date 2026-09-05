use omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity;

use crate::{
    FunctionRelativeOptimizationRealizationError,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
    WholeFunctionExitContractError,
};
use omega_post_allocation_machine_to_selected_form_encoding::{
    OptimizedSelectedFormEncodingError, StagedOptimizedSelectedFormEncoding,
};
use omega_register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
};
use omega_selected_form_encoding_to_resolved_layout::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
};

use omega_selected_instructions_to_register_homes::{
    AllocationEvidence, AllocationReplayError, RetainedAllocation,
};

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
    pub(super) machine: omega_physical_instructions::PostAllocationMachineIdentity,
    pub(super) encoding: omega_machine_code::SelectedFormEncodingIdentity,
    pub(super) layout: omega_machine_code::ResolvedSelectedFormLayoutIdentity,
    pub(super) exit_contract: omega_machine_code::WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> &AllocationEvidence {
        &self.source
    }
    pub const fn machine(&self) -> omega_physical_instructions::PostAllocationMachineIdentity {
        self.machine
    }
    pub const fn encoding(&self) -> omega_machine_code::SelectedFormEncodingIdentity {
        self.encoding
    }
    pub const fn layout(&self) -> omega_machine_code::ResolvedSelectedFormLayoutIdentity {
        self.layout
    }
    pub const fn exit_contract(&self) -> omega_machine_code::WholeFunctionExitContractIdentity {
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
