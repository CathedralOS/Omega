use std::sync::Arc;

use crate::analyses::allocation_legality as legality;
use crate::analyses::allocation_legality::{
    ValidatedAllocationLegality, analyze_allocation_legality, analyze_allocation_legality_reusing,
};
use crate::rewrites::test_support::{budget, runtime_spill_source};
use crate::{
    ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedRuntimeSpill,
    analyze_live_ranges, analyze_liveness, materialize_allocator_availability,
    spill_selected_runtime_value,
};
use optimization_core::OptimizationUnitIdentity;
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_homes::AllocatorAvailabilityPolicy;
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::MachineId;
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

fn source(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut selected = runtime_spill_source(target, OptimizationUnitIdentity::from_bytes([2; 32]));
    let mut second = selected.transformed().functions[0].clone();
    second.machine = MachineId::new(2).unwrap();
    Arc::make_mut(&mut selected.transformed)
        .functions
        .push(second);
    refresh_identity(&mut selected);
    selected
}

fn refresh_identity(selected: &mut ValidatedRuntimeSpill) {
    let identity = selected_instruction_plan_identity(selected.transformed());
    selected.receipt.source_selected = identity;
    selected.receipt.transformed_selected = identity;
}

fn ranges(selected: &ValidatedRuntimeSpill) -> ValidatedLiveRanges {
    analyze_live_ranges(selected, &analyze_liveness(selected).unwrap()).unwrap()
}

fn availability(
    environment: &ValidatedTargetRegisterEnvironment,
    target: NativeTarget,
) -> ValidatedAllocatorAvailability {
    materialize_allocator_availability(
        environment.identity(),
        target,
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
    )
    .unwrap()
}

fn analyze(
    ranges: &ValidatedLiveRanges,
    environment: &ValidatedTargetRegisterEnvironment,
    availability: &ValidatedAllocatorAvailability,
    previous: Option<(&ValidatedLiveRanges, &ValidatedAllocationLegality)>,
) -> Result<ValidatedAllocationLegality, legality::AllocationLegalityError> {
    match previous {
        Some((prior_ranges, prior_legality)) => analyze_allocation_legality_reusing(
            prior_ranges,
            prior_legality,
            ranges,
            availability,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
        ),
        None => analyze_allocation_legality(
            ranges,
            availability,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
        ),
    }
}

#[test]
fn legality_reuse_matches_full_analysis_for_spills_and_machine_constraints() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let availability = availability(&environment, target);
        let previous = source(target);
        let prior_ranges = ranges(&previous);
        let prior_legality = analyze(&prior_ranges, &environment, &availability, None).unwrap();
        for mutation in 0..5 {
            let mut selected = previous.clone();
            match mutation {
                0 => {}
                1 => {
                    selected = spill_selected_runtime_value(
                        &previous,
                        0,
                        VirtualRegisterId(1),
                        &environment,
                        budget(),
                    )
                    .unwrap()
                }
                2 => Arc::make_mut(&mut selected.transformed).functions[0].blocks[0].instructions
                    [0]
                .implicit_uses
                .push(environment.physical().model().units[0].id),
                3 => {
                    Arc::make_mut(&mut selected.transformed).functions[0].blocks[0].instructions
                        [0]
                    .operands[1]
                        .early_clobber = true
                }
                4 => {
                    let functions = &mut Arc::make_mut(&mut selected.transformed).functions;
                    let first = functions[0].clone();
                    functions[0] = functions[1].clone();
                    functions[1] = first;
                }
                _ => unreachable!(),
            }
            refresh_identity(&mut selected);
            let current_ranges = ranges(&selected);
            legality::compute::REUSED_FUNCTIONS.set(0);
            let reused = analyze(
                &current_ranges,
                &environment,
                &availability,
                Some((&prior_ranges, &prior_legality)),
            )
            .unwrap();
            assert_eq!(
                legality::compute::REUSED_FUNCTIONS.get(),
                match mutation {
                    0 => 2,
                    4 => 0,
                    _ => 1,
                }
            );
            assert_eq!(
                reused,
                analyze(&current_ranges, &environment, &availability, None).unwrap()
            );
        }
    }
}

#[test]
fn changed_legality_roots_disable_proposal_reuse() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let availability = availability(&environment, target);
    let ranges = ranges(&source(target));
    let legality = analyze(&ranges, &environment, &availability, None).unwrap();
    for mutation in 0..3 {
        let mut prior = legality.clone();
        let plan = Arc::make_mut(&mut prior.plan);
        match mutation {
            0 => plan.ranges = selected_instructions::LiveRangeIdentity::from_bytes([7; 32]),
            1 => {
                plan.register_environment =
                    register_model::TargetRegisterEnvironmentIdentity::from_bytes([7; 32])
            }
            2 => {
                plan.allocator_availability =
                    register_homes::AllocatorAvailabilityIdentity::from_bytes([7; 32])
            }
            _ => unreachable!(),
        }
        legality::compute::REUSED_FUNCTIONS.set(0);
        assert_eq!(
            analyze(
                &ranges,
                &environment,
                &availability,
                Some((&ranges, &prior))
            )
            .unwrap(),
            legality
        );
        assert_eq!(legality::compute::REUSED_FUNCTIONS.get(), 0);
    }
}

#[test]
fn forged_prior_legality_is_rejected_even_with_a_refreshed_identity() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let availability = availability(&environment, target);
    let ranges = ranges(&source(target));
    let mut legality = analyze(&ranges, &environment, &availability, None).unwrap();
    Arc::make_mut(&mut legality.plan).functions[1].virtual_registers[0].points[0]
        .candidates
        .clear();
    legality.receipt.identity = register_homes::allocation_legality_identity(legality.plan());
    legality::compute::REUSED_FUNCTIONS.set(0);
    assert!(matches!(
        analyze(
            &ranges,
            &environment,
            &availability,
            Some((&ranges, &legality))
        ),
        Err(legality::AllocationLegalityError::VirtualRegisterMismatch { function: 1, .. })
    ));
    assert_eq!(legality::compute::REUSED_FUNCTIONS.get(), 2);
}
