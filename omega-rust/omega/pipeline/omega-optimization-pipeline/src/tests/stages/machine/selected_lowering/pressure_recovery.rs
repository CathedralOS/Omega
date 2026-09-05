use crate::tests::*;
use omega_regalloc::ValidatedSelectedAnalysis;

#[test]
fn explicit_one_view_availability_reaches_real_pressure_and_recovery_on_both_architectures() {
    for (target, sole_view_name) in [
        (NativeTarget::linux_x64(), "rdi"),
        (NativeTarget::linux_arm64(), "x0"),
    ] {
        let ranges = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
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
        let fixed_return = (target == NativeTarget::linux_x64())
            .then(|| environment.physical().model().view_named("rax").unwrap().id);
        assert!(matches!(
            materialize_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![sole_view, sole_view],
                },
            ),
            Err(AllocatorAvailabilityError::NonCanonicalAllowlist)
        ));
        assert!(matches!(
            materialize_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![RegisterViewId(u16::MAX)],
                },
            ),
            Err(AllocatorAvailabilityError::UnknownView { .. })
        ));
        let reserved_view = environment
            .physical()
            .model()
            .views
            .iter()
            .find(|view| {
                view.allocatable
                    && view
                        .units
                        .iter()
                        .chain(&view.write_units)
                        .any(|unit| environment.reservations().reserved_units().contains(unit))
            })
            .unwrap();
        assert!(matches!(
            materialize_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![reserved_view.id],
                },
            ),
            Err(AllocatorAvailabilityError::ViewNotEnvironmentAllocatable { .. })
        ));
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
        let mut noncanonical = availability.plan().clone();
        let retained_row = noncanonical
            .classes
            .iter_mut()
            .find(|row| !row.unconstrained_views.is_empty())
            .unwrap();
        retained_row.unconstrained_views.push(sole_view);
        assert_eq!(
            validate_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                noncanonical,
            ),
            Err(AllocatorAvailabilityError::NonCanonicalPlan)
        );
        let encoded = availability.plan().encode();
        assert_eq!(
            omega_regalloc::AllocatorAvailabilityPlan::decode(&encoded).unwrap(),
            *availability.plan()
        );
        let legality =
            stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap();
        if let Some(fixed_return) = fixed_return {
            assert_ne!(fixed_return, sole_view);
            assert!(
                legality
                    .legality()
                    .plan()
                    .functions
                    .iter()
                    .flat_map(|function| &function.virtual_registers)
                    .flat_map(|register| &register.points)
                    .any(|point| point.candidates == vec![fixed_return])
            );
        }
        assert_eq!(
            legality.custody().allocator_availability(),
            legality.allocator_availability().receipt().identity()
        );
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
            OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
        )
        .unwrap();
        let choice = choices.plan().functions[0].choice.as_ref().unwrap();
        assert_eq!(choice.incoming, VirtualRegisterId(2));
        assert_eq!(choice.selected_victim, choice.incoming);
        assert_eq!(choice.incoming_common_candidates, vec![sole_view]);

        let recovery = classify_pressure_recovery(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
        )
        .unwrap();
        let row = recovery.plan().functions[0]
            .classification
            .as_ref()
            .unwrap();
        assert_eq!(row.victim, VirtualRegisterId(2));
        assert_eq!(row.role, RecoveryVictimRole::Incoming);
        assert!(matches!(
            row.classification,
            RecoveryClassification::ImmediateU64RematerializationCandidate {
                value: IntegerValue::Unsigned(8),
                ..
            }
        ));
    }
}

