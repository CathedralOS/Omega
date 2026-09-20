use crate::tests::{
    AllocationEvidence, AllocationReplayError, AllocationSource, FixedViewCopyPolicy, NativeTarget,
    Optimization, OptimizationSelections, OptimizationWorkBudget,
    OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest,
    OptimizedFixedViewCopyCustodyFieldForTest, OptimizedPostCopyRegisterHomeCustodyFieldForTest,
    OptimizedSelectedReanalysisCustodyFieldForTest, PostAllocationSelectedTransformation,
    SelectedInstructionKind, SelectedTerminator, StagedOptimizedAllocationLegality,
    StagedOptimizedFixedPrecoloredSegmentHomes, analyze_machine_effects, selected_lowering_budget,
    stage_leaf_local_fixed_view_register_allocation, stage_optimized_allocation_legality,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
    stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_register_homes_after_fixed_view_copies, stage_optimized_selected_reanalysis,
    staged_forwarded_conditional, validate_fixed_view_copies, validate_machine_effects,
    validate_optimized_fixed_precolored_segment_home_custody,
    validate_optimized_fixed_view_copy_custody,
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_selected_reanalysis_custody,
};
fn with_segment_homes(
    source: StagedOptimizedAllocationLegality,
) -> StagedOptimizedFixedPrecoloredSegmentHomes {
    stage_optimized_fixed_precolored_segment_homes(
        source,
        OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap(),
    )
    .unwrap()
}

#[test]
fn explicit_abi_transfers_are_not_duplicated_and_reanalysis_is_deterministic() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = || {
            let source = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            stage_optimized_fixed_view_copies(
                with_segment_homes(source),
                FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                selected_lowering_budget(),
            )
            .unwrap()
        };
        let materialized = build();
        let repeated = build();
        assert_eq!(materialized.copies(), repeated.copies());
        assert_eq!(materialized.custody(), repeated.custody());
        let source = materialized.source_legality_stage();
        let selected_stage = source.live_range_stage().liveness_stage().selected_stage();
        let environment = selected_stage.register_environment();
        let plan = materialized.copies().plan();
        assert!(plan.copies.is_empty());
        assert_eq!(materialized.custody().copy_count(), 0);
        assert_eq!(plan.transformed.as_ref(), selected_stage.selected().plan());
        assert_eq!(
            materialized.custody().source_selected(),
            materialized.custody().transformed_selected()
        );
        // The two return copies already belong to the selected program.
        for block in &plan.transformed.functions[0].blocks[1..] {
            let copy = block.instructions.last().unwrap();
            assert_eq!(copy.kind, SelectedInstructionKind::CopyI64);
            let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
                unreachable!()
            };
            assert_eq!(
                copy.operands[1].virtual_register,
                instruction.operands[0].virtual_register
            );
            assert_ne!(
                copy.operands[0].virtual_register,
                copy.operands[1].virtual_register
            );
        }
        let replay = |candidate| {
            validate_fixed_view_copies(
                selected_stage.selected(),
                source.live_range_stage().ranges(),
                source.legality(),
                materialized.source_segment_home_stage().fixed_intervals(),
                materialized
                    .source_segment_home_stage()
                    .split_requirements(),
                materialized.source_segment_home_stage().segment_homes(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
                candidate,
            )
        };
        assert!(replay(plan.clone()).is_ok());
        let mut changed = plan.clone();
        changed.usage.validation_steps += 1;
        assert!(replay(changed).is_err());
        let mut changed = plan.clone();
        std::sync::Arc::make_mut(&mut changed.transformed).functions[0].blocks[1].instructions[0]
            .provenance
            .values
            .clear();
        assert!(replay(changed).is_err());
        let effects = analyze_machine_effects(materialized.copies(), environment).unwrap();
        validate_machine_effects(materialized.copies(), environment, &effects).unwrap();
        assert_eq!(
            effects.receipt().selected(),
            materialized.custody().transformed_selected()
        );
        let reanalyzed = stage_optimized_selected_reanalysis(materialized).unwrap();
        assert_eq!(reanalyzed.custody().entry_transition_count(), 0);
        let homes = stage_optimized_register_homes_after_fixed_view_copies(reanalyzed).unwrap();
        let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
        assert_eq!(
            post.machine().receipt().selected(),
            homes.reanalysis_stage().ranges().receipt().selected()
        );
    }
}

