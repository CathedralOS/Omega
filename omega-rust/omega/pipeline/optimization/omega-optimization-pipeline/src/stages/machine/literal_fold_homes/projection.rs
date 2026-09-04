use omega_optimization_core::PrePhysicalOptimizationManifestIdentity;
use omega_regalloc::PostAllocationSelectedTransformation;

use crate::{
    StagedOptimizedLiteralFoldCustodyReceipt, StagedSelectedLoweringOptimizationCustodyReceipt,
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
    &omega_regalloc::ValidatedLiveRanges,
    &omega_regalloc::ValidatedAllocationLegality,
) {
    match run.steps().last() {
        Some(step) => (step.ranges(), step.legality()),
        None => (
            run.source_legality_stage().live_range_stage().ranges(),
            run.source_legality_stage().legality(),
        ),
    }
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
