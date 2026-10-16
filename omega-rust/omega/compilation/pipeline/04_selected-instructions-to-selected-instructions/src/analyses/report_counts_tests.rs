//! Receipt projection comparisons: retain one function and rebuild another.
//! These raw rows exercise reporting only, not semantic admission authority.

use register_homes::{
    AllocationLegalityPlan, AllocatorAvailabilityIdentity, FunctionAllocationLegality,
    VirtualLegalitySpan, VirtualRegisterAllocationLegality,
};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    FunctionLiveRanges, LiveRangeFragment, LiveRangeIdentity, LiveRangePlan, LiveRangePoint,
    LivenessIdentity, SelectedBlockId, SelectedInstructionPlanIdentity, VirtualLiveRange,
    VirtualRegisterId,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use std::sync::Arc;

#[test]
fn range_receipt_counts_match_fresh_projection_after_partial_carry() {
    let register = VirtualLiveRange {
        virtual_register: VirtualRegisterId(0),
        class: RegisterClassId(0),
        occurrences: Vec::new(),
        fixed_constraints: Vec::new(),
        fragments: vec![LiveRangeFragment {
            block: SelectedBlockId(0),
            start: LiveRangePoint(1),
            end: LiveRangePoint(4),
        }],
        edge_connectors: Vec::new(),
    };
    let function = |machine| {
        Arc::new(FunctionLiveRanges {
            machine: MachineId::new(machine).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: vec![register.clone()],
            tied_pairs: Vec::new(),
            edge_transfers: Vec::new(),
            copy_affinities: Vec::new(),
            early_clobbers: Vec::new(),
            architectural_units: Vec::new(),
            interference: Vec::new(),
        })
    };
    let mut plan = LiveRangePlan {
        selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"counts"),
        liveness: LivenessIdentity::from_bytes([1; 32]),
        optimization_unit: optimization_core::OptimizationUnitIdentity::from_bytes([2; 32]),
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target: target::NativeTarget::macos_arm64(),
        functions: vec![function(1), function(2)],
    };
    let previous = super::live_ranges::validate::admit(plan.clone(), &[], &[], &[]);
    Arc::make_mut(&mut plan.functions[1])
        .virtual_registers
        .push(VirtualLiveRange {
            virtual_register: VirtualRegisterId(1),
            ..register
        });
    let fresh = super::live_ranges::validate::admit(plan.clone(), &[], &[], &[]);
    assert_eq!(fresh.receipt().virtual_register_count(), 3);
    assert_eq!(fresh.receipt().virtual_fragment_count(), 3);
    for previous_counts in [&previous.function_counts[..], &[]] {
        let carried = super::live_ranges::validate::admit(
            plan.clone(),
            &[true, false],
            &previous.function_digests,
            previous_counts,
        );
        assert_eq!(carried, fresh);
    }
}

#[test]
fn legality_receipt_counts_match_fresh_projection_after_partial_carry() {
    let register = VirtualRegisterAllocationLegality {
        virtual_register: VirtualRegisterId(0),
        class: RegisterClassId(0),
        spans: vec![VirtualLegalitySpan {
            block: SelectedBlockId(0),
            start: LiveRangePoint(1),
            end: LiveRangePoint(4),
            candidates: vec![RegisterViewId(0), RegisterViewId(1)],
        }],
        early_clobber_points: Vec::new(),
        entry_transitions: Vec::new(),
    };
    let function = |machine| {
        Arc::new(FunctionAllocationLegality {
            machine: MachineId::new(machine).unwrap(),
            virtual_registers: vec![register.clone()],
        })
    };
    let mut plan = AllocationLegalityPlan {
        ranges: LiveRangeIdentity::from_bytes([1; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([2; 32]),
        allocator_availability: AllocatorAvailabilityIdentity::from_bytes([3; 32]),
        functions: vec![function(1), function(2)],
    };
    let previous =
        super::allocation_legality::validate::admit(plan.clone(), &[], &[], &[]).unwrap();
    let changed = &mut Arc::make_mut(&mut plan.functions[1]).virtual_registers[0];
    changed.spans[0].end = LiveRangePoint(8);
    changed.spans[0].candidates.push(RegisterViewId(2));
    let fresh = super::allocation_legality::validate::admit(plan.clone(), &[], &[], &[]).unwrap();
    assert_eq!(fresh.receipt().point_count(), 10);
    assert_eq!(fresh.receipt().candidate_count(), 27);
    for previous_counts in [&previous.function_counts[..], &[]] {
        let carried = super::allocation_legality::validate::admit(
            plan.clone(),
            &[true, false],
            &previous.function_digests,
            previous_counts,
        )
        .unwrap();
        assert_eq!(carried, fresh);
    }
}
