use crate::tests::*;

#[test]
fn logical_spill_operations_replay_the_active_resident_pressure_case_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let legality = staged_active_resident_two_view_legality(target);
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
        let first = selected_instructions_to_register_homes::plan_logical_spill_operations(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let second = selected_instructions_to_register_homes::plan_logical_spill_operations(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
            selected_lowering_budget(),
        )
        .unwrap();
        assert_eq!(first, second);
        let action = first.plan().functions[0].action.as_ref().unwrap();
        assert_eq!(action.block.0, 1);
        assert_eq!(action.pressure_point, LiveRangePoint(9));
        assert_eq!(action.incoming, VirtualRegisterId(3));
        assert_eq!(action.victim, VirtualRegisterId(1));
        assert_eq!(action.store.before_instruction, SelectedInstructionId(4));
        assert_eq!(action.store.source, action.victim);
        assert_eq!(action.store.storage, action.storage.id);
        assert_eq!(action.reload.before_instruction, SelectedInstructionId(6));
        assert_eq!(action.reload.storage, action.storage.id);
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
        assert_eq!(
            action.reload.before_instruction,
            action.rewrites[0].instruction
        );
        assert!(action.rewrites.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(
            action
                .rewrites
                .iter()
                .all(|rewrite| rewrite.result == action.reload.result)
        );
        assert_eq!(first.receipt().planned_function_count(), 1);
        assert_eq!(first.receipt().store_count(), 1);
        assert_eq!(first.receipt().reload_count(), 1);
        assert_eq!(first.receipt().rewritten_use_count(), 2);
        assert_eq!(
            selected_instructions_to_register_homes::LogicalSpillOperationPlan::decode(
                &first.plan().encode()
            )
            .unwrap(),
            *first.plan()
        );

        let mut corrupted = first.plan().clone();
        corrupted.functions[0].action.as_mut().unwrap().rewrites[0].operand += 1;
        assert_eq!(
            selected_instructions_to_register_homes::validate_logical_spill_operations(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                corrupted,
            ),
            Err(selected_instructions_to_register_homes::LogicalSpillOperationError::DecisionMismatch { function: 0 })
        );

        let exact_budget = OptimizationWorkBudget::new(1, 1, 3, 1, 1).unwrap();
        assert!(selected_instructions_to_register_homes::plan_logical_spill_operations(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
            exact_budget,
        )
        .is_ok());
        let insufficient = OptimizationWorkBudget::new(1, 1, 2, 1, 1).unwrap();
        assert!(matches!(
            selected_instructions_to_register_homes::plan_logical_spill_operations(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
                insufficient,
            ),
            Err(selected_instructions_to_register_homes::LogicalSpillOperationError::BudgetExceeded { .. })
        ));
    }
}

#[test]
fn logical_spill_v1_refuses_an_incoming_victim() {
    let target = NativeTarget::linux_x64();
    let ranges = stage_optimized_live_ranges(
        stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
    )
    .unwrap();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let sole_view = environment.physical().model().view_named("rdi").unwrap().id;
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
    let legality =
        stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap();
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
    assert_eq!(
        choices.plan().functions[0]
            .choice
            .as_ref()
            .unwrap()
            .selected_victim,
        choices.plan().functions[0]
            .choice
            .as_ref()
            .unwrap()
            .incoming
    );
    assert!(matches!(
        selected_instructions_to_register_homes::plan_logical_spill_operations(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
            selected_lowering_budget(),
        ),
        Err(selected_instructions_to_register_homes::LogicalSpillOperationError::UnsupportedVictimRole { .. })
    ));
}

