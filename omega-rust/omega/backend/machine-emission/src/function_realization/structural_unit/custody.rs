use crate::{
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};
use register_homes::RegisterHomeCustodyReceipt;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::model::StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt;

pub(super) fn structural_unit_realization_receipt(
    source: RegisterHomeCustodyReceipt,
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
