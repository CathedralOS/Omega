use crate::tests::*;

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
            LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
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
            LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
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
            LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
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
            LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
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
            LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1
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
        let machine_effects =
            stage_optimized_machine_effects_after_literal_folds(&staged_folds).unwrap();
        assert_eq!(
            machine_effects.effects().receipt().selected(),
            staged_folds.custody().final_selected()
        );
        assert_eq!(
            machine_effects.custody().source(),
            &StagedOptimizedMachineEffectSourceCustodyReceipt::LiteralFolds(
                staged_folds.custody().clone()
            )
        );
        assert_eq!(
            &validate_optimized_machine_effect_custody_after_literal_folds(
                &staged_folds,
                machine_effects.effects(),
            )
            .unwrap(),
            machine_effects.custody()
        );
        let expected_transformations = staged_folds
            .custody()
            .transformations()
            .iter()
            .copied()
            .map(PostAllocationSelectedTransformation::LiteralFold)
            .collect::<Vec<_>>();
        let staged_homes =
            stage_optimized_register_homes_after_literal_folds(staged_folds).unwrap();
        let post = stage_optimized_post_allocation_machine_plan_after_literal_folds(&staged_homes)
            .unwrap();
        assert_eq!(
            post.machine().receipt().selected(),
            staged_homes.fold_stage().custody().final_selected()
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_after_literal_fold_custody(
                &staged_homes,
                &post,
            )
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
    }
}

#[test]
fn named_selected_lowering_suite_reaches_a_verified_fixed_point_on_both_architectures() {
    for (target, sole_view_name) in [
        (NativeTarget::linux_x64(), "rax"),
        (NativeTarget::linux_arm64(), "x0"),
    ] {
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let ranges = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                target,
                selections.clone(),
                selected_lowering_budget(),
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
        let legality =
            stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap();
        let run = run_selected_lowering_optimizations(legality).unwrap();

        assert_eq!(run.selections(), &selections);
        assert_eq!(run.custody().selections(), selections.identity());
        assert_eq!(
            run.selected_lowering_selections().as_slice(),
            &[Optimization::SelectedIncomingU12ExactAddImmediate]
        );
        assert_eq!(
            run.custody().selected_lowering_selections(),
            run.selected_lowering_selections().identity()
        );
        assert_eq!(run.steps().len(), 2);
        assert_eq!(run.custody().action_count(), 2);
        assert_eq!(
            run.custody()
                .initial_virtual_register_count()
                .checked_sub(run.custody().action_count()),
            Some(run.custody().final_virtual_register_count())
        );
        assert_eq!(run.custody().iterations().len(), 2);
        assert_eq!(run.attempt().fold().receipt().applied_count(), 0);
        assert_eq!(
            run.attempt().fold().receipt().source_selected(),
            run.attempt().fold().receipt().transformed_selected()
        );
        assert!(run.custody().usage().within(run.custody().budget()));
        assert_eq!(
            validate_selected_lowering_optimization_custody(&run).unwrap(),
            *run.custody()
        );
        let completion = run.custody().identity();
        let final_selected = run.custody().final_selected();
        let expected_transformations = run
            .custody()
            .iterations()
            .iter()
            .map(|iteration| PostAllocationSelectedTransformation::LiteralFold(iteration.fold()))
            .collect::<Vec<_>>();
        let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
        assert_eq!(
            homes
                .post_allocation_manifest()
                .record()
                .selected_lowering_completion,
            Some(completion)
        );
        assert_eq!(
            homes
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            expected_transformations
        );
        assert_eq!(
            validate_optimized_register_home_after_selected_lowering_custody(&homes).unwrap(),
            *homes.custody()
        );
        let realization = stage_selected_lowering_function_relative_realization(homes).unwrap();
        let post = realization.machine();
        assert_eq!(post.machine().receipt().selected(), final_selected);
        assert_eq!(
            validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
                realization.homes(),
                post,
            )
            .unwrap(),
            *post.custody()
        );
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(&realization).unwrap(),
            *realization.custody()
        );
        let manifest = realization.manifest().record();
        assert_eq!(manifest.selections, selections.identity());
        assert_eq!(
            manifest.selected_lowering_selections,
            selections
                .for_phase(omega_optimization_core::OptimizationExecutionPhase::SelectedLowering,)
                .identity()
        );
        assert_eq!(manifest.selected_lowering_completion, Some(completion));
        assert_eq!(
            manifest.function_relative_layout_selections,
            selections
                .for_phase(
                    omega_optimization_core::OptimizationExecutionPhase::FunctionRelativeLayout,
                )
                .identity()
        );
        assert_eq!(manifest.selected, final_selected);
        assert_eq!(manifest.pre_layout, realization.encoding().identity());
        assert_eq!(
            manifest.baseline_resolved_layout,
            realization.layout().identity()
        );
        assert_eq!(manifest.resolved_layout, realization.layout().identity());
        assert_eq!(manifest.x86_branch_relaxation, None);
        assert_eq!(
            manifest.whole_function_exit_contract,
            realization.exit_contract().identity()
        );
        assert_eq!(
            realization.exit_contract().contract().functions[0]
                .returns
                .len(),
            2
        );
        assert_eq!(manifest.statistics.functions, 1);
        assert_eq!(manifest.statistics.blocks, 3);
        assert_eq!(manifest.statistics.resolved_conditional_branches, 1);
        assert_eq!(manifest.statistics.structural_unit_functions, 0);
        assert_eq!(manifest.statistics.structural_unit_blocks, 0);
        assert_eq!(manifest.statistics.structural_unit_instructions, 0);
        assert_eq!(manifest.statistics.structural_unit_bytes, 0);
        assert_eq!(manifest.statistics.unresolved_internal_machine_fixups, 0);
        assert_eq!(
            manifest.statistics.bytes,
            realization
                .layout()
                .functions()
                .iter()
                .map(|function| function.byte_count)
                .sum()
        );
    }
}

