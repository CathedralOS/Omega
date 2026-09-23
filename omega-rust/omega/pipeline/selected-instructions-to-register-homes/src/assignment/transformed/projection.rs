use crate::PostAllocationSelectedTransformation;
use optimization_core::PrePhysicalOptimizationManifestIdentity;

use crate::{
    StagedOptimizedLiteralFoldCustodyReceipt, StagedPreAllocationOptimizationCustodyReceipt,
    StagedPreAllocationOptimizationRun, StagedSelectedLoweringOptimizationCustodyReceipt,
    StagedSelectedLoweringOptimizationRun,
};

pub(super) fn literal_fold_transformations(
    source: &StagedOptimizedLiteralFoldCustodyReceipt,
) -> Vec<PostAllocationSelectedTransformation> {
    source
        .transformations()
        .iter()
        .copied()
        .map(PostAllocationSelectedTransformation::LiteralFold)
        .collect()
}

pub(super) fn literal_fold_pre_physical(
    source: &StagedOptimizedLiteralFoldCustodyReceipt,
) -> PrePhysicalOptimizationManifestIdentity {
    source.source().manifest()
}

pub(super) fn selected_lowering_final_analysis(
    run: &StagedSelectedLoweringOptimizationRun,
) -> (
    &crate::ValidatedLiveRanges,
    &crate::ValidatedAllocationLegality,
) {
    match run.steps().last() {
        Some(step) => (step.ranges(), step.legality()),
        None => (
            run.source_legality_stage().ranges(),
            run.source_legality_stage().legality(),
        ),
    }
}

/// The pre-allocation run's final ranges and legality: the last committed
/// step's rebuilt facts, or the run's admitted analyses when the clean pass
/// found no admissible copy.
pub(super) fn pre_allocation_final_analysis(
    run: &StagedPreAllocationOptimizationRun,
) -> (
    &crate::ValidatedLiveRanges,
    &crate::ValidatedAllocationLegality,
) {
    (run.ranges(), run.legality())
}

pub(super) fn pre_allocation_transformations(
    source: &StagedPreAllocationOptimizationCustodyReceipt,
) -> Vec<PostAllocationSelectedTransformation> {
    source
        .iterations()
        .iter()
        .map(|iteration| match iteration.transformation() {
            selected_instructions_to_selected_instructions::PreAllocationTransformationIdentity::CopyRemoval(identity) => {
                PostAllocationSelectedTransformation::CopyRemoval(identity)
            }
            selected_instructions_to_selected_instructions::PreAllocationTransformationIdentity::RedundantExtension(identity) => {
                PostAllocationSelectedTransformation::RedundantExtension(identity)
            }
        })
        .collect()
}

pub(super) fn selected_lowering_transformations(
    source: &StagedSelectedLoweringOptimizationCustodyReceipt,
) -> Vec<PostAllocationSelectedTransformation> {
    source
        .iterations()
        .iter()
        .map(|iteration| PostAllocationSelectedTransformation::LiteralFold(iteration.fold()))
        .collect()
}
