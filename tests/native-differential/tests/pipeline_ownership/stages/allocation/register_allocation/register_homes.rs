use crate::tests::{
    AllocationReplayError, AllocationSource, LiteralFoldPolicy, NativeTarget, Optimization,
    OptimizationSelections, OptimizedPostLiteralFoldHomeCustodyError,
    OptimizedPostLiteralFoldHomeCustodyFieldForTest, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyFieldForTest, OptimizedRegisterHomeCustodyFieldForTest,
    PostAllocationOptimizationManifest, PostAllocationOptimizationManifestError,
    RecoveryClassificationPolicy, RegisterHomeError, RegisterHomePlan, SpillChoicePolicy,
    register_home_identity, run_selected_lowering_optimizations, selected_lowering_budget,
    stage_first_optimized_literal_fold, stage_next_optimized_literal_fold,
    stage_optimized_allocation_legality, stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_register_homes, stage_optimized_register_homes_after_literal_folds,
    stage_optimized_register_homes_after_selected_lowering, staged_conditional,
    staged_forwarded_conditional, staged_single_block_exact_add_fold_legality,
    staged_widened_u8_exact_add_conditional_with_selections,
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
    validate_optimized_register_home_custody, validate_post_allocation_optimization_manifest,
    validate_register_homes,
};
#[test]
fn transition_free_register_homes_are_deterministic_and_cfg_exact() {
    for (target, condition_view, result_view) in [
        (NativeTarget::linux_x64(), "rdi", "rax"),
        (NativeTarget::linux_arm64(), "x0", "x0"),
    ] {
        let staged = stage_optimized_register_homes(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let function = &staged.homes().plan().functions[0];
        assert_eq!(function.assignments.len(), 6);
        let environment = staged
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let encoded = staged.homes().plan().encode();
        let decoded = RegisterHomePlan::decode(&encoded).unwrap();
        assert_eq!(&decoded, staged.homes().plan());
        let legality = staged.legality_stage();
        let ranges = legality.live_range_stage();
        let replay = validate_register_homes(
            legality.legality(),
            ranges.ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            decoded,
        )
        .unwrap();
        assert_eq!(replay, *staged.homes());
        let manifest = staged.post_allocation_manifest().record();
        assert_eq!(manifest.identity, manifest.recomputed_identity());
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&manifest.encode()),
            Ok(manifest.clone())
        );
        assert_eq!(manifest.pre_physical, staged.custody().manifest());
        assert_eq!(manifest.target, target);
        assert!(manifest.selected_transformations.is_empty());
        assert_eq!(manifest.homes, staged.homes().receipt().identity());
        assert_eq!(manifest.statistics.functions, 1);
        assert_eq!(manifest.statistics.assignments, 6);
        assert_eq!(manifest.statistics.fixed_view_transitions, 0);
        assert_eq!(
            staged.custody().post_allocation_manifest(),
            manifest.identity
        );
        assert_eq!(
            validate_optimized_register_home_custody(
                legality,
                staged.homes(),
                staged.post_allocation_manifest(),
            )
            .unwrap(),
            staged.custody()
        );
        assert!(manifest.render_text().contains("frame: unavailable"));
        assert_eq!(
            validate_post_allocation_optimization_manifest(
                manifest,
                staged.custody().manifest(),
                &[],
                ranges.ranges(),
                legality.legality(),
                staged.homes(),
            )
            .unwrap(),
            *staged.post_allocation_manifest()
        );
        let mut corrupted = manifest.clone();
        corrupted.statistics.assignments += 1;
        assert_eq!(
            validate_post_allocation_optimization_manifest(
                &corrupted,
                staged.custody().manifest(),
                &[],
                ranges.ranges(),
                legality.legality(),
                staged.homes(),
            ),
            Err(PostAllocationOptimizationManifestError::IdentityMismatch)
        );
        corrupted.identity = corrupted.recomputed_identity();
        assert_eq!(
            validate_post_allocation_optimization_manifest(
                &corrupted,
                staged.custody().manifest(),
                &[],
                ranges.ranges(),
                legality.legality(),
                staged.homes(),
            ),
            Err(PostAllocationOptimizationManifestError::ContentMismatch)
        );
        let model = environment.physical().model();
        assert_eq!(
            function.assignments[0].view,
            model.view_named(condition_view).unwrap().id
        );
        assert_eq!(
            function.assignments[3].view,
            model.view_named(result_view).unwrap().id
        );
        assert_eq!(function.assignments[3].view, function.assignments[5].view);
        assert!(
            staged
                .legality_stage()
                .live_range_stage()
                .ranges()
                .plan()
                .functions[0]
                .interference
                .is_empty()
        );
        for assignment in &function.assignments {
            let view = &model.views[usize::from(assignment.view.0)];
            assert_eq!(view.class, assignment.class);
            assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                environment
                    .reservations()
                    .reserved_units()
                    .binary_search(unit)
                    .is_err()
            }));
        }
        assert_eq!(staged.custody().assignment_count(), 6);
        assert_eq!(
            staged.custody().homes(),
            staged.homes().receipt().identity()
        );
        assert_eq!(
            staged.custody().register_environment(),
            environment.identity()
        );

        let repeated = stage_optimized_register_homes(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(staged.homes(), repeated.homes());
        assert_eq!(staged.custody(), repeated.custody());

        let mut corrupted = staged.homes().plan().clone();
        let original_view = corrupted.functions[0].assignments[0].view;
        corrupted.functions[0].assignments[0].view = model
            .views
            .iter()
            .find(|view| {
                view.class == corrupted.functions[0].assignments[0].class
                    && view.id != original_view
            })
            .expect("fixture register class has a distinct corruption view")
            .id;
        assert_ne!(
            register_home_identity(&corrupted),
            staged.homes().receipt().identity()
        );
        // A fresh canonical frame can carry invalid assignments. Data integrity
        // does not grant the independent allocator admission retained above.
        let decoded = RegisterHomePlan::decode(&corrupted.encode()).unwrap();
        assert_eq!(decoded, corrupted);
        let legality = staged.legality_stage();
        let ranges = legality.live_range_stage();
        assert!(matches!(
            validate_register_homes(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
                decoded,
            ),
            Err(RegisterHomeError::VirtualRegisterMismatch { .. })
                | Err(RegisterHomeError::UnknownOrIncompatibleView { .. })
        ));
    }

    let forwarded = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let forwarded = stage_optimized_register_homes(forwarded).unwrap();
    assert_eq!(forwarded.custody().assignment_count(), 6);
}

