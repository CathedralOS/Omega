use crate::tests::{
    NativeTarget, OptimizationWorkBudget, OptimizationWorkUsage, StagedOptimizedAllocationLegality,
    stage_optimized_allocation_legality, stage_optimized_live_ranges, stage_optimized_liveness,
    staged_forwarded_conditional,
};
pub(super) const EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 14,
    candidates: 194,
    validation_steps: 310,
    commits: 15,
    iterations: 153,
};

pub(super) fn exact_usage(target: NativeTarget) -> OptimizationWorkUsage {
    if target == NativeTarget::linux_x64() {
        EXACT_USAGE
    } else {
        OptimizationWorkUsage {
            candidates: 839,
            validation_steps: 1030,
            iterations: 273,
            ..EXACT_USAGE
        }
    }
}

pub(super) struct HomeFixture {
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) fixed:
        selected_instructions_to_selected_instructions::ValidatedFixedPrecoloredIntervals,
    pub(super) requirements:
        selected_instructions_to_selected_instructions::ValidatedFixedPrecoloredSplitRequirements,
}

pub(super) fn source(target: NativeTarget) -> HomeFixture {
    let selected = staged_forwarded_conditional(target);
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let source = stage_optimized_allocation_legality(ranges).unwrap();
    let fixed = selected_instructions_to_selected_instructions::analyze_fixed_precolored_intervals(
        source.live_range_stage().ranges(),
        source.legality(),
        register_homes::FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
        generous_budget(),
    )
    .unwrap();
    let requirements =
        selected_instructions_to_selected_instructions::analyze_fixed_precolored_split_requirements(
            source.live_range_stage().ranges(),
            source.legality(),
            &fixed,
            register_homes::FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1,
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
    selected_instructions_to_selected_instructions::ValidatedFixedPrecoloredSegmentHomes,
    selected_instructions_to_selected_instructions::FixedPrecoloredSegmentHomeError,
> {
    let environment = fixture
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    selected_instructions_to_selected_instructions::assign_fixed_precolored_segment_homes(
        fixture.source.live_range_stage().ranges(),
        fixture.source.legality(),
        &fixture.fixed,
        &fixture.requirements,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        register_homes::FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1,
        budget,
    )
}

pub(super) fn validate(
    fixture: &HomeFixture,
    plan: register_homes::FixedPrecoloredSegmentHomePlan,
) -> Result<
    selected_instructions_to_selected_instructions::ValidatedFixedPrecoloredSegmentHomes,
    selected_instructions_to_selected_instructions::FixedPrecoloredSegmentHomeError,
> {
    let environment = fixture
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    selected_instructions_to_selected_instructions::validate_fixed_precolored_segment_homes(
        fixture.source.live_range_stage().ranges(),
        fixture.source.legality(),
        &fixture.fixed,
        &fixture.requirements,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        plan,
    )
}
