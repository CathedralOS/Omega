use super::{SpillPressureProbe, analyze, assign};
use optimization_core::OptimizationUnitIdentity;
use register_environment::baseline_target_register_environment;
use register_homes::AllocatorAvailabilityPolicy;
use selected_instructions::VirtualRegisterId;
use selected_instructions_to_selected_instructions::{
    RuntimeSpillSpanPolicy, ValidatedRuntimeSpill, analyze_allocation_legality,
    analyze_live_ranges, analyze_liveness, materialize_allocator_availability,
    spill_selected_runtime_value, spill_selected_runtime_value_with_span_policy,
    test_support::{budget, runtime_spill_source},
};
use target::NativeTarget;

fn failed_crossing_probe(target: NativeTarget) -> (SpillPressureProbe, ValidatedRuntimeSpill) {
    let environment = baseline_target_register_environment(target).unwrap();
    let source = runtime_spill_source(target, OptimizationUnitIdentity::from_bytes([2; 32]));
    let all_views = materialize_allocator_availability(
        environment.identity(),
        target,
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
    )
    .unwrap();
    let class = source.transformed().functions[0].virtual_registers[0].class;
    let view = all_views
        .plan()
        .classes
        .iter()
        .find(|entry| entry.class == class)
        .unwrap()
        .unconstrained_views[0];
    let availability = materialize_allocator_availability(
        environment.identity(),
        target,
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views: vec![view] },
    )
    .unwrap();
    let liveness = analyze_liveness(&source).unwrap();
    let ranges = analyze_live_ranges(&source, &liveness).unwrap();
    let legality = analyze_allocation_legality(
        &ranges,
        &availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .unwrap();
    let crossing = spill_selected_runtime_value_with_span_policy(
        &source,
        0,
        VirtualRegisterId(1),
        &environment,
        budget(),
        RuntimeSpillSpanPolicy::UnitWriteCrossing,
    )
    .unwrap();
    let bounded =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    // Both policies independently admitted the same plan. One available view
    // leaves live values competing for a home, not lacking legal candidates.
    assert_eq!(crossing, bounded);
    assert!(!std::sync::Arc::ptr_eq(
        &crossing.shared_transformed(),
        &bounded.shared_transformed()
    ));
    let facts = analyze(
        &environment,
        &availability,
        &source,
        &liveness,
        &ranges,
        &legality,
        &crossing,
    )
    .unwrap();
    let failure = assign(&environment, &facts.ranges, &facts.legality).unwrap_err();
    assert!(matches!(
        failure,
        crate::RegisterHomeError::NoCompatibleHome { .. }
    ));
    let replayed = analyze(
        &environment,
        &availability,
        &source,
        &liveness,
        &ranges,
        &legality,
        &bounded,
    )
    .unwrap();
    assert_eq!(facts.liveness, replayed.liveness);
    assert_eq!(facts.ranges, replayed.ranges);
    assert_eq!(facts.legality, replayed.legality);
    assert_eq!(
        assign(&environment, &replayed.ranges, &replayed.legality),
        Err(failure.clone())
    );
    (
        SpillPressureProbe {
            rewrite: crossing,
            facts,
            failure,
        },
        bounded,
    )
}

#[test]
fn identical_spill_reuses_the_checked_facts_and_pressure() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (probe, bounded) = failed_crossing_probe(target);
        let original_ranges = probe.facts.ranges.clone();
        let original_legality = probe.facts.legality.clone();
        let expected_pressure = probe.failure.clone();
        let (facts, pressure) = probe.reuse_for(&bounded).unwrap();
        assert!(std::ptr::eq(facts.ranges.plan(), original_ranges.plan()));
        assert!(std::ptr::eq(
            facts.legality.plan(),
            original_legality.plan()
        ));
        assert_eq!(pressure, expected_pressure);
    }
}

#[test]
fn another_spill_program_cannot_reuse_the_probe() {
    let target = NativeTarget::linux_x64();
    let (probe, bounded) = failed_crossing_probe(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let source = runtime_spill_source(target, OptimizationUnitIdentity::from_bytes([2; 32]));
    let different =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(0), &environment, budget())
            .unwrap();
    assert_ne!(different.transformed(), bounded.transformed());
    assert!(probe.reuse_for(&different).is_none());
}

#[test]
fn same_program_under_another_proof_root_cannot_reuse_the_probe() {
    let target = NativeTarget::linux_x64();
    let (probe, bounded) = failed_crossing_probe(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let source = runtime_spill_source(target, OptimizationUnitIdentity::from_bytes([3; 32]));
    let different =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    assert_eq!(different.transformed(), bounded.transformed());
    assert_ne!(
        different.receipt().optimization_unit(),
        bounded.receipt().optimization_unit()
    );
    assert!(probe.reuse_for(&different).is_none());
}
