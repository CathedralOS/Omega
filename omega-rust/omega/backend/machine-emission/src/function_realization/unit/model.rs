use optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity;
use physical_instructions::PostAllocationMachineCustodyReceipt;
use register_homes::RegisterHomeCustodyReceipt;
use selected_instructions_to_register_homes::{AllocationReplayError, RetainedAllocation};

use crate::ValidatedTargetFrameProtocolEncoding;
use crate::frame_layout::ValidatedTargetFrameLayout;
use crate::frame_layout::{
    NonAuthoritativeCalleeSaveStorageIdentity, ValidatedNonAuthoritativeCalleeSaveStorage,
};
use crate::{
    FunctionRelativeOptimizationRealizationError,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};
use machine_code::{
    TargetFrameLayoutIdentity, TargetFrameProtocolEncodingIdentity,
    WholeFunctionExitContractIdentity,
};
use post_allocation_machine_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;
use post_allocation_machine_to_resolved_layout::selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
};
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedRequirementIdentity, ValidatedAllocatedCalleeSavedRequirements,
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
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
    pub(super) source: RegisterHomeCustodyReceipt,
    pub(super) machine: PostAllocationMachineCustodyReceipt,
    pub(super) frame: Option<UnitSavedReturnAddressFrameReceipt>,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> RegisterHomeCustodyReceipt {
        self.source
    }

    pub const fn machine(&self) -> &PostAllocationMachineCustodyReceipt {
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
    Encoding(post_allocation_machine_to_resolved_layout::selected_form_encoding::OptimizedSelectedFormEncodingError),
    Layout(post_allocation_machine_to_resolved_layout::OptimizedResolvedSelectedFormLayoutError),
    CalleeSavedRequirements(
        selected_instructions_to_register_homes::AllocatedCalleeSavedRequirementError,
    ),
    CalleeSaveStorage(crate::frame_layout::NonAuthoritativeCalleeSaveStorageError),
    FrameLayout(crate::frame_layout::TargetFrameLayoutError),
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
