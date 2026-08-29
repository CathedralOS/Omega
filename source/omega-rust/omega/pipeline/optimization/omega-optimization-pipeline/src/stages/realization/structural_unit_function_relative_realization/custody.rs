use crate::{
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomeCustodyReceipt,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};

use super::model::StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt;

pub(super) fn structural_unit_realization_receipt(
    source: StagedOptimizedRegisterHomeCustodyReceipt,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
    StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
        source,
        machine: machine.custody().clone(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    }
}