#[test]
fn named_exact_subtract_immediate_suite_closes_pressure_and_rejects_policy_substitution() {
    for (target, sole_view_name) in [
        (NativeTarget::linux_x64(), "rax"),
        (NativeTarget::linux_arm64(), "x0"),
    ] {
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
        ])
        .unwrap();
        let ranges = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_subtract_conditional_with_selections(
                target,
                selections.clone(),
                selected_lowering_budget(),
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
        let legality =
            stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap();
        let run = run_selected_lowering_optimizations(legality).unwrap();

        assert_eq!(
            run.selected_lowering_selections().as_slice(),
            &[Optimization::SelectedIncomingU12ExactSubtractImmediate]
        );
        assert_eq!(
            run.custody().schedule(),
            SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactSubtractImmediateToNoChangeV1
        );
        assert_eq!(run.steps().len(), 2);
        assert_eq!(run.custody().action_count(), 2);
        assert_eq!(run.attempt().fold().receipt().applied_count(), 0);
        assert_eq!(
            validate_selected_lowering_optimization_custody(&run).unwrap(),
            *run.custody()
        );

        let final_plan = run.attempt().fold().transformed();
        let folded = final_plan.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.kind {
                SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
                    Some((immediate, &instruction.provenance))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            folded
                .iter()
                .map(|(immediate, _)| *immediate)
                .collect::<Vec<_>>(),
            vec![IntegerValue::Unsigned(5), IntegerValue::Unsigned(8)]
        );
        for (_, provenance) in folded {
            assert_eq!(provenance.operations.len(), 2);
            assert_eq!(provenance.fuel.len(), 2);
            assert_eq!(provenance.obligations.len(), 1);
        }

        let source = run.source_legality_stage();
        let selected = source.live_range_stage().liveness_stage().selected_stage();
        let environment = selected.register_environment();
        let first = &run.steps()[0];
        let mut substituted_policy = first.fold().plan().clone();
        substituted_policy.policy = LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1;
        assert!(
            validate_literal_fold(
                selected.selected(),
                source.live_range_stage().ranges(),
                source.legality(),
                first.choices(),
                first.recovery(),
                source.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                substituted_policy,
            )
            .is_err()
        );

        let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
        assert_eq!(
            validate_optimized_register_home_after_selected_lowering_custody(&homes).unwrap(),
            *homes.custody()
        );
    }
}

