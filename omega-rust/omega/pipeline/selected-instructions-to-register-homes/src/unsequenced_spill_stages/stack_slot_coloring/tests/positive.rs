use super::fixtures::{budget, source};
use crate::*;

#[test]
fn colors_the_validated_u64_logical_spill_relative_to_a_future_spill_area() {
    let source = source();
    let colored = color_logical_spill_stack_slots(
        &source,
        StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
        budget(),
    )
    .unwrap();
    let assignment = colored.plan().functions[0].assignments[0];
    assert_eq!(assignment.storage, LogicalSpillStorageId(0));
    assert_eq!(assignment.block, selected_instructions::SelectedBlockId(0));
    assert_eq!(assignment.live_from, LiveRangePoint(5));
    assert_eq!(assignment.live_through, LiveRangePoint(8));
    assert_eq!(assignment.size_bytes, 8);
    assert_eq!(assignment.alignment_bytes, 8);
    assert_eq!(assignment.spill_area_offset, 0);
    assert_eq!(colored.plan().functions[0].spill_area_bytes, 8);

    let receipt = colored.receipt();
    assert_eq!(
        receipt.logical_spill_operations(),
        source.receipt().identity()
    );
    assert_eq!(
        receipt.register_environment(),
        source.receipt().register_environment()
    );
    assert_eq!(
        receipt.allocator_availability(),
        source.receipt().allocator_availability()
    );
    assert_eq!(
        receipt.optimization_unit(),
        source.receipt().optimization_unit()
    );
    assert_eq!(receipt.fuel_schedule(), source.receipt().fuel_schedule());
    assert_eq!(receipt.budget(), budget());
    assert_eq!(receipt.function_count(), 1);
    assert_eq!(receipt.assignment_count(), 1);
    assert_eq!(receipt.distinct_slot_count(), 1);
    assert_eq!(receipt.reused_assignment_count(), 0);
    assert_eq!(receipt.max_function_spill_area_bytes(), 8);
    assert_eq!(
        receipt.identity(),
        stack_slot_coloring_identity(colored.plan())
    );
}

#[test]
fn no_logical_action_requires_no_spill_area() {
    let mut source = source();
    source.plan.functions[0].action = None;
    source = super::fixtures::validated_source(source.plan);
    let colored = color_logical_spill_stack_slots(
        &source,
        StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
        budget(),
    )
    .unwrap();
    assert!(colored.plan().functions[0].assignments.is_empty());
    assert_eq!(colored.plan().functions[0].spill_area_bytes, 0);
}

#[test]
fn exact_work_budget_is_sufficient() {
    let source = source();
    let exact = optimization_core::OptimizationWorkBudget::new(1, 1, 2, 1, 1).unwrap();
    assert!(
        color_logical_spill_stack_slots(
            &source,
            StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
            exact,
        )
        .is_ok()
    );
}
