//! Abstract spill-area store/reload scheduling before reload-home and frame assignment.

use crate::tests::*;

#[test]
fn exact_schedule_is_deterministic_on_both_architectures() {
    let mut frame_neutral_shape = None;
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (logical, slots) = spill_sources(target);
        let budget = OptimizationWorkBudget::new(1, 1, 5, 1, 1).unwrap();
        let first = selected_instructions_to_register_homes::schedule_abstract_spill_insertion(
            &logical,
            &slots,
            selected_instructions_to_register_homes::AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1,
            budget,
        )
        .unwrap();
        let second = selected_instructions_to_register_homes::schedule_abstract_spill_insertion(
            &logical,
            &slots,
            selected_instructions_to_register_homes::AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1,
            budget,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.plan().usage.rule_evaluations, 1);
        assert_eq!(first.plan().usage.candidates, 1);
        assert_eq!(first.plan().usage.validation_steps, 5);
        assert_eq!(first.plan().usage.commits, 1);
        assert_eq!(first.plan().usage.iterations, 1);
        assert_eq!(
            first.receipt().logical_spill_operations(),
            logical.receipt().identity()
        );
        assert_eq!(
            first.receipt().stack_slot_coloring(),
            slots.receipt().identity()
        );
        assert_eq!(first.receipt().action_count(), 1);
        assert_eq!(first.receipt().access_count(), 2);
        assert_eq!(first.receipt().rewritten_use_count(), 2);
        assert_eq!(first.receipt().max_spill_area_bytes(), 8);

        let function = &first.plan().functions[0];
        assert_eq!(function.spill_area_bytes, 8);
        let action = function.action.as_ref().unwrap();
        assert_eq!(action.pressure_point, LiveRangePoint(9));
        assert_eq!(action.store.before_instruction, SelectedInstructionId(4));
        assert_eq!(action.store.source, action.victim);
        assert_eq!(action.store.source_view, action.victim_view);
        assert_eq!(action.reload.before_instruction, SelectedInstructionId(6));
        assert_eq!(
            action.reload.before_instruction,
            action.rewrites[0].instruction
        );
        assert_eq!(action.slot.storage, action.store.slot);
        assert_eq!(action.slot.storage, action.reload.slot);
        assert_eq!(action.slot.size_bytes, 8);
        assert_eq!(action.slot.alignment_bytes, 8);
        assert_eq!(action.slot.spill_area_offset, 0);
        assert_eq!(
            action
                .rewrites
                .iter()
                .map(|rewrite| (rewrite.point, rewrite.instruction, rewrite.operand))
                .collect::<Vec<_>>(),
            vec![
                (LiveRangePoint(12), SelectedInstructionId(6), 0),
                (LiveRangePoint(14), SelectedInstructionId(7), 0),
            ]
        );
        assert!(action.rewrites.windows(2).all(|pair| pair[0] < pair[1]));

        let shape = (
            action.slot,
            action.store.before_instruction,
            action.reload.before_instruction,
            action.rewrites.clone(),
        );
        if let Some(expected) = &frame_neutral_shape {
            assert_eq!(&shape, expected);
        } else {
            frame_neutral_shape = Some(shape);
        }
    }
}

