use omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity;
use omega_selected_instructions_to_register_homes::{AllocationReplayError, RetainedAllocation};

use crate::{
    AllocatedCalleeSavedRequirementIdentity, FunctionRelativeOptimizationRealizationError,
    NonAuthoritativeCalleeSaveStorageIdentity, OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomeCustodyReceipt, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, TargetFrameLayoutIdentity,
    TargetFrameProtocolEncodingIdentity, ValidatedAllocatedCalleeSavedRequirements,
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    ValidatedNonAuthoritativeCalleeSaveStorage, ValidatedTargetFrameLayout,
    ValidatedTargetFrameProtocolEncoding, ValidatedWholeFunctionExitContract,
    WholeFunctionExitContractIdentity,
};

/// Exact baseline realization for the currently admitted receiver-free Unit
/// semantic entry. This carrier proves function-relative bytes and exit
/// behavior only; it owns no source ProgramEntry signature, wrapper, process
/// entry, image, installation, or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedUnitFunctionRelativeRealization {
    pub(super) allocation: RetainedAllocation,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) frame: Option<UnitSavedReturnAddressFrame>,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt,
}

impl StagedOptimizedUnitFunctionRelativeRealization {
    pub const fn allocation(&self) -> &RetainedAllocation {
        &self.allocation
    }

    #[cfg(test)]
    pub(crate) fn allocation_mut(&mut self) -> &mut RetainedAllocation {
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

    pub const fn frame(&self) -> Option<&UnitSavedReturnAddressFrame> {
        self.frame.as_ref()
    }

    pub fn protocol(&self) -> Option<&ValidatedTargetFrameProtocolEncoding> {
        self.frame.as_ref().map(|frame| &frame.protocol)
    }

    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }

    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> &StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
        &mut self.exit_contract
    }
}

/// The incoming return address of an AArch64 Unit function occupies an exact
/// frame slot. The ordinary machine emitter saves it in every AArch64 Unit
/// function and the object boundary requires it, so the optimized route agrees
/// with them rather than taking the AAPCS64 leaf exemption. An x86-64 Unit
/// function returns through the caller's activation record and owns no frame,
/// so it carries none of this.
#[derive(Debug)]
pub struct UnitSavedReturnAddressFrame {
    pub(super) requirements: ValidatedAllocatedCalleeSavedRequirements,
    pub(super) storage: ValidatedNonAuthoritativeCalleeSaveStorage,
    pub(super) layout: ValidatedTargetFrameLayout,
    pub(super) protocol: ValidatedTargetFrameProtocolEncoding,
}

impl UnitSavedReturnAddressFrame {
    pub const fn requirements(&self) -> &ValidatedAllocatedCalleeSavedRequirements {
        &self.requirements
    }

    pub const fn storage(&self) -> &ValidatedNonAuthoritativeCalleeSaveStorage {
        &self.storage
    }

    pub const fn layout(&self) -> &ValidatedTargetFrameLayout {
        &self.layout
    }

    pub const fn protocol(&self) -> &ValidatedTargetFrameProtocolEncoding {
        &self.protocol
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitSavedReturnAddressFrameReceipt {
    pub(super) requirements: AllocatedCalleeSavedRequirementIdentity,
    pub(super) storage: NonAuthoritativeCalleeSaveStorageIdentity,
    pub(super) layout: TargetFrameLayoutIdentity,
    pub(super) protocol: TargetFrameProtocolEncodingIdentity,
}

impl UnitSavedReturnAddressFrameReceipt {
    pub const fn requirements(self) -> AllocatedCalleeSavedRequirementIdentity {
        self.requirements
    }

    pub const fn storage(self) -> NonAuthoritativeCalleeSaveStorageIdentity {
        self.storage
    }

    pub const fn layout(self) -> TargetFrameLayoutIdentity {
        self.layout
    }

    pub const fn protocol(self) -> TargetFrameProtocolEncodingIdentity {
        self.protocol
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: StagedOptimizedRegisterHomeCustodyReceipt,
    pub(super) machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    pub(super) frame: Option<UnitSavedReturnAddressFrameReceipt>,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.source
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }

    pub const fn frame(&self) -> Option<UnitSavedReturnAddressFrameReceipt> {
        self.frame
    }

    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }

    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedUnitFunctionRelativeRealizationError {
    Allocation(AllocationReplayError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Encoding(crate::OptimizedSelectedFormEncodingError),
    Layout(crate::OptimizedResolvedSelectedFormLayoutError),
    CalleeSavedRequirements(crate::AllocatedCalleeSavedRequirementError),
    CalleeSaveStorage(crate::NonAuthoritativeCalleeSaveStorageError),
    FrameLayout(crate::TargetFrameLayoutError),
    FrameProtocol(crate::TargetFrameProtocolEncodingError),
    Exit(crate::WholeFunctionExitContractError),
    UnsupportedSelectionPhase,
    UnsupportedUnitShape,
    RootMismatch,
    ReceiptMismatch,
    Manifest(FunctionRelativeOptimizationRealizationError),
}

impl std::fmt::Display for OptimizedUnitFunctionRelativeRealizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized Unit function-relative realization failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedUnitFunctionRelativeRealizationError {}
