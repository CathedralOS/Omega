//! Optimizer module role: executable entrance. Post-allocation manifest construction and independent admission entrance.
//!
//! Direct-home and selected-lowering routes join here. The durable record,
//! canonical identity, persistence, and human rendering live in
//! `register_homes::post_allocation_manifest`; projection, reconstruction, and
//! validation stay transform-local.

mod model;
mod projection;
mod reconstruction;
mod validation;

use optimization_core::{
    PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity,
};

use crate::{ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedRegisterHomes};

pub use model::*;
pub use register_homes::post_allocation_manifest::*;

pub fn project_post_allocation_optimization_manifest(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    projection::project(
        pre_physical,
        None,
        selected_transformations,
        ranges,
        legality,
        homes,
    )
}

pub fn project_post_allocation_optimization_manifest_after_selected_lowering(
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
        selected_transformations,
        ranges,
        legality,
        homes,
    )
}

pub fn validate_post_allocation_optimization_manifest_after_selected_lowering(
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
        selected_transformations,
        ranges,
        legality,
        homes,
    )
}
