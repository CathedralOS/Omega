//! Optimizer module role: executable entrance. Post-allocation manifest construction and independent admission entrance.
//!
//! Direct-home and selected-lowering routes join here. The durable record,
//! canonical identity, persistence, and human rendering live in
//! `register_homes::post_allocation_manifest`; projection, reconstruction, and
//! validation stay transform-local.

mod projection;
mod reconstruction;

use optimization_core::{
    PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};

use crate::ValidatedRegisterHomes;
use selected_instructions_to_selected_instructions::{
    ValidatedAllocationLegality, ValidatedLiveRanges,
};

pub use register_homes::post_allocation_manifest::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
    PostAllocationUnavailableData,
};

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

/// The general projection: binds whichever executed rewrite suites the
/// source prefix ran under. Runtime-spill recovery over a completed
/// selected-lowering or pre-allocation run records that suite's completion
/// identity beside the prefix ledger, so composed custody does not
/// downgrade the manifest to an unrun-suite shape.
pub(crate) fn project_post_allocation_optimization_manifest_with_suite_completions(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    projection::project(
        pre_physical,
        selected_lowering_completion,
        pre_allocation_completion,
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