#[test]
fn independent_replay_rejects_root_schedule_slot_rewrite_and_usage_corruption() {
    let (logical, slots) = spill_sources(NativeTarget::linux_x64());
    let staged = selected_instructions_to_register_homes::schedule_abstract_spill_insertion(
        &logical,
        &slots,
        selected_instructions_to_register_homes::AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let canonical = staged.plan().clone();

    let mut root = canonical.clone();
    root.stack_slot_coloring =
        selected_instructions_to_register_homes::StackSlotColoringIdentity::from_bytes([0xd1; 32]);
    assert_eq!(
        selected_instructions_to_register_homes::validate_abstract_spill_insertion(
            &logical, &slots, root
        ),
        Err(selected_instructions_to_register_homes::AbstractSpillInsertionError::RootMismatch)
    );

    for corrupt in [
        |plan: &mut selected_instructions_to_register_homes::AbstractSpillInsertionPlan| {
            plan.functions[0]
                .action
                .as_mut()
                .unwrap()
                .store
                .source_view
                .0 += 1;
        },
        |plan: &mut selected_instructions_to_register_homes::AbstractSpillInsertionPlan| {
            plan.functions[0]
                .action
                .as_mut()
                .unwrap()
                .reload
                .destination_class
                .0 += 1;
        },
        |plan: &mut selected_instructions_to_register_homes::AbstractSpillInsertionPlan| {
            plan.functions[0]
                .action
                .as_mut()
                .unwrap()
                .slot
                .spill_area_offset += 8;
        },
        |plan: &mut selected_instructions_to_register_homes::AbstractSpillInsertionPlan| {
            plan.functions[0]
                .action
                .as_mut()
                .unwrap()
                .rewrites
                .reverse();
        },
    ] {
        let mut changed = canonical.clone();
        corrupt(&mut changed);
        assert_eq!(
            selected_instructions_to_register_homes::validate_abstract_spill_insertion(&logical, &slots, changed),
            Err(selected_instructions_to_register_homes::AbstractSpillInsertionError::NonCanonicalSchedule { function: 0 })
        );
    }

    let mut usage = canonical.clone();
    usage.usage.validation_steps += 1;
    assert_eq!(
        selected_instructions_to_register_homes::validate_abstract_spill_insertion(
            &logical, &slots, usage
        ),
        Err(selected_instructions_to_register_homes::AbstractSpillInsertionError::UsageMismatch)
    );

    assert!(matches!(
        selected_instructions_to_register_homes::schedule_abstract_spill_insertion(
            &logical,
            &slots,
            selected_instructions_to_register_homes::AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1,
            OptimizationWorkBudget::new(1, 1, 4, 1, 1).unwrap(),
        ),
        Err(selected_instructions_to_register_homes::AbstractSpillInsertionError::BudgetExceeded { .. })
    ));
}

#[test]
fn empty_pressure_retains_zero_abstract_spill_area_without_insertions() {
    let legality = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let logical = logical_from_legality(&legality);
    let slots = selected_instructions_to_register_homes::color_logical_spill_stack_slots(
        &logical,
        selected_instructions_to_register_homes::StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let staged = selected_instructions_to_register_homes::schedule_abstract_spill_insertion(
        &logical,
        &slots,
        selected_instructions_to_register_homes::AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1,
        selected_lowering_budget(),
    )
    .unwrap();
    assert!(
        staged
            .plan()
            .functions
            .iter()
            .all(|function| function.spill_area_bytes == 0 && function.action.is_none())
    );
    assert_eq!(staged.receipt().action_count(), 0);
    assert_eq!(staged.receipt().access_count(), 0);
    assert_eq!(staged.receipt().max_spill_area_bytes(), 0);
}

fn spill_sources(
    target: NativeTarget,
) -> (
    selected_instructions_to_register_homes::ValidatedLogicalSpillOperations,
    selected_instructions_to_register_homes::ValidatedStackSlotColoring,
) {
    let logical = logical_from_legality(&staged_active_resident_two_view_legality(target));
    let slots = selected_instructions_to_register_homes::color_logical_spill_stack_slots(
        &logical,
        selected_instructions_to_register_homes::StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
        selected_lowering_budget(),
    )
    .unwrap();
    (logical, slots)
}

fn logical_from_legality(
    legality: &StagedOptimizedAllocationLegality,
) -> selected_instructions_to_register_homes::ValidatedLogicalSpillOperations {
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
        &environment.allocation_constraint_keys(),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        selected_lowering_budget(),
    )
    .unwrap();
    selected_instructions_to_register_homes::plan_logical_spill_operations(
        selected.selected(),
        ranges.ranges(),
        legality.legality(),
        &choices,
        selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
        selected_lowering_budget(),
    )
    .unwrap()
}
