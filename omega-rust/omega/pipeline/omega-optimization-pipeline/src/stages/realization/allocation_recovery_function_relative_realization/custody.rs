use crate::{
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, ValidatedFunctionRelativeOptimizationRealizationManifest,
    ValidatedWholeFunctionExitContract,
};

use super::model::StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt;
use omega_selected_instructions_to_register_homes::AllocationEvidence;

pub(super) fn receipt(
    source: AllocationEvidence,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt {
    StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt {
        source,
        machine: machine.machine().receipt().identity(),
        encoding: encoding.identity(),
        layout: layout.identity(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    }
}
