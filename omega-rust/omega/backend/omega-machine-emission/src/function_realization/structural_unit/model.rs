use omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity;
use omega_selected_instructions_to_register_homes::{AllocationReplayError, RetainedAllocation};

use crate::{
    FunctionRelativeOptimizationRealizationError,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};
use omega_machine_code::WholeFunctionExitContractIdentity;
use omega_post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use omega_register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
};
use omega_selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;
use omega_selected_instructions_to_register_homes::StagedOptimizedRegisterHomeCustodyReceipt;

/// Owning function-relative custody for the bounded structural-signature Unit
/// route. The internal call remains a typed unresolved MachineId fixup; this
/// carrier grants no section placement, object relocation, or executable-byte
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedStructuralUnitFunctionRelativeRealization {
    pub(super) allocation: RetainedAllocation,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt,
}

impl StagedOptimizedStructuralUnitFunctionRelativeRealization {
    pub const fn allocation(&self) -> &RetainedAllocation {
        &self.allocation
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn allocation_mut(&mut self) -> &mut RetainedAllocation {
        &mut self.allocation
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
    ) -> &StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn layout_mut(&mut self) -> &mut StagedOptimizedResolvedSelectedFormLayout {
        &mut self.layout
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
        &mut self.exit_contract
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn manifest_mut(
        &mut self,
    ) -> &mut ValidatedFunctionRelativeOptimizationRealizationManifest {
        &mut self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: StagedOptimizedRegisterHomeCustodyReceipt,
    pub(super) machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.source
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }

    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }

    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedStructuralUnitFunctionRelativeRealizationError {
    Allocation(AllocationReplayError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Encoding(
        omega_post_allocation_machine_to_selected_form_encoding::OptimizedSelectedFormEncodingError,
    ),
    Layout(
        omega_selected_form_encoding_to_resolved_layout::OptimizedResolvedSelectedFormLayoutError,
    ),
    Exit(crate::WholeFunctionExitContractError),
    UnsupportedSelectionPhase,
    UnsupportedStructuralUnitShape,
    RootMismatch,
    ReceiptMismatch,
    Manifest(FunctionRelativeOptimizationRealizationError),
}

impl std::fmt::Display for OptimizedStructuralUnitFunctionRelativeRealizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized structural Unit function-relative realization failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedStructuralUnitFunctionRelativeRealizationError {}
