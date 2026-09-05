use super::super::prelude::*;
use super::super::{carriers::*, model::*};

pub(in crate::function_realization) fn custody_receipt(
    source: &PostSelectedLoweringHomeCustodyReceipt,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
        source: source.clone(),
        machine: machine.custody().clone(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}

pub(in crate::function_realization) fn direct_custody_receipt(
    source: RegisterHomeCustodyReceipt,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    relaxation: &StagedOptimizedX86BranchRelaxation,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
    StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
        source,
        machine: machine.custody().clone(),
        relaxation: relaxation.identity(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}
