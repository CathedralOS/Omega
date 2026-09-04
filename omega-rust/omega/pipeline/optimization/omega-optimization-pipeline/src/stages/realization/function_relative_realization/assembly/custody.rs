use super::super::prelude::*;
use super::super::{carriers::*, model::*};

pub(in crate::stages::realization::function_relative_realization) fn custody_receipt(
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
    StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt {
        source: homes.custody().clone(),
        machine: machine.custody().clone(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}

pub(in crate::stages::realization::function_relative_realization) fn direct_custody_receipt(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    relaxation: &StagedOptimizedX86BranchRelaxation,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
    StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt {
        source: homes.custody(),
        machine: machine.custody().clone(),
        relaxation: relaxation.identity(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}