#[test]
fn combined_exact_immediate_selection_executes_each_named_shape() {
    for subtract in [false, true] {
        let target = NativeTarget::linux_x64();
        let selections = OptimizationSelections::new([
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
        ])
        .unwrap();
        let selected = if subtract {
            staged_exact_subtract_conditional_with_selections(
                target,
                selections.clone(),
                selected_lowering_budget(),
            )
        } else {
            staged_exact_add_conditional_with_selections(
                target,
                selections.clone(),
                selected_lowering_budget(),
            )
        };
        let ranges =
            stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
        let environment = ranges
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let sole_view = environment.physical().model().view_named("rax").unwrap().id;
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
        let run = run_selected_lowering_optimizations(
            stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap(),
        )
        .unwrap();

        assert_eq!(
            run.custody().schedule(),
            SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactAddAndSubtractImmediateToNoChangeV1
        );
        assert_eq!(run.custody().action_count(), 2);
        let final_plan = run.attempt().fold().transformed();
        let matching_immediate_count = final_plan.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                if subtract {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::ExactSubtractI64Immediate { .. }
                    )
                } else {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::ExactAddI64Immediate { .. }
                    )
                }
            })
            .count();
        assert_eq!(matching_immediate_count, 2);
        assert_eq!(
            validate_selected_lowering_optimization_custody(&run).unwrap(),
            *run.custody()
        );
    }
}

