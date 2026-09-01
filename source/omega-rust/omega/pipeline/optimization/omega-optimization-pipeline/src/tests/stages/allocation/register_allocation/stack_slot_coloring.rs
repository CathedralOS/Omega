//! Target-neutral spill-area slot coloring after validated logical spilling.

use omega_regalloc::{LogicalSpillStorageClass, LogicalSpillStorageId};

use crate::tests::*;

#[test]
fn stack_slot_coloring_is_deterministic_and_target_neutral() {
    let mut canonical = None;
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let logical = active_resident_logical_spill(target);
        let first = omega_regalloc::color_logical_spill_stack_slots(
            &logical,
            omega_regalloc::StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let second = omega_regalloc::color_logical_spill_stack_slots(
            &logical,
            omega_regalloc::StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
            selected_lowering_budget(),
        )
        .unwrap();
        assert_eq!(first, second);
        let function = &first.plan().functions[0];
        let assignment = function.assignments.first().unwrap();
        assert_eq!(assignment.storage, LogicalSpillStorageId(0));
        assert_eq!(
            assignment.class,
            LogicalSpillStorageClass::NonAddressUnsignedU64V1
        );
        assert_eq!(assignment.live_from, LiveRangePoint(9));
        assert_eq!(assignment.live_through, LiveRangePoint(12));
        assert_eq!(assignment.size_bytes, 8);
        assert_eq!(assignment.alignment_bytes, 8);
        assert_eq!(assignment.spill_area_offset, 0);
        assert_eq!(function.spill_area_bytes, 8);
        assert_eq!(first.receipt().assignment_count(), 1);
        assert_eq!(first.receipt().distinct_slot_count(), 1);
        assert_eq!(first.receipt().reused_assignment_count(), 0);
        assert_eq!(first.receipt().max_function_spill_area_bytes(), 8);
        assert_eq!(
            first.receipt().logical_spill_operations(),
            logical.receipt().identity()
        );
        assert_eq!(
            omega_regalloc::StackSlotColoringPlan::decode(&first.plan().encode()),
            Ok(first.plan().clone())
        );
        assert_eq!(
            omega_regalloc::validate_stack_slot_coloring(&logical, first.plan().clone(),).unwrap(),
            first
        );

        let target_neutral_shape = (
            assignment.block,
            assignment.live_from,
            assignment.live_through,
            assignment.size_bytes,
            assignment.alignment_bytes,
            assignment.spill_area_offset,
            function.spill_area_bytes,
        );
        if let Some(expected) = canonical {
            assert_eq!(target_neutral_shape, expected);
        } else {
            canonical = Some(target_neutral_shape);
        }
    }
}

#[test]
fn stack_slot_coloring_rejects_root_assignment_extent_and_usage_corruption() {
    let logical = active_resident_logical_spill(NativeTarget::linux_x64());
    let validated = omega_regalloc::color_logical_spill_stack_slots(
        &logical,
        omega_regalloc::StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
        selected_lowering_budget(),
    )
    .unwrap();

    let mut root = validated.plan().clone();
    root.logical_spill_operations =
        omega_regalloc::LogicalSpillOperationIdentity::from_bytes([211; 32]);
    assert_eq!(
        omega_regalloc::validate_stack_slot_coloring(&logical, root),
        Err(omega_regalloc::StackSlotColoringError::RootMismatch)
    );

    let mut assignment = validated.plan().clone();
    assignment.functions[0].assignments[0].spill_area_offset = 8;
    assert_eq!(
        omega_regalloc::validate_stack_slot_coloring(&logical, assignment),
        Err(omega_regalloc::StackSlotColoringError::NonCanonicalAssignments { function: 0 })
    );

    let mut extent = validated.plan().clone();
    extent.functions[0].spill_area_bytes = 16;
    assert_eq!(
        omega_regalloc::validate_stack_slot_coloring(&logical, extent),
        Err(omega_regalloc::StackSlotColoringError::NonCanonicalAssignments { function: 0 })
    );

    let mut usage = validated.plan().clone();
    usage.usage.validation_steps += 1;
    assert_eq!(
        omega_regalloc::validate_stack_slot_coloring(&logical, usage),
        Err(omega_regalloc::StackSlotColoringError::UsageMismatch)
    );
}

#[test]
fn stack_slot_coloring_preserves_an_empty_pressure_plan_without_allocating_a_slot() {
    let legality = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let logical = logical_spill_from_legality(&legality);
    let colored = omega_regalloc::color_logical_spill_stack_slots(
        &logical,
        omega_regalloc::StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
        selected_lowering_budget(),
    )
    .unwrap();
    assert!(colored
        .plan()
        .functions
        .iter()
        .all(|function| function.assignments.is_empty() && function.spill_area_bytes == 0));
    assert_eq!(colored.receipt().assignment_count(), 0);
    assert_eq!(colored.receipt().distinct_slot_count(), 0);
    assert_eq!(colored.receipt().max_function_spill_area_bytes(), 0);
}

fn active_resident_logical_spill(
    target: NativeTarget,
) -> omega_regalloc::ValidatedLogicalSpillOperations {
    let legality = staged_active_resident_two_view_legality(target);
    logical_spill_from_legality(&legality)
}

fn logical_spill_from_legality(
    legality: &StagedOptimizedAllocationLegality,
) -> omega_regalloc::ValidatedLogicalSpillOperations {
    let ranges = legality.live_range_stage();
    let selected = ranges.liveness_stage().selected_stage();
    let environment = selected.register_environment();
    let choices = choose_spill_victims(
        legality.legality(),
        ranges.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        selected_lowering_budget(),
    )
    .unwrap();
    omega_regalloc::plan_logical_spill_operations(
        selected.selected(),
        ranges.ranges(),
        legality.legality(),
        &choices,
        omega_regalloc::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
        selected_lowering_budget(),
    )
    .unwrap()
}
