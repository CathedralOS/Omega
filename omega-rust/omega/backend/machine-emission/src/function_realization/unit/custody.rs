use crate::{
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};
use register_homes::RegisterHomeCustodyReceipt;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::model::{
    StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt, UnitSavedReturnAddressFrame,
    UnitSavedReturnAddressFrameReceipt,
};

pub(super) fn unit_realization_receipt(
    source: RegisterHomeCustodyReceipt,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    frame: Option<&UnitSavedReturnAddressFrame>,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
    StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
        source,
        machine: machine.custody().clone(),
        frame: frame.map(|frame| UnitSavedReturnAddressFrameReceipt {
            requirements: frame.requirements().receipt().identity(),
            storage: frame.storage().receipt().identity(),
            layout: frame.layout().receipt().identity(),
            protocol: frame.protocol().receipt().identity(),
        }),
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    }
}
