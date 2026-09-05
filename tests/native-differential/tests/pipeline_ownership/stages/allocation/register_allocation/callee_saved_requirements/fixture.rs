use crate::tests::*;

pub(super) fn call_homes(target: NativeTarget) -> StagedOptimizedRegisterHomes {
    stage_optimized_register_homes(
        stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_scalar_call_unit(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

pub(super) fn ordinary_homes(target: NativeTarget) -> StagedOptimizedRegisterHomes {
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

pub(super) fn stage(
    source: &StagedOptimizedRegisterHomes,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError> {
    stage_allocated_callee_saved_requirements(
        source,
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
        budget,
    )
}

pub(super) fn wide_budget() -> optimization_core::OptimizationWorkBudget {
    optimization_core::OptimizationWorkBudget::new(
        1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
    )
    .unwrap()
}

pub(super) fn exact_budget(
    usage: optimization_core::OptimizationWorkUsage,
) -> optimization_core::OptimizationWorkBudget {
    optimization_core::OptimizationWorkBudget::new(
        usage.rule_evaluations,
        usage.candidates,
        usage.validation_steps,
        usage.commits,
        usage.iterations,
    )
    .unwrap()
}
