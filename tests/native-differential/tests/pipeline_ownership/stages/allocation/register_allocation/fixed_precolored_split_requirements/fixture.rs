use crate::tests::{
    NativeTarget, OptimizationWorkBudget, OptimizationWorkUsage, StagedOptimizedAllocationLegality,
    StagedOptimizedSelectedInstructions, stage_optimized_allocation_legality,
    stage_optimized_live_ranges, stage_optimized_liveness, staged_chained_forwarded,
    staged_forwarded_conditional, staged_joined_parameter, staged_joined_unbound,
};
pub(super) const X64_EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 7,
    candidates: 19,
    validation_steps: 217,
    commits: 8,
    iterations: 28,
};

pub(super) const ARM64_EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 7,
    candidates: 19,
    // Four fixed points plus fifteen domains of 27 allocatable views.
    validation_steps: 442,
    commits: 8,
    iterations: 28,
};

pub(super) const X64_CHAIN_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 4,
    candidates: 9,
    validation_steps: 104,
    commits: 5,
    iterations: 15,
};

pub(super) const ARM64_CHAIN_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 4,
    candidates: 9,
    validation_steps: 209,
    commits: 5,
    iterations: 15,
};

pub(super) const X64_JOIN_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 11,
    candidates: 24,
    validation_steps: 308,
    commits: 12,
    iterations: 37,
};

pub(super) const ARM64_JOIN_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 11,
    candidates: 24,
    validation_steps: 638,
    commits: 12,
    iterations: 37,
};

pub(super) fn exact_budget(target: NativeTarget) -> OptimizationWorkBudget {
    budget_for(if target == NativeTarget::linux_x64() {
        X64_EXACT_USAGE
    } else {
        ARM64_EXACT_USAGE
    })
}

pub(super) fn chain_exact_budget(target: NativeTarget) -> OptimizationWorkBudget {
    budget_for(if target == NativeTarget::linux_x64() {
        X64_CHAIN_USAGE
    } else {
        ARM64_CHAIN_USAGE
    })
}

pub(super) fn join_exact_budget(target: NativeTarget) -> OptimizationWorkBudget {
    budget_for(if target == NativeTarget::linux_x64() {
        X64_JOIN_USAGE
    } else {
        ARM64_JOIN_USAGE
    })
}

fn budget_for(usage: OptimizationWorkUsage) -> OptimizationWorkBudget {
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
    pub(super) fixed: selected_instructions_to_register_homes::ValidatedFixedPrecoloredIntervals,
}

pub(super) fn source(target: NativeTarget) -> SplitFixture {
    staged(staged_forwarded_conditional(target))
}

/// The `entry -> mid -> leaf` chain: the forwarded register's source range
/// keeps the parameter's entry view live through the pass-through middle
/// block, so its second connector does not originate at the source fragment.
pub(super) fn chain(target: NativeTarget) -> SplitFixture {
    staged(staged_chained_forwarded(target))
}

/// The two-way join whose block parameter is bound from a different register
/// on each incoming edge.
pub(super) fn joined(target: NativeTarget) -> SplitFixture {
    staged(staged_joined_parameter(target))
}

/// The same diamond with no parameter binding, so the forwarded register is
/// live into the join on both edges and carries two connectors into one block.
pub(super) fn joined_unbound(target: NativeTarget) -> SplitFixture {
    staged(staged_joined_unbound(target))
}

fn staged(selected: StagedOptimizedSelectedInstructions) -> SplitFixture {
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let source = stage_optimized_allocation_legality(ranges).unwrap();
    let fixed = selected_instructions_to_register_homes::analyze_fixed_precolored_intervals(
        source.live_range_stage().ranges(),
        source.legality(),
        register_homes::FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
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
    selected_instructions_to_register_homes::ValidatedFixedPrecoloredSplitRequirements,
    selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError,
> {
    selected_instructions_to_register_homes::analyze_fixed_precolored_split_requirements(
        fixture.source.live_range_stage().ranges(),
        fixture.source.legality(),
        &fixture.fixed,
        register_homes::FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1,
        budget,
    )
}

pub(super) fn validate(
    fixture: &SplitFixture,
    plan: register_homes::FixedPrecoloredSplitRequirementPlan,
) -> Result<
    selected_instructions_to_register_homes::ValidatedFixedPrecoloredSplitRequirements,
    selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError,
> {
    selected_instructions_to_register_homes::validate_fixed_precolored_split_requirements(
        fixture.source.live_range_stage().ranges(),
        fixture.source.legality(),
        &fixture.fixed,
        plan,
    )
}