fn leaf_local_input(
    target: NativeTarget,
    selections: OptimizationSelections,
) -> StagedOptimizedAllocationLegality {
    use crate::tests::{
        AdmissionProfile, ExplicitOptimizationRequest, OptimizedTargetLoweringRequest,
        conditional_forwarded_parameter_artifact, lower_optimized_to_target_operations,
        optimize_artifact_sections, stage_optimized_instruction_selection,
    };
    // The fixed/precolored analyses consume the unit's declared per-pass
    // budget, so the staged input carries the same explicit budget the
    // shared-entry recovery fixture declares.
    let (semantic, proof) = conditional_forwarded_parameter_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(
            selections,
            OptimizationWorkBudget::new(1024, 2048, 2048, 1024, 1024).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let target_input = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    let selected = stage_optimized_instruction_selection(target_input).unwrap();
    stage_optimized_allocation_legality(
        stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap(),
    )
    .unwrap()
}

#[test]
fn leaf_local_default_path_sequence_retains_and_publishes() {
    use selected_instructions_to_register_homes::{AllocationSource, RetainedAllocation};

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = || {
            stage_leaf_local_fixed_view_register_allocation(leaf_local_input(
                target,
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            ))
            .unwrap()
        };
        let homes = build();
        let copy_custody = homes.custody().source().source();
        assert_eq!(
            copy_custody.policy(),
            FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1
        );
        assert_eq!(copy_custody.copy_count(), 0);
        let retained = RetainedAllocation::try_from(build()).unwrap();
        let current = retained.current();
        let replayed = retained.replay_allocation().unwrap();
        assert_eq!(current.selected_plan(), replayed.selected_plan());
        assert_eq!(current.homes(), replayed.homes());
        assert_eq!(current.evidence(), replayed.evidence());
        assert!(matches!(
            current.evidence(),
            AllocationEvidence::FixedViewCopies(_)
        ));
        // The post-allocation manifest records the fixed-view transformation
        // even when the admitted boundary set materialized no copies.
        assert_eq!(
            current
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            vec![PostAllocationSelectedTransformation::FixedViewCopy(
                copy_custody.transformation()
            )]
        );
        // The retained owner publishes through the same machine-plan stage as
        // the staged result it wraps.
        let published = stage_optimized_post_allocation_machine_plan(&retained).unwrap();
        assert_eq!(
            published,
            stage_optimized_post_allocation_machine_plan(&current).unwrap()
        );
        assert_eq!(
            published,
            stage_optimized_post_allocation_machine_plan(&homes).unwrap()
        );
    }
}

#[test]
fn leaf_local_default_path_sequence_rejects_a_declared_recovery_selection() {
    use selected_instructions_to_register_homes::RetainedAllocation;

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let homes = stage_leaf_local_fixed_view_register_allocation(leaf_local_input(
            target,
            OptimizationSelections::new([
                Optimization::CopyPropagation,
                Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            ])
            .unwrap(),
        ))
        .unwrap();
        assert!(matches!(
            RetainedAllocation::try_from(homes),
            Err(AllocationReplayError::SelectionMismatch)
        ));
    }
}

#[test]
fn optimized_selected_reanalysis_custody_rejects_every_one_field_substitution() {
    use OptimizedSelectedReanalysisCustodyFieldForTest::*;
    let fields: [(&str, OptimizedSelectedReanalysisCustodyFieldForTest); 8] = [
        ("source", Source),
        ("transformed_liveness", TransformedLiveness),
        ("transformed_ranges", TransformedRanges),
        ("transformed_legality", TransformedLegality),
        ("allocator_availability", AllocatorAvailability),
        ("function_count", FunctionCount),
        ("virtual_register_count", VirtualRegisterCount),
        ("entry_transition_count", EntryTransitionCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            let source = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            stage_optimized_selected_reanalysis(
                stage_optimized_fixed_view_copies(
                    with_segment_homes(source),
                    FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                    selected_lowering_budget(),
                )
                .unwrap(),
            )
            .unwrap()
        };
        // An authentic foreign reanalysis on the opposite architecture is the
        // donor for the nested fixed-view-copy source custody receipt.
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
            let honest = substituted.custody();
            substituted.corrupt_custody_for_test(field, &donor);
            assert_ne!(
                substituted.custody(),
                honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            let rebuilt = validate_optimized_selected_reanalysis_custody(
                substituted.transformation_stage(),
                substituted.liveness(),
                substituted.ranges(),
                substituted.legality(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{target:?}: honest replay must still succeed after custody mutation `{name}`: {error:?}"
                )
            });
            assert_ne!(
                rebuilt,
                substituted.custody(),
                "{target:?}: independent replay must reject substituted reanalysis-custody field {name}",
            );
        }
    }
}

