use super::model::*;
use super::prelude::*;
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

impl StagedFixedFrameFunctionRelativeRealization {
    pub fn relaxation(&self) -> Option<&StagedOptimizedX86BranchRelaxation> {
        self.layout_optimization.relaxation()
    }
    #[cfg(feature = "test-support")]
    pub fn relaxation_mut(&mut self) -> Option<&mut StagedOptimizedX86BranchRelaxation> {
        self.layout_optimization.relaxation_mut_for_test()
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
    #[cfg(any(test, feature = "test-support"))]
    pub fn encoding_mut(&mut self) -> &mut StagedOptimizedSelectedFormEncoding {
        &mut self.encoding
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn baseline_layout_mut(&mut self) -> &mut StagedOptimizedResolvedSelectedFormLayout {
        &mut self.baseline_layout
    }
    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }
    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
    }
    pub fn layout(&self) -> &ResolvedMachineLayout {
        self.layout_optimization.layout()
    }
    pub fn layout_optimization(&self) -> &ResolvedLayoutOptimization {
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
    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }
    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> &StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
        &self.custody
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

impl StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> &AllocationEvidence {
        &self.source
    }
    pub const fn machine(&self) -> physical_instructions::PostAllocationMachineIdentity {
        self.machine
    }
    pub const fn requirements(&self) -> AllocatedCalleeSavedRequirementIdentity {
        self.requirements
    }
    pub const fn storage(&self) -> NonAuthoritativeCalleeSaveStorageIdentity {
        self.storage
    }
    pub const fn frame(&self) -> TargetFrameLayoutIdentity {
        self.frame
    }
    pub const fn protocol(&self) -> TargetFrameProtocolEncodingIdentity {
        self.protocol
    }
    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}