#[test]
fn two_explicit_u12_exact_add_folds_close_one_view_pressure_on_both_architectures() {
    for (target, sole_view_name) in [
        (NativeTarget::linux_x64(), "rax"),
        (NativeTarget::linux_arm64(), "x0"),
    ] {
        let ranges = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
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
            budget(),
        )
        .unwrap();
        let recovery = classify_pressure_recovery(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            budget(),
        )
        .unwrap();
        let fold_one = fold_selected_incoming_literal(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            &recovery,
            legality.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            LiteralFoldPolicy::EXACT_ADD_V1,
            budget(),
        )
        .unwrap();
        assert_eq!(fold_one.receipt().applied_count(), 1);
        assert_eq!(
            LiteralFoldPlan::decode(&fold_one.plan().encode()).unwrap(),
            *fold_one.plan()
        );
        let mut corrupted_recipe = fold_one.plan().clone();
        corrupted_recipe.functions[0]
            .action
            .as_mut()
            .unwrap()
            .immediate += 1;
        assert!(matches!(
            validate_literal_fold(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                &recovery,
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted_recipe,
            ),
            Err(omega_regalloc::LiteralFoldError::DecisionMismatch { .. })
        ));
        let foreign_target = match target.architecture {
            omega_target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            omega_target::Architecture::Aarch64 => NativeTarget::linux_x64(),
        };
        let foreign_environment = baseline_target_register_environment(foreign_target).unwrap();
        assert_eq!(
            validate_literal_fold(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                &recovery,
                legality.allocator_availability(),
                environment.identity(),
                foreign_environment.physical(),
                foreign_environment.constraints(),
                foreign_environment.reservations(),
                environment.allocation_constraint_keys(),
                fold_one.plan().clone(),
            ),
            Err(omega_regalloc::LiteralFoldError::RootMismatch)
        );
        let folded_add = &fold_one.transformed().functions[0].blocks[1].instructions[1];
        assert!(matches!(
            folded_add.kind,
            SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(8),
                ..
            }
        ));
        assert_eq!(folded_add.provenance.operations.len(), 2);
        assert_eq!(folded_add.provenance.obligations.len(), 1);

        let liveness_one = analyze_liveness(&fold_one).unwrap();
        let ranges_one = analyze_live_ranges(&fold_one, &liveness_one).unwrap();
        let legality_one = analyze_allocation_legality(
            &ranges_one,
            legality.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
        )
        .unwrap();
        let choices_one = choose_spill_victims(
            &legality_one,
            &ranges_one,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            budget(),
        )
        .unwrap();
        assert_eq!(
            choices_one.plan().functions[0]
                .choice
                .as_ref()
                .unwrap()
                .incoming,
            VirtualRegisterId(4)
        );
        let recovery_one = classify_pressure_recovery(
            &fold_one,
            &ranges_one,
            &legality_one,
            &choices_one,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            budget(),
        )
        .unwrap();
        let fold_two = fold_selected_incoming_literal(
            &fold_one,
            &ranges_one,
            &legality_one,
            &choices_one,
            &recovery_one,
            legality.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            LiteralFoldPolicy::EXACT_ADD_V1,
            budget(),
        )
        .unwrap();
        assert_eq!(fold_two.receipt().applied_count(), 1);

        let liveness_two = analyze_liveness(&fold_two).unwrap();
        let ranges_two = analyze_live_ranges(&fold_two, &liveness_two).unwrap();
        let legality_two = analyze_allocation_legality(
            &ranges_two,
            legality.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
        )
        .unwrap();
        let choices_two = choose_spill_victims(
            &legality_two,
            &ranges_two,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            budget(),
        )
        .unwrap();
        assert!(
            choices_two
                .plan()
                .functions
                .iter()
                .all(|function| function.choice.is_none())
        );
        let homes = omega_regalloc::assign_register_homes(
            &legality_two,
            &ranges_two,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
        )
        .unwrap();
        assert_eq!(
            homes.plan().functions[0].assignments.len(),
            fold_two.transformed().functions[0].virtual_registers.len()
        );

        let staged_folds = stage_first_optimized_literal_fold(
            legality,
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            LiteralFoldPolicy::EXACT_ADD_V1,
            budget(),
        )
        .unwrap();
        assert_eq!(staged_folds.steps().len(), 1);
        let staged_environment = staged_folds
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        assert!(matches!(
            omega_regalloc::assign_register_homes(
                staged_folds.final_step().legality(),
                staged_folds.final_step().ranges(),
                staged_environment.identity(),
                staged_environment.physical(),
                staged_environment.constraints(),
                staged_environment.reservations(),
                staged_environment.allocation_constraint_keys(),
            ),
            Err(RegisterHomeError::NoCompatibleHome { .. })
        ));
        let staged_folds = stage_next_optimized_literal_fold(
            staged_folds,
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            LiteralFoldPolicy::EXACT_ADD_V1,
            budget(),
        )
        .unwrap();
        assert_eq!(staged_folds.steps().len(), 2);
        assert_eq!(staged_folds.custody().transformations().len(), 2);
        let [first_iteration, second_iteration] = staged_folds.custody().iterations() else {
            panic!("two explicit fold calls must retain two iteration receipts");
        };
        assert_eq!(
            first_iteration.transformed_selected(),
            second_iteration.source_selected()
        );
        assert_eq!(
            first_iteration.fresh_ranges(),
            second_iteration.source_ranges()
        );
        assert_eq!(
            first_iteration.fresh_legality(),
            second_iteration.source_legality()
        );
        assert_eq!(
            second_iteration.fold_policy(),
            LiteralFoldPolicy::EXACT_ADD_V1
        );
        assert_eq!(
            validate_optimized_literal_fold_custody(&staged_folds).unwrap(),
            *staged_folds.custody()
        );
        assert_eq!(
            staged_folds.custody().final_selected(),
            staged_folds
                .final_step()
                .fold()
                .receipt()
                .transformed_selected()
        );
        let machine_effects = analyze_machine_effects(
            staged_folds.final_step().fold(),
            staged_folds
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
        )
        .unwrap();
        assert_eq!(
            machine_effects.receipt().selected(),
            staged_folds.custody().final_selected()
        );
        assert_eq!(
            machine_effects.receipt().selected(),
            staged_folds.custody().final_selected()
        );
        validate_machine_effects(
            staged_folds.final_step().fold(),
            staged_folds
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment(),
            &machine_effects,
        )
        .unwrap();
        let expected_transformations = staged_folds
            .custody()
            .transformations()
            .iter()
            .copied()
            .map(PostAllocationSelectedTransformation::LiteralFold)
            .collect::<Vec<_>>();
        let staged_homes =
            stage_optimized_register_homes_after_literal_folds(staged_folds).unwrap();
        let post = stage_optimized_post_allocation_machine_plan(&staged_homes).unwrap();
        assert_eq!(
            post.machine().receipt().selected(),
            staged_homes.fold_stage().custody().final_selected()
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_custody(&staged_homes, &post,)
                .unwrap(),
            post.custody()
        );
        assert_eq!(
            staged_homes
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            expected_transformations
        );
        assert_eq!(
            staged_homes.post_allocation_manifest().record().selected,
            staged_homes.fold_stage().custody().final_selected()
        );
        assert_eq!(
            validate_optimized_register_home_after_literal_fold_custody(&staged_homes).unwrap(),
            *staged_homes.custody()
        );
        let original = staged_homes.replay_allocation().unwrap();
        let selected_owner = original.selected().shared_selected_plan();
        let home_owner = original.homes().shared_plan();
        let retained = omega_selected_instructions_to_register_homes::RetainedAllocation::try_from(
            staged_homes,
        )
        .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &selected_owner,
            &retained.program().selected
        ));
        assert!(std::sync::Arc::ptr_eq(
            &home_owner,
            &retained.program().homes
        ));
        assert_eq!(
            retained.current().selected_plan(),
            retained.replay_allocation().unwrap().selected_plan()
        );
        assert_eq!(
            stage_optimized_post_allocation_machine_plan(&retained).unwrap(),
            post
        );
    }
}
