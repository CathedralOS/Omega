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
    omega_selected_instructions_to_register_homes::ValidatedFixedPrecoloredIntervals,
    omega_selected_instructions_to_register_homes::FixedPrecoloredIntervalError,
> {
    omega_selected_instructions_to_register_homes::analyze_fixed_precolored_intervals(
        source.live_range_stage().ranges(),
        source.legality(),
        omega_selected_instructions_to_register_homes::FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
        budget,
    )
}

pub(super) fn validate(
    source: &StagedOptimizedAllocationLegality,
    plan: omega_selected_instructions_to_register_homes::FixedPrecoloredIntervalPlan,
) -> Result<
    omega_selected_instructions_to_register_homes::ValidatedFixedPrecoloredIntervals,
    omega_selected_instructions_to_register_homes::FixedPrecoloredIntervalError,
> {
    omega_selected_instructions_to_register_homes::validate_fixed_precolored_intervals(
        source.live_range_stage().ranges(),
        source.legality(),
        plan,
    )
}
