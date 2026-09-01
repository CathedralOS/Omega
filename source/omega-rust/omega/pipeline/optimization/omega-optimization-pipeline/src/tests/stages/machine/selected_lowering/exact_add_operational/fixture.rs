use crate::tests::*;

pub(super) fn targets() -> [(NativeTarget, &'static str); 2] {
    [
        (NativeTarget::linux_x64(), "rax"),
        (NativeTarget::linux_arm64(), "x0"),
    ]
}

pub(super) fn selections(enabled: bool) -> OptimizationSelections {
    OptimizationSelections::new([if enabled {
        Optimization::SelectedIncomingU12ExactAddImmediate
    } else {
        Optimization::SelectedIncomingU12ExactSubtractImmediate
    }])
    .unwrap()
}

pub(super) fn source(
    target: NativeTarget,
    sole_view_name: &str,
    enabled: bool,
    budget: OptimizationWorkBudget,
) -> StagedOptimizedAllocationLegality {
    source_with_selections(target, sole_view_name, selections(enabled), budget)
}

pub(super) fn source_with_selections(
    target: NativeTarget,
    sole_view_name: &str,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> StagedOptimizedAllocationLegality {
    let ranges = stage_optimized_live_ranges(
        stage_optimized_liveness(staged_exact_add_conditional_with_selections(
            target, selections, budget,
        ))
        .unwrap(),
    )
    .unwrap();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let sole_view = environment
        .physical()
        .model()
        .view_named(sole_view_name)
        .unwrap()
        .id;
    let availability = materialize_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
            views: vec![sole_view],
        },
    )
    .unwrap();
    stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap()
}

pub(super) fn run(
    target: NativeTarget,
    sole_view_name: &str,
    enabled: bool,
    budget: OptimizationWorkBudget,
) -> Result<StagedSelectedLoweringOptimizationRun, OptimizedLiteralFoldCustodyError> {
    run_selected_lowering_optimizations(source(target, sole_view_name, enabled, budget))
}
