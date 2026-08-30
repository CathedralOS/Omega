//! Optimizer module role: executable entrance. Post-allocation manifest construction and independent admission entrance.
//!
//! Direct-home and selected-lowering routes join here. Record shape, canonical
//! identity, persistence, reconstruction, validation, and human rendering
//! descend into named leaves.

mod codec;
mod error;
mod identity;
mod model;
mod projection;
mod reconstruction;
mod rendering;
mod validation;

use omega_optimization_core::{
    PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity,
};

use crate::{ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedRegisterHomes};

pub use error::*;
pub use model::*;

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

#[cfg(test)]
mod tests;
