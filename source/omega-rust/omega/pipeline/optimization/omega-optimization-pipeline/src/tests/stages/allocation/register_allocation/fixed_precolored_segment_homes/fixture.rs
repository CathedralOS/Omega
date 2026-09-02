use crate::tests::*;

pub(super) const EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 8,
    candidates: 11,
    validation_steps: 29,
    commits: 9,
    iterations: 28,
};

pub(super) struct HomeFixture {
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) fixed: omega_regalloc::ValidatedFixedPrecoloredIntervals,
    pub(super) requirements: omega_regalloc::ValidatedFixedPrecoloredSplitRequirements,
}

pub(super) fn source(target: NativeTarget) -> HomeFixture {
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
    let requirements = omega_regalloc::analyze_fixed_precolored_split_requirements(
        source.live_range_stage().ranges(),
        source.legality(),
        &fixed,
        omega_regalloc::FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1,
        generous_budget(),
    )
    .unwrap();
    HomeFixture {
        source,
        fixed,
        requirements,
    }
}

pub(super) fn generous_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap()
}

pub(super) fn assign(
    fixture: &HomeFixture,
    budget: OptimizationWorkBudget,
) -> Result<
    omega_regalloc::ValidatedFixedPrecoloredSegmentHomes,
    omega_regalloc::FixedPrecoloredSegmentHomeError,
> {
    let environment = fixture
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    omega_regalloc::assign_fixed_precolored_segment_homes(
        fixture.source.live_range_stage().ranges(),
        fixture.source.legality(),
        &fixture.fixed,
        &fixture.requirements,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        omega_regalloc::FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1,
        budget,
    )
}

pub(super) fn validate(
    fixture: &HomeFixture,
    plan: omega_regalloc::FixedPrecoloredSegmentHomePlan,
) -> Result<
    omega_regalloc::ValidatedFixedPrecoloredSegmentHomes,
    omega_regalloc::FixedPrecoloredSegmentHomeError,
> {
    let environment = fixture
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    omega_regalloc::validate_fixed_precolored_segment_homes(
        fixture.source.live_range_stage().ranges(),
        fixture.source.legality(),
        &fixture.fixed,
        &fixture.requirements,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        plan,
    )
}
