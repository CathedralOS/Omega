use crate::tests::*;

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
fn fixed_view_copies_are_explicit_reanalyzed_and_deterministic() {
    for (target, entry_name, result_name) in [
        (NativeTarget::linux_x64(), "rsi", "rax"),
        (NativeTarget::linux_arm64(), "x1", "x0"),
    ] {
        let source = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let source_selected = source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .selected()
            .plan()
            .clone();
        let source_manifest = source.custody().manifest();
        let materialized = stage_optimized_fixed_view_copies(
            with_segment_homes(source),
            FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            budget(),
        )
        .unwrap();
        let machine_effects =
            stage_optimized_machine_effects_after_fixed_view_copies(&materialized).unwrap();
        assert_eq!(
            machine_effects.effects().receipt().selected(),
            materialized.custody().transformed_selected()
        );
        assert_eq!(
            machine_effects.custody().source(),
            &StagedOptimizedMachineEffectSourceCustodyReceipt::FixedViewCopies(
                materialized.custody()
            )
        );
        assert_eq!(
            &validate_optimized_machine_effect_custody_after_fixed_view_copies(
                &materialized,
                machine_effects.effects(),
            )
            .unwrap(),
            machine_effects.custody()
        );
        let copy_plan = materialized.copies().plan();
        assert_eq!(copy_plan.copies.len(), 2);
        assert_eq!(materialized.custody().copy_count(), 2);
        assert_eq!(materialized.custody().manifest(), source_manifest);
        assert_eq!(
            copy_plan.usage,
            omega_optimization_core::OptimizationWorkUsage {
                rule_evaluations: 5,
                candidates: 4,
                validation_steps: 20,
                commits: 4,
                iterations: 11,
            }
        );
        assert_ne!(
            materialized.custody().source_selected(),
            materialized.custody().transformed_selected()
        );
        assert_eq!(
            fixed_view_copy_identity(copy_plan),
            materialized.custody().transformation()
        );
        let transformed = &copy_plan.transformed;
        assert_eq!(transformed.functions[0].virtual_registers.len(), 4);
        let environment = materialized
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let entry_view = environment
            .physical()
            .model()
            .view_named(entry_name)
            .unwrap()
            .id;
        let result_view = environment
            .physical()
            .model()
            .view_named(result_name)
            .unwrap()
            .id;
        for (index, copy) in copy_plan.copies.iter().enumerate() {
            assert_eq!(copy.source_virtual_register, VirtualRegisterId(1));
            assert_eq!(copy.result_virtual_register.0, 2 + index as u32);
            assert_eq!(copy.copy_instruction.0, 4 + index as u32);
            assert_eq!(copy.from_view, entry_view);
            assert_eq!(copy.to_view, result_view);
            assert_eq!(copy.copy_constraint, environment.selected_keys().copy_i64);
            let block = &transformed.functions[0].blocks[index + 1];
            let instruction = block.instructions.last().unwrap();
            assert_eq!(instruction.id, copy.copy_instruction);
            assert_eq!(instruction.kind, SelectedInstructionKind::CopyI64);
            assert_eq!(
                instruction.operands[0].virtual_register,
                copy.source_virtual_register
            );
            assert_eq!(
                instruction.operands[1].virtual_register,
                copy.result_virtual_register
            );
            assert!(instruction.provenance.operations.is_empty());
            assert_eq!(instruction.provenance.values, vec![copy.source_value]);
            assert!(instruction.provenance.edges.is_empty());
            assert!(instruction.provenance.obligations.is_empty());
            assert!(instruction.provenance.fuel.is_empty());
            let SelectedTerminator::Return {
                instruction: source_return,
                ..
            } = &source_selected.functions[0].blocks[index + 1].terminator
            else {
                unreachable!()
            };
            let SelectedTerminator::Return {
                instruction: transformed_return,
                ..
            } = &block.terminator
            else {
                unreachable!()
            };
            assert_eq!(source_return.id, transformed_return.id);
            assert_eq!(source_return.provenance, transformed_return.provenance);
            assert_eq!(
                transformed_return.operands[0].virtual_register,
                copy.result_virtual_register
            );
        }

        let mut corrupted = materialized.copies().plan().clone();
        corrupted.copies[0].from_view = result_view;
        assert!(matches!(
            validate_fixed_view_copies(
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .selected(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .ranges(),
                materialized.source_legality_stage().legality(),
                materialized.source_segment_home_stage().fixed_intervals(),
                materialized
                    .source_segment_home_stage()
                    .split_requirements(),
                materialized.source_segment_home_stage().segment_homes(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted,
            ),
            Err(FixedViewCopyError::CopyMismatch { index: 0 })
        ));
        let mut corrupted = materialized.copies().plan().clone();
        corrupted.transformed.functions[0].blocks[1].instructions[0]
            .provenance
            .values
            .clear();
        assert!(matches!(
            validate_fixed_view_copies(
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .selected(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .ranges(),
                materialized.source_legality_stage().legality(),
                materialized.source_segment_home_stage().fixed_intervals(),
                materialized
                    .source_segment_home_stage()
                    .split_requirements(),
                materialized.source_segment_home_stage().segment_homes(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted,
            ),
            Err(FixedViewCopyError::TransformedPlanMismatch)
        ));
        let mut corrupted = materialized.copies().plan().clone();
        corrupted.usage.commits += 1;
        assert!(matches!(
            validate_fixed_view_copies(
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .selected(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .ranges(),
                materialized.source_legality_stage().legality(),
                materialized.source_segment_home_stage().fixed_intervals(),
                materialized
                    .source_segment_home_stage()
                    .split_requirements(),
                materialized.source_segment_home_stage().segment_homes(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted,
            ),
            Err(FixedViewCopyError::ReceiptMismatch)
        ));
        assert!(matches!(
            validate_live_ranges(
                materialized.copies(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .liveness_stage()
                    .liveness(),
                materialized
                    .source_legality_stage()
                    .live_range_stage()
                    .ranges()
                    .plan()
                    .clone(),
            ),
            Err(LiveRangeError::LivenessRevalidation(
                LivenessError::RootMismatch
            ))
        ));

        let reanalyzed = stage_optimized_selected_reanalysis(materialized).unwrap();
        assert_eq!(reanalyzed.custody().entry_transition_count(), 0);
        assert_eq!(reanalyzed.legality().receipt().entry_transition_count(), 0);
        let homes = stage_optimized_register_homes_after_fixed_view_copies(reanalyzed).unwrap();
        let post =
            stage_optimized_post_allocation_machine_plan_after_fixed_view_copies(&homes).unwrap();
        assert_eq!(
            post.machine().receipt().selected(),
            homes.reanalysis_stage().ranges().receipt().selected()
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody(
                &homes, &post,
            )
            .unwrap(),
            post.custody()
        );
        let assignments = &homes.homes().plan().functions[0].assignments;
        assert_eq!(assignments.len(), 4);
        assert_eq!(assignments[1].view, entry_view);
        assert_ne!(assignments[0].view, assignments[1].view);
        assert_eq!(assignments[2].view, result_view);
        assert_eq!(assignments[3].view, result_view);
        assert_eq!(
            homes.reanalysis_stage().ranges().plan().functions[0].interference,
            vec![VirtualInterference {
                lower: VirtualRegisterId(0),
                higher: VirtualRegisterId(1),
            }]
        );
        assert_eq!(homes.custody().assignment_count(), 4);
        let manifest = homes.post_allocation_manifest().record();
        assert_eq!(manifest.identity, manifest.recomputed_identity());
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&manifest.encode()),
            Ok(manifest.clone())
        );
        assert_eq!(
            manifest.selected_transformations,
            vec![PostAllocationSelectedTransformation::FixedViewCopy(
                homes.custody().source().source().transformation()
            )]
        );
        assert_eq!(
            manifest.selected,
            homes.reanalysis_stage().ranges().plan().selected
        );
        assert_eq!(manifest.statistics.assignments, 4);
        assert_eq!(manifest.statistics.virtual_interferences, 1);
        let transformation = PostAllocationSelectedTransformation::FixedViewCopy(
            homes.custody().source().source().transformation(),
        );
        assert_eq!(
            validate_post_allocation_optimization_manifest(
                manifest,
                homes.custody().source().source().manifest(),
                &[transformation, transformation],
                homes.reanalysis_stage().ranges(),
                homes.reanalysis_stage().legality(),
                homes.homes(),
            ),
            Err(PostAllocationOptimizationManifestError::NonCanonicalTransformationLedger)
        );
        assert_eq!(
            homes.custody().post_allocation_manifest(),
            manifest.identity
        );
        assert_eq!(
            validate_optimized_register_home_after_fixed_view_copy_custody(
                homes.reanalysis_stage(),
                homes.homes(),
                homes.post_allocation_manifest(),
            )
            .unwrap(),
            homes.custody()
        );

        let repeated = stage_optimized_register_homes_after_fixed_view_copies(
            stage_optimized_selected_reanalysis(
                stage_optimized_fixed_view_copies(
                    with_segment_homes(
                        stage_optimized_allocation_legality(
                            stage_optimized_live_ranges(
                                stage_optimized_liveness(staged_forwarded_conditional(target))
                                    .unwrap(),
                            )
                            .unwrap(),
                        )
                        .unwrap(),
                    ),
                    FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                    budget(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(homes.homes(), repeated.homes());
        assert_eq!(homes.custody(), repeated.custody());
    }

    let constrained = OptimizationWorkBudget::new(128, 128, 128, 1, 16).unwrap();
    let source = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        stage_optimized_fixed_view_copies(
            with_segment_homes(source),
            FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            constrained,
        ),
        Err(OptimizedFixedViewCopyCustodyError::Materialization(
            FixedViewCopyError::BudgetExceeded { .. }
        ))
    ));

    let constant = stage_optimized_fixed_view_copies(
        with_segment_homes(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64()))
                        .unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        ),
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
        budget(),
    )
    .unwrap();
    assert!(constant.copies().plan().copies.is_empty());
    assert_eq!(
        constant.copies().plan().source_selected,
        constant.copies().receipt().transformed_selected()
    );
}
