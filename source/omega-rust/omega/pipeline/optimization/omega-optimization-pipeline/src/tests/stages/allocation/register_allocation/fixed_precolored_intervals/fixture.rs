use crate::tests::*;

pub(super) const EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 1,
    candidates: 4,
    validation_steps: 6,
    commits: 4,
    iterations: 2,
};

pub(super) fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1, 4, 6, 4, 2).unwrap()
}

pub(super) fn source(target: NativeTarget) -> StagedOptimizedAllocationLegality {
    stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

pub(super) fn analyze(
    source: &StagedOptimizedAllocationLegality,
    budget: OptimizationWorkBudget,
) -> Result<
    omega_regalloc::ValidatedFixedPrecoloredIntervals,
    omega_regalloc::FixedPrecoloredIntervalError,
> {
    omega_regalloc::analyze_fixed_precolored_intervals(
        source.live_range_stage().ranges(),
        source.legality(),
        omega_regalloc::FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
        budget,
    )
}

pub(super) fn validate(
    source: &StagedOptimizedAllocationLegality,
    plan: omega_regalloc::FixedPrecoloredIntervalPlan,
) -> Result<
    omega_regalloc::ValidatedFixedPrecoloredIntervals,
    omega_regalloc::FixedPrecoloredIntervalError,
> {
    omega_regalloc::validate_fixed_precolored_intervals(
        source.live_range_stage().ranges(),
        source.legality(),
        plan,
    )
}
