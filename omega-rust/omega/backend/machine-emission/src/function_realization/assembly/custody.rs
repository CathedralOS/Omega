use super::super::prelude::*;
use super::super::{carriers::*, model::*};

pub(in crate::function_realization) fn custody_receipt(
    source: &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
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
