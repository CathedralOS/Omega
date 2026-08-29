use omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity;

use crate::{
    FunctionRelativeOptimizationRealizationError,
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
    WholeFunctionExitContractError,
};

/// Explicit-staging-only completion of the active-resident rematerialization
/// vertical at the function-relative, frameless whole-function-exit boundary.
/// It grants no frame, emission, section, object, image, installation, or
/// publication authority.
#[derive(Debug)]
pub struct StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization {
    pub(super) source: StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody:
        StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization {
    pub const fn source(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
        &self.source
    }

    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }

    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }

    pub const fn custody(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt
    {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt {
    pub(super) source:
        StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    pub(super) exit_contract: crate::WholeFunctionExitContractIdentity,
    pub(super) realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt
    {
        &self.source
    }

    pub const fn exit_contract(&self) -> crate::WholeFunctionExitContractIdentity {
        self.exit_contract
    }

    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedActiveResidentRematerializationFunctionRelativeRealizationError {
    Source(OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError),
    ExitContract(WholeFunctionExitContractError),
    Manifest(FunctionRelativeOptimizationRealizationError),
    LaterPhaseSelected,
    RootMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display
    for OptimizedActiveResidentRematerializationFunctionRelativeRealizationError
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized active-resident rematerialization function-relative realization failed: {self:?}"
        )
    }
}

impl std::error::Error
    for OptimizedActiveResidentRematerializationFunctionRelativeRealizationError
{
}
