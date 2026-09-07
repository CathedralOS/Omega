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
