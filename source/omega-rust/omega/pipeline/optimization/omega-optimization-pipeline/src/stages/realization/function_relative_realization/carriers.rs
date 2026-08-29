use super::prelude::*;
use super::{assembly::final_layout, model::*};

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
}

/// Completed direct-homes realization for the exact AArch64 CBNZ
/// post-allocation transformation. Both baseline and transformed forms remain
/// owned so later custody can prove the four-byte change without treating the
/// transformed layout as baseline authority.
#[derive(Debug)]
pub struct StagedAarch64CbnzFunctionRelativeRealization {
    pub(super) homes: StagedOptimizedRegisterHomes,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) fusion: StagedOptimizedAarch64CbnzFusion,
    pub(super) baseline_encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
}

impl StagedAarch64CbnzFunctionRelativeRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomes {
        &self.homes
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn fusion(&self) -> &StagedOptimizedAarch64CbnzFusion {
        &self.fusion
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
    pub const fn custody(&self) -> &StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
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

/// Same completed CBNZ realization after a named selected-lowering run.
#[derive(Debug)]
pub struct StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization {
    pub(super) homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) fusion: StagedOptimizedAarch64CbnzFusion,
    pub(super) baseline_encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
}

impl StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomesAfterSelectedLowering {
        &self.homes
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }
    pub const fn fusion(&self) -> &StagedOptimizedAarch64CbnzFusion {
        &self.fusion
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
    ) -> &StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: StagedOptimizedRegisterHomeCustodyReceipt,
    pub(super) machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    pub(super) fusion: StagedOptimizedAarch64CbnzFusionCustodyReceipt,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    pub(super) machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    pub(super) fusion: StagedOptimizedAarch64CbnzFusionCustodyReceipt,
    pub(super) exit_contract: WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.source
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }
    pub const fn fusion(&self) -> StagedOptimizedAarch64CbnzFusionCustodyReceipt {
        self.fusion
    }
    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

impl StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        &self.source
    }
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }
    pub const fn fusion(&self) -> StagedOptimizedAarch64CbnzFusionCustodyReceipt {
        self.fusion
    }
    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }
    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
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
