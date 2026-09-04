use crate::tests::*;

pub(super) const X64_EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 3,
    candidates: 7,
    validation_steps: 59,
    commits: 6,
    iterations: 12,
};

pub(super) const ARM64_EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 3,
    candidates: 7,
    validation_steps: 104,
    commits: 6,
    iterations: 12,
};

pub(super) fn exact_budget(target: NativeTarget) -> OptimizationWorkBudget {
    let usage = if target == NativeTarget::linux_x64() {
        X64_EXACT_USAGE
    } else {
        ARM64_EXACT_USAGE
    };
    OptimizationWorkBudget::new(
        usage.rule_evaluations,
        usage.candidates,
        usage.validation_steps,
        usage.commits,
        usage.iterations,
    )
    .unwrap()
}

pub(super) struct SplitFixture {
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) fixed: omega_regalloc::ValidatedFixedPrecoloredIntervals,
}

pub(super) fn source(target: NativeTarget) -> SplitFixture {
    let selected = staged_forwarded_conditional(target);
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let source = stage_optimized_allocation_legality(ranges).unwrap();
    let fixed = omega_regalloc::analyze_fixed_precolored_intervals(
        source.live_range_stage().ranges(),
        source.legality(),
        omega_regalloc::FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
        generous_budget(),
    )
    .unwrap();
    SplitFixture { source, fixed }
}

pub(super) fn generous_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap()
}

pub(super) fn analyze(
    fixture: &SplitFixture,
    budget: OptimizationWorkBudget,
) -> Result<
    omega_regalloc::ValidatedFixedPrecoloredSplitRequirements,
    omega_regalloc::FixedPrecoloredSplitRequirementError,
> {
    omega_regalloc::analyze_fixed_precolored_split_requirements(
        fixture.source.live_range_stage().ranges(),
        fixture.source.legality(),
        &fixture.fixed,
        omega_regalloc::FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1,
        budget,
    )
}

pub(super) fn validate(
    fixture: &SplitFixture,
    plan: omega_regalloc::FixedPrecoloredSplitRequirementPlan,
) -> Result<
    omega_regalloc::ValidatedFixedPrecoloredSplitRequirements,
    omega_regalloc::FixedPrecoloredSplitRequirementError,
> {
    omega_regalloc::validate_fixed_precolored_split_requirements(
        fixture.source.live_range_stage().ranges(),
        fixture.source.legality(),
        &fixture.fixed,
        plan,
    )
}
