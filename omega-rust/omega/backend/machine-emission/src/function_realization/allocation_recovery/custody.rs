use crate::{
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};
use post_allocation_machine_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;
use post_allocation_machine_to_resolved_layout::selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes::AllocationEvidence;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::model::StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt;

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
