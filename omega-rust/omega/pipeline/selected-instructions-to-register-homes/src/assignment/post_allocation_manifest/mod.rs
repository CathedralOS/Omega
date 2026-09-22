//! Optimizer module role: executable entrance. Post-allocation manifest construction and independent admission entrance.
//!
//! Direct-home and selected-lowering routes join here. The durable record,
//! canonical identity, persistence, and human rendering live in
//! `register_homes::post_allocation_manifest`; projection, reconstruction, and
//! validation stay transform-local.

mod projection;
mod reconstruction;
mod validation;

use optimization_core::{
    PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};

use crate::{ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedRegisterHomes};

pub use register_homes::post_allocation_manifest::*;

pub(crate) fn project_post_allocation_optimization_manifest(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    projection::project(
        pre_physical,
        None,
        None,
        selected_transformations,
        ranges,
        legality,
        homes,
    )
}

pub(crate) fn project_post_allocation_optimization_manifest_after_selected_lowering(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    completion: SelectedLoweringOptimizationCompletionIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    projection::project(
        pre_physical,
        Some(completion),
        None,
        selected_transformations,
        ranges,
        legality,
        homes,
    )
}

pub(crate) fn project_post_allocation_optimization_manifest_after_pre_allocation(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    completion: PreAllocationOptimizationCompletionIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    projection::project(
        pre_physical,
        None,
        Some(completion),
        selected_transformations,
        ranges,
        legality,
        homes,
    )
}

pub fn validate_post_allocation_optimization_manifest(
    candidate: &PostAllocationOptimizationManifest,
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    validation::validate(
        candidate,
        pre_physical,
        None,
        None,
        selected_transformations,
        ranges,
        legality,
        homes,
    )
}

pub(crate) fn validate_post_allocation_optimization_manifest_after_selected_lowering(
    candidate: &PostAllocationOptimizationManifest,
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    completion: SelectedLoweringOptimizationCompletionIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    validation::validate(
        candidate,
        pre_physical,
        Some(completion),
        None,
        selected_transformations,
        ranges,
        legality,
        homes,
    )
}

pub(crate) fn validate_post_allocation_optimization_manifest_after_pre_allocation(
    candidate: &PostAllocationOptimizationManifest,
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    completion: PreAllocationOptimizationCompletionIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    validation::validate(
        candidate,
        pre_physical,
        None,
        Some(completion),
        selected_transformations,
        ranges,
        legality,
        homes,
    )
}

/// Validated authority receipt for the durable manifest record. The transform
/// owns admission; the record itself lives in `register_homes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPostAllocationOptimizationManifest {
    record: PostAllocationOptimizationManifest,
}

impl ValidatedPostAllocationOptimizationManifest {
    pub const fn record(&self) -> &PostAllocationOptimizationManifest {
        &self.record
    }
}
