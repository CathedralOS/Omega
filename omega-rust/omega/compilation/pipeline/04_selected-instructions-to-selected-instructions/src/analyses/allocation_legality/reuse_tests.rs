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
