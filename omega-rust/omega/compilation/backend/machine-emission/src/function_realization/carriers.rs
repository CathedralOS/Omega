use super::ValidatedFunctionRelativeOptimizationRealizationManifest;
use crate::exit_contract::ValidatedWholeFunctionExitContract;
use crate::frame_layout::{
    NonAuthoritativeCalleeSaveStorageIdentity, ValidatedNonAuthoritativeCalleeSaveStorage,
    ValidatedTargetFrameLayout,
};
use crate::frame_protocol::ValidatedTargetFrameProtocolEncoding;
use machine_code::{
    ResolvedMachineLayout, TargetFrameLayoutIdentity, TargetFrameProtocolEncodingIdentity,
    WholeFunctionExitContractIdentity,
};
use optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use resolved_layout_to_resolved_layout::{
    ResolvedLayoutOptimization, StagedOptimizedX86BranchRelaxation,
};
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedRequirementIdentity, ValidatedAllocatedCalleeSavedRequirements,
};
use selected_instructions_to_register_homes::{AllocationEvidence, RetainedAllocation};

/// Direct ordinary realization whose call, preservation, and return effects
/// are discharged by one exact canonical target frame. Frame requirements,
/// abstract storage, geometry, and byte protocol remain distinct retained
/// artifacts so validation can replay every join independently.
#[derive(Debug)]
pub struct StagedFixedFrameFunctionRelativeRealization {
    pub(super) allocation: RetainedAllocation,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) layout_optimization: ResolvedLayoutOptimization,
    pub(super) frame: super::FunctionRelativeFrame,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
}

optimization_core::record_views! {
    record StagedFixedFrameFunctionRelativeRealization;
    borrowed {
        allocation: RetainedAllocation,
        machine: StagedOptimizedPostAllocationMachinePlan,
        encoding: StagedOptimizedSelectedFormEncoding,
        baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
        exit_contract: ValidatedWholeFunctionExitContract,
        manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
        custody: StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
    }
}

/// Frame parts are read through the retained frame, and the layout parts
/// through the retained layout optimization; those reads are not field views.
impl StagedFixedFrameFunctionRelativeRealization {
    pub fn relaxation(&self) -> Option<&StagedOptimizedX86BranchRelaxation> {
        self.layout_optimization.relaxation()
    }
    pub fn layout(&self) -> &ResolvedMachineLayout {
        self.layout_optimization.layout()
    }
    pub(crate) fn layout_optimization(&self) -> &ResolvedLayoutOptimization {
        &self.layout_optimization
    }
    pub const fn requirements(&self) -> &ValidatedAllocatedCalleeSavedRequirements {
        self.frame.requirements()
    }
    pub const fn storage(&self) -> &ValidatedNonAuthoritativeCalleeSaveStorage {
        self.frame.storage()
    }
    pub const fn frame(&self) -> &ValidatedTargetFrameLayout {
        self.frame.layout()
    }
    pub const fn protocol(&self) -> &ValidatedTargetFrameProtocolEncoding {
        self.frame.protocol()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: AllocationEvidence,
    pub(super) machine: physical_instructions::PostAllocationMachineIdentity,
    pub(super) requirements: AllocatedCalleeSavedRequirementIdentity,
    pub(super) storage: NonAuthoritativeCalleeSaveStorageIdentity,
    pub(super) frame: TargetFrameLayoutIdentity,
    pub(super) protocol: TargetFrameProtocolEncodingIdentity,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

optimization_core::record_views! {
    record StagedFixedFrameFunctionRelativeRealizationCustodyReceipt;
    shared {
        machine: physical_instructions::PostAllocationMachineIdentity,
        requirements: AllocatedCalleeSavedRequirementIdentity,
        storage: NonAuthoritativeCalleeSaveStorageIdentity,
        frame: TargetFrameLayoutIdentity,
        protocol: TargetFrameProtocolEncodingIdentity,
        exit_contract: WholeFunctionExitContractIdentity,
        realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    }
    borrowed {
        source: AllocationEvidence,
    }
}