#[test]
fn post_copy_register_home_custody_rejects_every_one_field_substitution() {
    use OptimizedPostCopyRegisterHomeCustodyFieldForTest::*;
    let fields: [(&str, OptimizedPostCopyRegisterHomeCustodyFieldForTest); 5] = [
        ("source", Source),
        ("homes", Homes),
        ("post_allocation_manifest", PostAllocationManifest),
        ("function_count", FunctionCount),
        ("assignment_count", AssignmentCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            let source = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            stage_optimized_register_homes_after_fixed_view_copies(
                stage_optimized_selected_reanalysis(
                    stage_optimized_fixed_view_copies(
                        with_segment_homes(source),
                        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                        selected_lowering_budget(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
        };
        // An authentic foreign homes stage on the opposite architecture is
        // the donor for the nested reanalysis source custody receipt.
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
            let honest = substituted.custody();
            substituted.corrupt_custody_for_test(field, &donor);
            assert_ne!(
                substituted.custody(),
                honest,
                "{target:?}: mutation `{name}` must change the retained custody receipt",
            );
            let rebuilt = validate_optimized_register_home_after_fixed_view_copy_custody(
                substituted.reanalysis_stage(),
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
                "{target:?}: independent replay must reject substituted post-copy custody field {name}",
            );
            assert_eq!(
                substituted.replay_allocation().err(),
                Some(AllocationReplayError::ReceiptMismatch),
                "{target:?}: allocation replay must reject substituted post-copy custody field {name}",
            );
        }
    }
}

#[test]
fn fixed_precolored_segment_home_custody_rejects_every_one_field_substitution() {
    use OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest::*;
    let fields: [(&str, OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest); 4] = [
        ("upstream", Upstream),
        ("fixed", Fixed),
        ("requirements", Requirements),
        ("homes", Homes),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            let source = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            with_segment_homes(source)
        };
        // An authentic foreign segment-homes stage on the opposite
        // architecture is the donor for the nested receipts.
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
            let rebuilt = validate_optimized_fixed_precolored_segment_home_custody(
                substituted.source_legality_stage(),
                substituted.fixed_intervals(),
                substituted.split_requirements(),
                substituted.segment_homes(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{target:?}: honest replay must still succeed after custody mutation `{name}`: {error:?}"
                )
            });
            assert_ne!(
                rebuilt,
                substituted.custody(),
                "{target:?}: independent replay must reject substituted segment-home custody field {name}",
            );
        }
    }
}

#[test]
fn fixed_view_copy_custody_rejects_every_one_field_substitution() {
    use OptimizedFixedViewCopyCustodyFieldForTest::*;
    let fields: [(&str, OptimizedFixedViewCopyCustodyFieldForTest); 23] = [
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
        ("source_selected", SourceSelected),
        ("source_liveness", SourceLiveness),
        ("source_ranges", SourceRanges),
        ("source_legality", SourceLegality),
        ("fixed_intervals", FixedIntervals),
        ("split_requirements", SplitRequirements),
        ("segment_homes", SegmentHomes),
        ("transformation", Transformation),
        ("transformed_selected", TransformedSelected),
        ("policy", Policy),
        ("usage", Usage),
        ("function_count", FunctionCount),
        ("copy_count", CopyCount),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let build = |target: NativeTarget| {
            let source = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            stage_optimized_fixed_view_copies(
                with_segment_homes(source),
                FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                selected_lowering_budget(),
            )
            .unwrap()
        };
        // An authentic foreign copy stage on the opposite architecture is the
        // donor; flat fields mutate in place and ignore it.
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
            let rebuilt = validate_optimized_fixed_view_copy_custody(
                substituted.source_segment_home_stage(),
                substituted.copies(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{target:?}: honest replay must still succeed after custody mutation `{name}`: {error:?}"
                )
            });
            assert_ne!(
                rebuilt,
                substituted.custody(),
                "{target:?}: independent replay must reject substituted fixed-view-copy custody field {name}",
            );
        }
    }
}