#[test]
fn logical_spill_validation_rejects_root_decision_and_namespace_corruption() {
    let legality = staged_active_resident_two_view_legality(NativeTarget::linux_x64());
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
    let validated = selected_instructions_to_register_homes::plan_logical_spill_operations(
        selected.selected(),
        ranges.ranges(),
        legality.legality(),
        &choices,
        selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let baseline = validated.plan().clone();

    let mut root_variants = Vec::new();
    let mut changed = baseline.clone();
    changed.selected = selected_instructions::SelectedInstructionPlanIdentity::from_bytes([91; 32]);
    root_variants.push(changed);
    let mut changed = baseline.clone();
    changed.ranges =
        selected_instructions_to_register_homes::LiveRangeIdentity::from_bytes([92; 32]);
    root_variants.push(changed);
    let mut changed = baseline.clone();
    changed.legality =
        selected_instructions_to_register_homes::AllocationLegalityIdentity::from_bytes([93; 32]);
    root_variants.push(changed);
    let mut changed = baseline.clone();
    changed.spill_choices =
        selected_instructions_to_register_homes::SpillChoiceIdentity::from_bytes([94; 32]);
    root_variants.push(changed);
    let mut changed = baseline.clone();
    changed.register_environment =
        register_model::TargetRegisterEnvironmentIdentity::from_bytes([95; 32]);
    root_variants.push(changed);
    let mut changed = baseline.clone();
    changed.allocator_availability =
        selected_instructions_to_register_homes::AllocatorAvailabilityIdentity::from_bytes(
            [96; 32],
        );
    root_variants.push(changed);
    let mut changed = baseline.clone();
    changed.optimization_unit = optimization_core::OptimizationUnitIdentity::from_bytes([97; 32]);
    root_variants.push(changed);
    let mut changed = baseline.clone();
    changed.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(98).unwrap();
    root_variants.push(changed);
    for corrupted in root_variants {
        assert_eq!(
            selected_instructions_to_register_homes::validate_logical_spill_operations(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                corrupted,
            ),
            Err(selected_instructions_to_register_homes::LogicalSpillOperationError::RootMismatch)
        );
    }

    let mut decision_variants = Vec::new();
    let mut changed = baseline.clone();
    changed.functions[0].machine = MachineId::new(99).unwrap();
    decision_variants.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0]
        .action
        .as_mut()
        .unwrap()
        .pressure_point
        .0 += 1;
    decision_variants.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0].action.as_mut().unwrap().victim = VirtualRegisterId(2);
    decision_variants.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0]
        .action
        .as_mut()
        .unwrap()
        .victim_scalar_type = ScalarType::Boolean;
    decision_variants.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0].action.as_mut().unwrap().current_view.0 += 1;
    decision_variants.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0]
        .action
        .as_mut()
        .unwrap()
        .store
        .before_instruction
        .0 += 1;
    decision_variants.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0]
        .action
        .as_mut()
        .unwrap()
        .reload
        .before_instruction
        .0 += 1;
    decision_variants.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0]
        .action
        .as_mut()
        .unwrap()
        .rewrites
        .remove(0);
    decision_variants.push(changed);
    let mut changed = baseline.clone();
    changed.usage.validation_steps += 1;
    for corrupted in decision_variants {
        assert!(matches!(
            selected_instructions_to_register_homes::validate_logical_spill_operations(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                corrupted,
            ),
            Err(selected_instructions_to_register_homes::LogicalSpillOperationError::FunctionMismatch { .. })
                | Err(selected_instructions_to_register_homes::LogicalSpillOperationError::DecisionMismatch { .. })
        ));
    }
    assert_eq!(
        selected_instructions_to_register_homes::validate_logical_spill_operations(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            changed,
        ),
        Err(selected_instructions_to_register_homes::LogicalSpillOperationError::UsageMismatch)
    );

    let mut namespace = baseline;
    namespace.functions[0].action.as_mut().unwrap().storage.id.0 = 1;
    assert_eq!(
        selected_instructions_to_register_homes::validate_logical_spill_operations(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            namespace,
        ),
        Err(selected_instructions_to_register_homes::LogicalSpillOperationError::NonCanonicalStorageIds { function: 0 })
    );
}

#[test]
fn logical_spill_plan_is_canonical_when_pressure_is_absent() {
    let legality = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
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
    let plan = selected_instructions_to_register_homes::plan_logical_spill_operations(
        selected.selected(),
        ranges.ranges(),
        legality.legality(),
        &choices,
        selected_instructions_to_register_homes::LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1,
        selected_lowering_budget(),
    )
    .unwrap();
    assert!(
        plan.plan()
            .functions
            .iter()
            .all(|function| function.action.is_none())
    );
    assert_eq!(plan.receipt().planned_function_count(), 0);
    assert_eq!(plan.receipt().rewritten_use_count(), 0);
}
