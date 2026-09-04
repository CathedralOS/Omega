use omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity;

use crate::{
    FunctionRelativeOptimizationRealizationError, OptimizedPostAllocationMachinePipelineError,
    OptimizedRegisterHomeCustodyError, StagedOptimizedPostAllocationMachineCustodyReceipt,
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, ValidatedFunctionRelativeOptimizationRealizationManifest,
    ValidatedWholeFunctionExitContract, WholeFunctionExitContractIdentity,
};

/// Owning function-relative custody for the bounded structural-signature Unit
/// route. The internal call remains a typed unresolved MachineId fixup; this
/// carrier grants no section placement, object relocation, or executable-byte
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedStructuralUnitFunctionRelativeRealization {
    pub(super) homes: StagedOptimizedRegisterHomes,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt,
}

impl StagedOptimizedStructuralUnitFunctionRelativeRealization {
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

    #[cfg(test)]
    pub(crate) fn layout_mut(&mut self) -> &mut StagedOptimizedResolvedSelectedFormLayout {
        &mut self.layout
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
        &mut self.exit_contract
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(
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
    Homes(OptimizedRegisterHomeCustodyError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Encoding(crate::OptimizedSelectedFormEncodingError),
    Layout(crate::OptimizedResolvedSelectedFormLayoutError),
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
