use crate::tests::{
    FixedViewCopyPolicy, NativeTarget, OptimizationWorkBudget, OptimizedFixedViewCopyCustodyError,
    StagedOptimizedFixedViewCopies, stage_optimized_allocation_legality,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
    stage_optimized_live_ranges, stage_optimized_liveness, staged_forwarded_conditional,
};

pub(super) const POLICY: FixedViewCopyPolicy =
    FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1;

pub(super) fn targets() -> [NativeTarget; 2] {
    [NativeTarget::linux_x64(), NativeTarget::linux_arm64()]
}

pub(super) fn run(
    target: NativeTarget,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
    let source = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let source = stage_optimized_fixed_precolored_segment_homes(
        source,
        OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap(),
    )
    .unwrap();
    stage_optimized_fixed_view_copies(source, POLICY, budget)
}