#[test]
fn optimized_register_home_custody_rejects_every_one_field_substitution() {
    use OptimizedRegisterHomeCustodyFieldForTest::*;
    let fields: [(&str, OptimizedRegisterHomeCustodyFieldForTest); 18] = [
        ("psi", Psi),
        ("target", Target),
        ("entry", Entry),
        ("optimization", Optimization),
        ("projection", Projection),
        ("manifest", Manifest),
        ("optimization_unit", OptimizationUnit),
        ("fuel_schedule", FuelSchedule),
        ("register_environment", RegisterEnvironment),
        ("allocator_availability", AllocatorAvailability),
        ("selected", Selected),
        ("liveness", Liveness),
        ("ranges", Ranges),
        ("legality", Legality),
        ("homes", Homes),
        ("post_allocation_manifest", PostAllocationManifest),
        ("function_count", FunctionCount),
        ("assignment_count", AssignmentCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            stage_optimized_register_homes(
                stage_optimized_allocation_legality(
                    stage_optimized_live_ranges(
                        stage_optimized_liveness(staged_conditional(target)).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
        };
        // The donor keeps the corruptor signature uniform with nested-source
        // families; no baseline field consumes it.
        let donor = build(match target.architecture {
            target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            _ => NativeTarget::linux_x64(),
        });
        assert_ne!(
            build(target).custody(),
            donor.custody(),
            "{target:?}: the foreign target must produce a distinct custody receipt",
        );
        for (name, field) in fields {
            let mut substituted = build(target);
            let honest = substituted.custody();
            substituted.corrupt_custody_for_test(field, &donor);
            assert_ne!(
                substituted.custody(),
                honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            let rebuilt = validate_optimized_register_home_custody(
                substituted.legality_stage(),
                substituted.homes(),
                substituted.post_allocation_manifest(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{target:?}: honest replay must still succeed after custody mutation `{name}`: {error:?}"
                )
            });
            assert_ne!(
                rebuilt,
                substituted.custody(),
                "{target:?}: independent replay must reject substituted homes-custody field {name}",
            );
            assert_eq!(
                substituted.replay_allocation().err(),
                Some(AllocationReplayError::ReceiptMismatch),
                "{target:?}: allocation replay must reject substituted homes-custody field {name}",
            );
        }
    }
}

#[test]
fn post_literal_fold_home_custody_rejects_every_one_field_substitution() {
    use OptimizedPostLiteralFoldHomeCustodyFieldForTest::*;
    let fields: [(&str, OptimizedPostLiteralFoldHomeCustodyFieldForTest); 5] = [
        ("source", Source),
        ("homes", Homes),
        ("post_allocation_manifest", PostAllocationManifest),
        ("function_count", FunctionCount),
        ("assignment_count", AssignmentCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            let legality = staged_single_block_exact_add_fold_legality(target);
            let folds = stage_first_optimized_literal_fold(
                legality,
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                LiteralFoldPolicy::EXACT_ADD_V1,
                selected_lowering_budget(),
            )
            .unwrap();
            // The first step folds one arm's stranded literal; the second
            // step folds the other arm's, after which every range seats in
            // the single allowlisted view.
            let folds = stage_next_optimized_literal_fold(
                folds,
                SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                LiteralFoldPolicy::EXACT_ADD_V1,
                selected_lowering_budget(),
            )
            .unwrap();
            stage_optimized_register_homes_after_literal_folds(folds).unwrap()
        };
        // An authentic foreign homes stage on the opposite architecture is
        // the donor for the nested literal-fold source custody receipt.
        let donor = build(match target.architecture {
            target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            _ => NativeTarget::linux_x64(),
        });
        assert_ne!(
            build(target).custody().source(),
            donor.custody().source(),
            "{target:?}: the foreign target must produce a distinct source custody receipt",
        );
        for (name, field) in fields {
            let mut substituted = build(target);
            let honest = substituted.custody().clone();
            substituted.corrupt_custody_for_test(field, &donor);
            assert_ne!(
                substituted.custody(),
                &honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            assert_eq!(
                validate_optimized_register_home_after_literal_fold_custody(&substituted),
                Err(OptimizedPostLiteralFoldHomeCustodyError::ReceiptMismatch),
                "{target:?}: independent replay must reject substituted literal-fold custody field {name}",
            );
            // The nested-source validator observes the custody mismatch first,
            // so the replay surfaces it through the stage wrapper rather than
            // the unreachable plain `ReceiptMismatch` arm.
            assert_eq!(
                substituted.replay_allocation().err(),
                Some(AllocationReplayError::LiteralFolds(
                    OptimizedPostLiteralFoldHomeCustodyError::ReceiptMismatch
                )),
                "{target:?}: allocation replay must reject substituted literal-fold custody field {name}",
            );
        }
    }
}

#[test]
fn post_selected_lowering_home_custody_rejects_every_one_field_substitution() {
    use OptimizedPostSelectedLoweringHomeCustodyFieldForTest::*;
    let fields: [(&str, OptimizedPostSelectedLoweringHomeCustodyFieldForTest); 5] = [
        ("source", Source),
        ("homes", Homes),
        ("post_allocation_manifest", PostAllocationManifest),
        ("function_count", FunctionCount),
        ("assignment_count", AssignmentCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            let legality = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(
                        staged_widened_u8_exact_add_conditional_with_selections(
                            target,
                            OptimizationSelections::new([
                                Optimization::CopyPropagation,
                                Optimization::SelectedIncomingU12ExactAddImmediate,
                            ])
                            .unwrap(),
                        ),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let run = run_selected_lowering_optimizations(legality).unwrap();
            stage_optimized_register_homes_after_selected_lowering(run).unwrap()
        };
        // An authentic foreign homes stage on the opposite architecture is
        // the donor for the nested selected-lowering source custody receipt.
        let donor = build(match target.architecture {
            target::Architecture::X86_64 => NativeTarget::linux_arm64(),
            _ => NativeTarget::linux_x64(),
        });
        assert_ne!(
            build(target).custody().source(),
            donor.custody().source(),
            "{target:?}: the foreign target must produce a distinct source custody receipt",
        );
        for (name, field) in fields {
            let mut substituted = build(target);
            let honest = substituted.custody().clone();
            substituted.corrupt_custody_for_test(field, &donor);
            assert_ne!(
                substituted.custody(),
                &honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            assert_eq!(
                validate_optimized_register_home_after_selected_lowering_custody(&substituted),
                Err(OptimizedPostSelectedLoweringHomeCustodyError::ReceiptMismatch),
                "{target:?}: independent replay must reject substituted selected-lowering custody field {name}",
            );
            // Same wrapper shape as the literal-fold matrix above.
            assert_eq!(
                substituted.replay_allocation().err(),
                Some(AllocationReplayError::SelectedLowering(
                    OptimizedPostSelectedLoweringHomeCustodyError::ReceiptMismatch
                )),
                "{target:?}: allocation replay must reject substituted selected-lowering custody field {name}",
            );
        }
    }
}
