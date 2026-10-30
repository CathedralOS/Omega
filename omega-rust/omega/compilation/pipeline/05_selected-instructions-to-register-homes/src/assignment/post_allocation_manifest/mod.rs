//! Optimizer module role: executable entrance. Post-allocation manifest construction and independent admission entrance.
//!
//! Direct-home and selected-lowering routes join here. The durable record,
//! canonical identity, persistence, and human rendering live in
//! `register_homes::post_allocation_manifest`; reconstruction and validation
//! stay transform-local.

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

/// The one projection every route publishes through: it binds whichever
/// executed rewrite suites the source prefix ran under. Those suites are the
/// prefix's own fact, so the caller names them — a direct or fixed-view-copy
/// prefix ran none, a completed selected-lowering or pre-allocation run names
/// its own completion identity, and runtime-spill recovery over such a run
/// records that suite's completion beside the prefix ledger, so composed
/// custody does not downgrade the manifest to an unrun-suite shape.
pub(crate) fn project_post_allocation_optimization_manifest(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<ValidatedPostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    let record = reconstruction::expected_record(
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

/// Validated authority receipt for the durable manifest record. The transform
/// owns admission; the record itself lives in `register_homes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPostAllocationOptimizationManifest {
    record: PostAllocationOptimizationManifest,
}

impl ValidatedPostAllocationOptimizationManifest {
    receipt_accessor_rows!(borrow ValidatedPostAllocationOptimizationManifest {
        record: &PostAllocationOptimizationManifest,
    });
}
