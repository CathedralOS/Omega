use crate::{
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};

use super::model::StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt;

pub(super) fn active_resident_realization_custody(
    source: StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt {
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt {
        source,
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    }
}
