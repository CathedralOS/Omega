use crate::tests::*;

pub(super) fn staged(target: NativeTarget) -> StagedOptimizedRegisterHomes {
    stage_optimized_register_homes(
        stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_conditional(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

pub(super) fn validate(
    source: &StagedOptimizedRegisterHomes,
    candidate: &PostAllocationOptimizationManifest,
) -> Result<
    omega_selected_instructions_to_register_homes::ValidatedPostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestError,
> {
    let legality = source.legality_stage();
    let ranges = legality.live_range_stage();
    validate_post_allocation_optimization_manifest(
        candidate,
        source.custody().manifest(),
        &[],
        ranges.ranges(),
        legality.legality(),
        source.homes(),
    )
}
