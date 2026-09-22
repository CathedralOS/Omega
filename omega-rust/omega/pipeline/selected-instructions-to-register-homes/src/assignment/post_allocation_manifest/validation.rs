use optimization_core::{
    PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};

use crate::{ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedRegisterHomes};

use super::{
    PostAllocationOptimizationManifest, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, ValidatedPostAllocationOptimizationManifest,
    reconstruction::expected_record,
};

pub(super) fn validate(
    candidate: &PostAllocationOptimizationManifest,
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    if candidate.identity != candidate.recomputed_identity() {
        return Err(PostAllocationOptimizationManifestError::IdentityMismatch);
    }
    let expected = expected_record(
        pre_physical,
        selected_lowering_completion,
        pre_allocation_completion,
        selected_transformations,
        ranges,
        legality,
        homes,
    )?;
    if candidate != &expected {
        return Err(PostAllocationOptimizationManifestError::ContentMismatch);
    }
    Ok(ValidatedPostAllocationOptimizationManifest {
        record: candidate.clone(),
    })
}
