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
    pub(super) fixed: selected_instructions_to_register_homes::ValidatedFixedPrecoloredIntervals,
    pub(super) requirements:
        selected_instructions_to_register_homes::ValidatedFixedPrecoloredSplitRequirements,
}

pub(super) fn source(target: NativeTarget) -> HomeFixture {
    let selected = staged_forwarded_conditional(target);
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let source = stage_optimized_allocation_legality(ranges).unwrap();
    let fixed = selected_instructions_to_register_homes::analyze_fixed_precolored_intervals(
        source.live_range_stage().ranges(),
        source.legality(),
        selected_instructions_to_register_homes::FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
        generous_budget(),
    )
    .unwrap();
    let requirements = selected_instructions_to_register_homes::analyze_fixed_precolored_split_requirements(
        source.live_range_stage().ranges(),
        source.legality(),
        &fixed,
        selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1,
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
    selected_instructions_to_register_homes::ValidatedFixedPrecoloredSegmentHomes,
    selected_instructions_to_register_homes::FixedPrecoloredSegmentHomeError,
> {
    let environment = fixture
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    selected_instructions_to_register_homes::assign_fixed_precolored_segment_homes(
        fixture.source.live_range_stage().ranges(),
        fixture.source.legality(),
        &fixture.fixed,
        &fixture.requirements,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        selected_instructions_to_register_homes::FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1,
        budget,
    )
}

pub(super) fn validate(
    fixture: &HomeFixture,
    plan: selected_instructions_to_register_homes::FixedPrecoloredSegmentHomePlan,
) -> Result<
    selected_instructions_to_register_homes::ValidatedFixedPrecoloredSegmentHomes,
    selected_instructions_to_register_homes::FixedPrecoloredSegmentHomeError,
> {
    let environment = fixture
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    selected_instructions_to_register_homes::validate_fixed_precolored_segment_homes(
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
