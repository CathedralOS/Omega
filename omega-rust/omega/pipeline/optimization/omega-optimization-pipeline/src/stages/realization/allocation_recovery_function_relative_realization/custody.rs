use crate::{
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, ValidatedFunctionRelativeOptimizationRealizationManifest,
    ValidatedWholeFunctionExitContract,
};

use super::model::StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt;
use super::source::StagedAllocationRecoverySourceCustodyReceipt;

pub(super) fn receipt(
    source: StagedAllocationRecoverySourceCustodyReceipt,
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