#[test]
fn named_selected_lowering_suite_retains_verified_no_change_completion() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let legality = stage_optimized_allocation_legality_for_frameless_leaf(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                    target,
                    selections.clone(),
                    selected_lowering_budget(),
                ))
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let source_selected = legality
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .selected()
            .receipt()
            .identity();
        let source_ranges = legality.live_range_stage().ranges().receipt().identity();
        let source_legality = legality.legality().receipt().identity();
        let run = run_selected_lowering_optimizations(legality).unwrap();

        assert!(run.steps().is_empty());
        assert_eq!(run.custody().action_count(), 0);
        assert_eq!(run.custody().final_selected(), source_selected);
        assert_eq!(run.custody().final_ranges(), source_ranges);
        assert_eq!(run.custody().final_legality(), source_legality);
        assert_eq!(run.attempt().fold().receipt().applied_count(), 0);
        assert_eq!(
            validate_selected_lowering_optimization_custody(&run).unwrap(),
            *run.custody()
        );
        let completion = run.custody().identity();
        let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
        let manifest = homes.post_allocation_manifest().record();
        assert_eq!(manifest.selected_lowering_completion, Some(completion));
        assert!(manifest.selected_transformations.is_empty());
        assert_eq!(manifest.selected, source_selected);
        let realization = stage_selected_lowering_function_relative_realization(homes).unwrap();
        let post = realization.machine();
        assert_eq!(post.machine().receipt().selected(), source_selected);
        assert_eq!(
            validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
                realization.homes(),
                post,
            )
            .unwrap(),
            *post.custody()
        );
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(&realization).unwrap(),
            *realization.custody()
        );
        let manifest = realization.manifest().record();
        assert_eq!(manifest.selected_lowering_completion, Some(completion));
        assert_eq!(manifest.selected, source_selected);
        assert_eq!(
            manifest.whole_function_exit_contract,
            realization.exit_contract().identity()
        );
        assert_eq!(realization.exit_contract().contract().functions.len(), 1);
        assert_eq!(
            realization.exit_contract().contract().functions[0]
                .returns
                .len(),
            2
        );
        assert!(
            realization.exit_contract().contract().functions[0]
                .modified_callee_saved_units
                .is_empty()
        );
        let exit = realization.exit_contract().contract();
        assert_eq!(
            exit.hardening,
            WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1
        );
        match (target.architecture, target.object_format) {
            (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Elf) => {
                assert_eq!(
                    exit.policy,
                    WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1
                );
                assert_eq!(
                    exit.entry_assumption,
                    WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
                );
                assert!(exit.functions[0].returns.iter().all(|returned| matches!(
                    returned.mechanism,
                    WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                        read_bytes: 8,
                        pop_bytes: 8,
                        ..
                    }
                )));
            }
            (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Coff) => {
                assert_eq!(
                    exit.policy,
                    WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1
                );
                assert_eq!(
                    exit.entry_assumption,
                    WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
                );
            }
            (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::Elf) => {
                assert_eq!(exit.policy, WholeFunctionExitPolicy::Aapcs64FramelessLeafV1);
                assert!(matches!(
                    exit.entry_assumption,
                    WholeFunctionEntryAssumption::CallerLinkRegisterV1 { .. }
                ));
                assert!(exit.functions[0].returns.iter().all(|returned| matches!(
                    returned.mechanism,
                    WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 { .. }
                )));
            }
            (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::MachO) => {
                assert_eq!(
                    exit.policy,
                    WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1
                );
                assert!(matches!(
                    exit.entry_assumption,
                    WholeFunctionEntryAssumption::CallerLinkRegisterV1 { .. }
                ));
            }
            _ => unreachable!(),
        }
        assert_eq!(manifest.statistics.functions, 1);
        assert_eq!(manifest.statistics.blocks, 3);
        assert_eq!(manifest.statistics.resolved_conditional_branches, 1);
        assert_eq!(
            manifest.publication,
            FunctionRelativeOptimizationUnavailableData::Unavailable
        );
        let encoded = manifest.encode();
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&encoded),
            Ok(manifest.clone())
        );
        let mut without_selected_lowering = manifest.clone();
        without_selected_lowering.selected_lowering_completion = None;
        without_selected_lowering.identity = without_selected_lowering.recomputed_identity();
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(
                &without_selected_lowering.encode()
            ),
            Ok(without_selected_lowering)
        );
        if target.architecture == omega_target::Architecture::X86_64 {
            let mut with_no_change_relaxation = manifest.clone();
            with_no_change_relaxation.x86_branch_relaxation =
                Some(X86BranchRelaxationIdentity::from_bytes([0x4f; 32]));
            with_no_change_relaxation.identity = with_no_change_relaxation.recomputed_identity();
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(
                    &with_no_change_relaxation.encode()
                ),
                Ok(with_no_change_relaxation)
            );
        }
        assert!(manifest.render_text().contains("publication: unavailable"));
        let mut identity_tamper = encoded.clone();
        identity_tamper[12] ^= 1;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&identity_tamper),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::IdentityMismatch)
        );
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&wrong_magic),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8..12].copy_from_slice(&9_u32.to_le_bytes());
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&wrong_version),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(9))
        );
        let mut legacy_version = encoded.clone();
        legacy_version[8..12].copy_from_slice(&7_u32.to_le_bytes());
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&legacy_version),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(7))
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&trailing),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::TrailingBytes)
        );
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&encoded[..encoded.len() - 1]),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)
        );
        let content_offset = 8 + 4 + 32;
        let selected_lowering_completion_status_offset = content_offset + 1 + 2 * 32;
        let x86_branch_relaxation_status_offset =
            selected_lowering_completion_status_offset + 1 + 32 + 12 * 32;
        let aarch64_cbnz_fusion_status_offset = x86_branch_relaxation_status_offset + 1;
        let aarch64_movn_materialization_status_offset = aarch64_cbnz_fusion_status_offset + 1;
        let mut unknown_stage = encoded.clone();
        unknown_stage[content_offset] = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&unknown_stage),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownStage(9))
        );
        let mut unknown_selected_lowering_completion = encoded.clone();
        unknown_selected_lowering_completion[selected_lowering_completion_status_offset] = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(
                &unknown_selected_lowering_completion
            ),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownSelectedLoweringCompletionStatus(9))
        );
        let mut unknown_x86_branch_relaxation = encoded.clone();
        unknown_x86_branch_relaxation[x86_branch_relaxation_status_offset] = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(
                &unknown_x86_branch_relaxation
            ),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownX86BranchRelaxationStatus(9))
        );
        let mut unknown_aarch64_cbnz_fusion = encoded.clone();
        unknown_aarch64_cbnz_fusion[aarch64_cbnz_fusion_status_offset] = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(
                &unknown_aarch64_cbnz_fusion
            ),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownAarch64CbnzFusionStatus(9))
        );
        let mut unknown_aarch64_movn_materialization = encoded.clone();
        unknown_aarch64_movn_materialization[aarch64_movn_materialization_status_offset] = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(
                &unknown_aarch64_movn_materialization
            ),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownAarch64MovnMaterializationStatus(9))
        );
        let target_offset = aarch64_movn_materialization_status_offset + 1 + 32;
        let mut unknown_architecture = encoded.clone();
        unknown_architecture[target_offset] = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&unknown_architecture),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownArchitecture(9))
        );
        let mut unknown_object_format = encoded.clone();
        unknown_object_format[target_offset + 1] = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&unknown_object_format),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownObjectFormat(9))
        );
        let layout_policy_offset = target_offset + 2 + 8 + 8;
        let mut unknown_layout_policy = encoded.clone();
        unknown_layout_policy[layout_policy_offset] = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&unknown_layout_policy),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownLayoutPolicy(9))
        );
        let mut unknown_scope = encoded.clone();
        unknown_scope[layout_policy_offset + 1] = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&unknown_scope),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownScope(9))
        );
        let mut unknown_unavailable = encoded.clone();
        *unknown_unavailable.last_mut().unwrap() = 9;
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&unknown_unavailable),
            Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownUnavailableStatus(9))
        );

        let mut corrupted = realization;
        macro_rules! assert_manifest_field_is_bound {
            ($field:ident, $replacement:expr) => {{
                let original = corrupted.manifest().record().$field;
                corrupted.manifest_mut().record_mut().$field = $replacement;
                assert_eq!(
                    validate_selected_lowering_function_relative_realization_custody(&corrupted),
                    Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
                );
                corrupted.manifest_mut().record_mut().$field = original;
            }};
        }
        assert_manifest_field_is_bound!(
            identity,
            omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(
                [0x50; 32]
            )
        );
        assert_manifest_field_is_bound!(
            selections,
            omega_optimization_core::OptimizationSelectionIdentity::from_bytes([0x51; 32])
        );
        assert_manifest_field_is_bound!(
            selected_lowering_selections,
            omega_optimization_core::OptimizationSelectionIdentity::from_bytes([0x52; 32])
        );
        assert_manifest_field_is_bound!(
            selected_lowering_completion,
            Some(
                omega_optimization_core::SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                    [0x53; 32]
                )
            )
        );
        assert_manifest_field_is_bound!(
            function_relative_layout_selections,
            omega_optimization_core::OptimizationSelectionIdentity::from_bytes([0x54; 32])
        );
        assert_manifest_field_is_bound!(
            pre_physical_manifest,
            omega_optimization_core::PrePhysicalOptimizationManifestIdentity::from_bytes(
                [0x54; 32]
            )
        );
        assert_manifest_field_is_bound!(
            post_allocation_manifest,
            omega_optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                [0x55; 32]
            )
        );
        assert_manifest_field_is_bound!(
            selected,
            omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0x56; 32])
        );
        assert_manifest_field_is_bound!(
            pre_allocation_machine_effects,
            omega_machine_optimizer::PreAllocationMachineEffectIdentity::from_bytes([0x57; 32])
        );
        assert_manifest_field_is_bound!(
            post_allocation_machine,
            omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes([0x58; 32])
        );
        assert_manifest_field_is_bound!(
            pre_layout,
            SelectedFormEncodingIdentity::from_bytes([0x59; 32])
        );
        assert_manifest_field_is_bound!(
            baseline_resolved_layout,
            ResolvedSelectedFormLayoutIdentity::from_bytes([0x5a; 32])
        );
        assert_manifest_field_is_bound!(
            resolved_layout,
            ResolvedSelectedFormLayoutIdentity::from_bytes([0x5b; 32])
        );
        assert_manifest_field_is_bound!(
            x86_branch_relaxation,
            Some(X86BranchRelaxationIdentity::from_bytes([0x5c; 32]))
        );
        assert_manifest_field_is_bound!(
            whole_function_exit_contract,
            WholeFunctionExitContractIdentity::from_bytes([0x5d; 32])
        );
        assert_manifest_field_is_bound!(
            target,
            if target == NativeTarget::linux_x64() {
                NativeTarget::linux_arm64()
            } else {
                NativeTarget::linux_x64()
            }
        );
        let original_bytes = corrupted.manifest().record().statistics.bytes;
        corrupted.manifest_mut().record_mut().statistics.bytes = original_bytes + 1;
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(&corrupted),
            Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
        );
        corrupted.manifest_mut().record_mut().statistics.bytes = original_bytes;
        let original_result_view = corrupted.exit_contract().contract().result_view;
        corrupted.exit_contract_mut().contract_mut().result_view = RegisterViewId(u16::MAX);
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(&corrupted),
            Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch
            ))
        );
        corrupted.exit_contract_mut().contract_mut().result_view = original_result_view;
        let original_exit_identity = corrupted.exit_contract().identity();
        corrupted.exit_contract_mut().contract_mut().identity =
            WholeFunctionExitContractIdentity::from_bytes([0x61; 32]);
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(&corrupted),
            Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch
            ))
        );
        corrupted.exit_contract_mut().contract_mut().identity = original_exit_identity;
        let original_offset = corrupted.exit_contract().contract().functions[0].returns[0].offset;
        corrupted.exit_contract_mut().contract_mut().functions[0].returns[0].offset =
            original_offset + 1;
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(&corrupted),
            Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch
            ))
        );
        corrupted.exit_contract_mut().contract_mut().functions[0].returns[0].offset =
            original_offset;
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(&corrupted).unwrap(),
            *corrupted.custody()
        );
    }
}

#[test]
fn frameless_exit_contract_rejects_unpreserved_x86_callee_saved_write() {
    let target = NativeTarget::linux_x64();
    let selections = OptimizationSelections::new([
        Optimization::CopyPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let legality = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                target,
                selections,
                selected_lowering_budget(),
            ))
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let run = run_selected_lowering_optimizations(legality).unwrap();
    assert!(run.steps().is_empty());
    let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
    let rbx_units = homes
        .selected_lowering_run()
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment()
        .physical()
        .model()
        .view_named("rbx")
        .unwrap()
        .units
        .clone();
    let error = stage_selected_lowering_function_relative_realization(homes).unwrap_err();
    let FunctionRelativeOptimizationRealizationError::ExitContract(
        WholeFunctionExitContractError::CalleeSavedWrite { instruction, unit },
    ) = error
    else {
        panic!("unpreserved RBX write must fail at the whole-function exit contract")
    };
    assert_eq!(
        instruction,
        omega_selected_instructions::SelectedInstructionId(3)
    );
    assert!(rbx_units.contains(&unit));
}
