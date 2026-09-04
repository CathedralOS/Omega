use omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity;

use crate::{
    FunctionRelativeOptimizationRealizationError, OptimizedPostAllocationMachinePipelineError,
    OptimizedRegisterHomeCustodyError, StagedOptimizedPostAllocationMachineCustodyReceipt,
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, ValidatedFunctionRelativeOptimizationRealizationManifest,
    ValidatedWholeFunctionExitContract, WholeFunctionExitContractIdentity,
};

/// Exact baseline realization for the currently admitted receiver-free Unit
/// semantic entry. This carrier proves function-relative bytes and exit
/// behavior only; it owns no source ProgramEntry signature, wrapper, process
/// entry, image, installation, or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedUnitFunctionRelativeRealization {
    pub(super) homes: StagedOptimizedRegisterHomes,
    pub(super) machine: StagedOptimizedPostAllocationMachinePlan,
    pub(super) encoding: StagedOptimizedSelectedFormEncoding,
    pub(super) layout: StagedOptimizedResolvedSelectedFormLayout,
    pub(super) exit_contract: ValidatedWholeFunctionExitContract,
    pub(super) manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    pub(super) custody: StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt,
}

impl StagedOptimizedUnitFunctionRelativeRealization {
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

    pub const fn custody(&self) -> &StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
        &mut self.exit_contract
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
    pub(super) source: StagedOptimizedRegisterHomeCustodyReceipt,
    pub(super) machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
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

    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }

    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedUnitFunctionRelativeRealizationError {
    Homes(OptimizedRegisterHomeCustodyError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Encoding(crate::OptimizedSelectedFormEncodingError),
    Layout(crate::OptimizedResolvedSelectedFormLayoutError),
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
