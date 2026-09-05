use crate::{
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

use super::model::StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt;
use selected_instructions_to_register_homes::AllocationEvidence;

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
