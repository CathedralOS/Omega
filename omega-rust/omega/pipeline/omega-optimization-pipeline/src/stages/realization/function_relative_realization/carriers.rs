use super::prelude::*;
use super::{assembly::final_layout, model::*};
use omega_selected_instructions_to_register_homes::{AllocationEvidence, RetainedAllocation};

/// Direct ordinary realization whose call, preservation, and return effects
/// are discharged by one exact canonical target frame. Frame requirements,
/// abstract storage, geometry, and byte protocol remain distinct retained
/// artifacts so validation can replay every join independently.
#[derive(Debug)]
pub struct StagedFixedFrameFunctionRelativeRealization {
    pub(super) homes: StagedOptimizedRegisterHomes,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) requirements: ValidatedAllocatedCalleeSavedRequirements,
    pub(super) storage: ValidatedNonAuthoritativeCalleeSaveStorage,
    pub(super) frame: ValidatedTargetFrameLayout,
    pub(super) protocol: ValidatedTargetFrameProtocolEncoding,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
}

impl StagedFixedFrameFunctionRelativeRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomes {
        &self.homes
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
    pub const fn requirements(&self) -> &ValidatedAllocatedCalleeSavedRequirements {
        &self.requirements
    }
    pub const fn storage(&self) -> &ValidatedNonAuthoritativeCalleeSaveStorage {
        &self.storage
    }
    pub const fn frame(&self) -> &ValidatedTargetFrameLayout {
        &self.frame
    }
    pub const fn protocol(&self) -> &ValidatedTargetFrameProtocolEncoding {
        &self.protocol
    }
    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }
    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: StagedOptimizedRegisterHomeCustodyReceipt,
    pub(super) machine: omega_machine_optimizer::PostAllocationMachineIdentity,
    pub(super) requirements: AllocatedCalleeSavedRequirementIdentity,
    pub(super) storage: NonAuthoritativeCalleeSaveStorageIdentity,
    pub(super) frame: TargetFrameLayoutIdentity,
    pub(super) protocol: TargetFrameProtocolEncodingIdentity,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.source
    }
    pub const fn machine(self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.machine
    }
    pub const fn requirements(self) -> AllocatedCalleeSavedRequirementIdentity {
        self.requirements
    }
    pub const fn storage(self) -> NonAuthoritativeCalleeSaveStorageIdentity {
        self.storage
    }
    pub const fn frame(self) -> TargetFrameLayoutIdentity {
        self.frame
    }
    pub const fn protocol(self) -> TargetFrameProtocolEncodingIdentity {
        self.protocol
    }
    pub const fn exit_contract(self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug)]
pub struct StagedPostAllocationMachineFunctionRelativeRealization {
    pub(super) allocation: RetainedAllocation,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) optimization: crate::StagedOptimizedPostAllocationMachineOptimization,
    pub(super) baseline_encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt,
}

impl StagedPostAllocationMachineFunctionRelativeRealization {
    pub const fn allocation(&self) -> &RetainedAllocation {
        &self.allocation
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn optimization(&self) -> &crate::StagedOptimizedPostAllocationMachineOptimization {
        &self.optimization
    }
    pub const fn baseline_encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.baseline_encoding
    }
    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }
    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
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
    ) -> &StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(
        &mut self,
    ) -> &mut ValidatedFunctionRelativeOptimizationRealizationManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
        &mut self.exit_contract
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: AllocationEvidence,
    pub(super) machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    pub(super) optimization: crate::PostAllocationMachineOptimizationCustody,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> &AllocationEvidence {
        &self.source
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }
    pub const fn optimization(&self) -> crate::PostAllocationMachineOptimizationCustody {
        self.optimization
    }
    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug)]
pub struct StagedSelectedLoweringFunctionRelativeRealization {
    pub(super) homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) relaxation: Option<StagedOptimizedX86BranchRelaxation>,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt,
}

impl StagedSelectedLoweringFunctionRelativeRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomesAfterSelectedLowering {
        &self.homes
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }
    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
    }
    pub const fn relaxation(&self) -> Option<&StagedOptimizedX86BranchRelaxation> {
        self.relaxation.as_ref()
    }
    pub fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        final_layout(&self.baseline_layout, self.relaxation.as_ref())
    }
    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }
    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
    pub const fn custody(
        &self,
    ) -> &StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(
        &mut self,
    ) -> &mut ValidatedFunctionRelativeOptimizationRealizationManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
        &mut self.exit_contract
    }
}

/// Function-relative realization reached directly from ordinary register homes
/// when the build selected a function-relative layout optimization but no
/// selected-lowering family. The absence of selected-lowering completion is
/// retained in its manifest and custody rather than synthesized.
#[derive(Debug)]
pub struct StagedFunctionRelativeLayoutOptimizationRealization {
    pub(super) homes: StagedOptimizedRegisterHomes,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) relaxation: StagedOptimizedX86BranchRelaxation,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt,
}

impl StagedFunctionRelativeLayoutOptimizationRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomes {
        &self.homes
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }
    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
    }
    pub const fn relaxation(&self) -> &StagedOptimizedX86BranchRelaxation {
        &self.relaxation
    }
    #[cfg(test)]
    pub(crate) fn relaxation_mut(&mut self) -> &mut StagedOptimizedX86BranchRelaxation {
        &mut self.relaxation
    }
    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        self.relaxation.layout()
    }
    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }
    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
    pub const fn custody(
        &self,
    ) -> &StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(
        &mut self,
    ) -> &mut ValidatedFunctionRelativeOptimizationRealizationManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
        &mut self.exit_contract
    }

    /// Test-only receipt corruption. The donor is already staged through the
    /// same public route; this method grants no construction or publication
    /// authority for either nested receipt.
    #[cfg(test)]
    pub(crate) fn corrupt_publication_custody_for_test(
        &mut self,
        field: FunctionRelativeLayoutPublicationCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            FunctionRelativeLayoutPublicationCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source;
            }
            FunctionRelativeLayoutPublicationCustodyFieldForTest::Machine => {
                self.custody.machine = donor.custody.machine.clone();
            }
            FunctionRelativeLayoutPublicationCustodyFieldForTest::Relaxation => {
                self.custody.relaxation = X86BranchRelaxationIdentity::from_bytes([0xa1; 32]);
            }
            FunctionRelativeLayoutPublicationCustodyFieldForTest::ExitContract => {
                self.custody.exit_contract =
                    WholeFunctionExitContractIdentity::from_bytes([0xa2; 32]);
            }
            FunctionRelativeLayoutPublicationCustodyFieldForTest::Realization => {
                self.custody.realization =
                    FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes([0xa3; 32]);
            }
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum FunctionRelativeLayoutPublicationCustodyFieldForTest {
    Source,
    Machine,
    Relaxation,
    ExitContract,
    Realization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
    pub(super) source: StagedOptimizedRegisterHomeCustodyReceipt,
    pub(super) machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    pub(super) relaxation: X86BranchRelaxationIdentity,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
    pub const fn source(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.source
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }
    pub const fn relaxation(&self) -> X86BranchRelaxationIdentity {
        self.relaxation
    }
    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    pub(super) machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        &self.source
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
