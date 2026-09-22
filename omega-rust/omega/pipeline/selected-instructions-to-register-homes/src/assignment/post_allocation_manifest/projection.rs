use optimization_core::{
    PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};

use crate::{ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedRegisterHomes};

use super::{
    PostAllocationOptimizationManifestError, PostAllocationSelectedTransformation,
    ValidatedPostAllocationOptimizationManifest, reconstruction::expected_record,
};

pub(super) fn project(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    let record = expected_record(
        pre_physical,
        selected_lowering_completion,
        pre_allocation_completion,
        selected_transformations,
        ranges,
        legality,
        homes,
    )?;
    Ok(ValidatedPostAllocationOptimizationManifest { record })
}
