use crate::tests::*;
use omega_regalloc::ValidatedSelectedAnalysis;

fn assert_owned_program(
    retained: &omega_selected_instructions_to_register_homes::RetainedAllocation,
) {
    let current = retained.current();
    assert!(std::ptr::eq(
        retained.program().selected.as_ref(),
        current.program().selected
    ));
    assert!(std::ptr::eq(
        retained.program().homes.as_ref(),
        current.program().homes
    ));
    assert!(std::ptr::eq(
        retained.program().selected.as_ref(),
        current.selected_plan()
    ));
    assert!(std::ptr::eq(
        retained.program().homes.as_ref(),
        current.homes().plan()
    ));
    let replayed = retained.replay_allocation().unwrap();
    assert_eq!(
        replayed.selected_plan(),
        retained.program().selected.as_ref()
    );
    assert_eq!(replayed.homes().plan(), retained.program().homes.as_ref());
}

fn baseline(target: NativeTarget) -> StagedOptimizedRegisterHomes {
    let selected = staged_exact_add_conditional(target);
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    stage_optimized_register_homes(legality).unwrap()
}

#[test]
fn allocation_view_preserves_exact_machine_output_and_rejects_substitution() {
    let x86 = baseline(NativeTarget::linux_x64());
    let arm = baseline(NativeTarget::linux_arm64());
    for source in [&x86, &arm] {
        let allocation = source.replay_allocation().unwrap();
        assert_eq!(
            allocation.selected().selected_identity(),
            allocation.post_allocation_manifest().record().selected
        );
        assert_eq!(allocation.homes(), source.homes());
        assert_eq!(
            allocation.evidence(),
            &AllocationEvidence::RegisterHomes(source.custody())
        );
        let direct = stage_optimized_post_allocation_machine_plan(source).unwrap();
        let current = stage_optimized_post_allocation_machine_plan(&allocation).unwrap();
        assert_eq!(direct, current);
        assert_eq!(
            direct.machine().plan().encode(),
            current.machine().plan().encode()
        );
        assert_eq!(
            validate_optimized_post_allocation_machine_plan_custody(&allocation, &direct).unwrap(),
            *direct.custody()
        );
    }
    let machine = stage_optimized_post_allocation_machine_plan(&x86).unwrap();
    assert!(
        validate_optimized_post_allocation_machine_plan_custody(
            &arm.replay_allocation().unwrap(),
            &machine
        )
        .is_err()
    );
}

#[test]
fn retained_allocation_projects_the_same_facts_as_fresh_replay() {
    use omega_selected_instructions_to_register_homes::RetainedAllocation;

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = baseline(target);
        let original = source.replay_allocation().unwrap();
        let selected_owner = original.selected().shared_selected_plan();
        let home_owner = original.homes().shared_plan();
        let expected_machine = stage_optimized_post_allocation_machine_plan(&source).unwrap();
        let retained = RetainedAllocation::try_from(source).unwrap();
        assert_owned_program(&retained);
        assert!(std::sync::Arc::ptr_eq(
            &selected_owner,
            &retained.program().selected
        ));
        assert!(std::sync::Arc::ptr_eq(
            &home_owner,
            &retained.program().homes
        ));
        let current = retained.current();
        let replayed = retained.replay_allocation().unwrap();
        assert_eq!(current.selected_plan(), replayed.selected_plan());
        assert_eq!(current.evidence(), replayed.evidence());
        assert_eq!(current.homes(), replayed.homes());
        assert_eq!(
            current.post_allocation_manifest(),
            replayed.post_allocation_manifest()
        );
        assert!(std::ptr::eq(
            current.target_input(),
            replayed.target_input()
        ));
        assert_eq!(
            stage_optimized_post_allocation_machine_plan(&current).unwrap(),
            expected_machine
        );
        assert_eq!(
            stage_optimized_post_allocation_machine_plan(&retained).unwrap(),
            expected_machine
        );
        let program = retained.program().clone();
        drop(retained);
        assert_eq!(program.selected, selected_owner);
        assert_eq!(program.homes, home_owner);
    }
}

#[test]
fn current_program_substitution_cannot_pass_replay() {
    use omega_selected_instructions_to_register_homes::RetainedAllocation;

    for replace_selected in [false, true] {
        let mut retained =
            RetainedAllocation::try_from(baseline(NativeTarget::linux_x64())).unwrap();
        let alternate =
            RetainedAllocation::try_from(baseline(NativeTarget::linux_arm64())).unwrap();
        let mut substituted = retained.program().clone();
        if replace_selected {
            substituted.selected = alternate.program().selected.clone();
        } else {
            substituted.homes = alternate.program().homes.clone();
        }
        retained.substitute_current_program_for_test(substituted);
        assert!(matches!(
            retained.replay_allocation(),
            Err(AllocationReplayError::CurrentProgramMismatch)
        ));
    }
}

#[test]
fn editing_a_raw_program_clone_cannot_mutate_admitted_allocation() {
    use omega_selected_instructions_to_register_homes::RetainedAllocation;

    let retained = RetainedAllocation::try_from(baseline(NativeTarget::linux_x64())).unwrap();
    let selected_identity = retained.current().selected().selected_identity();
    let original_homes = retained.program().homes.encode();
    let mut proposal = retained.program().clone();
    std::sync::Arc::make_mut(&mut proposal.selected)
        .functions
        .clear();
    std::sync::Arc::make_mut(&mut proposal.homes)
        .functions
        .clear();
    assert_ne!(proposal, *retained.program());
    assert_eq!(retained.program().homes.encode(), original_homes);
    assert_eq!(
        retained.current().selected().selected_identity(),
        selected_identity
    );
    assert_owned_program(&retained);
}

#[test]
fn retained_allocation_rejects_selected_recovery_without_recovery_evidence() {
    use omega_selected_instructions_to_register_homes::RetainedAllocation;

    let selected = staged_exact_add_conditional_with_selections(
        NativeTarget::linux_x64(),
        OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        ])
        .unwrap(),
        budget(),
    );
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    assert!(matches!(
        RetainedAllocation::try_from(homes),
        Err(AllocationReplayError::SelectionMismatch)
    ));
}

#[test]
fn plain_recovery_realization_rejects_non_recovery_allocation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let homes = baseline(target);
        let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
        assert!(matches!(
            stage_allocation_recovery_function_relative_realization(homes, machine),
            Err(AllocationRecoveryFunctionRelativeRealizationError::UnsupportedSelections)
        ));
    }
}

#[test]
fn allocation_phase_matches_explicit_baseline_and_selected_lowering_sequences() {
    fn ranges(target: NativeTarget, lowering: bool) -> StagedOptimizedLiveRanges {
        let selections = OptimizationSelections::new(if lowering {
            vec![
                Optimization::CopyPropagation,
                Optimization::SelectedIncomingU12ExactAddImmediate,
            ]
        } else {
            vec![Optimization::CopyPropagation]
        })
        .unwrap();
        let selected = staged_exact_add_conditional_with_selections(target, selections, budget());
        stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap()
    }

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for lowering in [false, true] {
            let phase = stage_register_allocation(ranges(target, lowering)).unwrap();
            assert_owned_program(&phase);
            let expected = if lowering {
                let legality =
                    stage_optimized_allocation_legality_for_frameless_leaf(ranges(target, true))
                        .unwrap();
                let run = run_selected_lowering_optimizations(legality).unwrap();
                let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
                omega_selected_instructions_to_register_homes::RetainedAllocation::try_from(homes)
                    .unwrap()
            } else {
                let legality = stage_optimized_allocation_legality(ranges(target, false)).unwrap();
                let homes = stage_optimized_register_homes(legality).unwrap();
                omega_selected_instructions_to_register_homes::RetainedAllocation::try_from(homes)
                    .unwrap()
            };
            assert_eq!(
                phase.current().selected_plan(),
                expected.current().selected_plan()
            );
            assert_eq!(phase.current().homes(), expected.current().homes());
            assert_eq!(phase.current().evidence(), expected.current().evidence());
            let actual_machine =
                stage_optimized_post_allocation_machine_plan(&phase.current()).unwrap();
            let expected_machine =
                stage_optimized_post_allocation_machine_plan(&expected.current()).unwrap();
            assert_eq!(actual_machine, expected_machine);
            assert_eq!(
                actual_machine.machine().plan().encode(),
                expected_machine.machine().plan().encode()
            );
        }
    }
}

#[test]
fn fixed_view_recovery_publishes_the_same_owned_program_contract() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = conditional_forwarded_parameter_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(
                OptimizationSelections::new([
                    Optimization::CopyPropagation,
                    Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                ])
                .unwrap(),
                OptimizationWorkBudget::new(1024, 1024, 1024, 1024, 1024).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let target_input = lower_optimized_to_target_operations(optimized, target).unwrap();
        let selected = stage_optimized_instruction_selection(target_input).unwrap();
        let ranges =
            stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
        let retained = stage_register_allocation(ranges).unwrap();
        assert_owned_program(&retained);
        assert!(matches!(
            retained.current().evidence(),
            AllocationEvidence::FixedViewCopies(_)
        ));
        assert_eq!(
            stage_optimized_post_allocation_machine_plan(&retained).unwrap(),
            stage_optimized_post_allocation_machine_plan(&retained.current()).unwrap(),
        );
    }
}

#[test]
fn allocation_phase_matches_explicit_recovery_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_active_resident_exact_add_chain_with_selections(
            target,
            OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .unwrap(),
        );
        let declared_budget = selected.optimized_target().optimized().budget_per_pass();
        let ranges =
            stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
        let allocation = stage_register_allocation(ranges).unwrap();
        assert_owned_program(&allocation);
        let expected = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality(target),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            declared_budget,
        )
        .unwrap();
        let machine = stage_optimized_post_allocation_machine_plan(&expected).unwrap();
        let expected_current = expected.replay_allocation().unwrap();
        assert_eq!(
            allocation.current().selected_plan(),
            expected_current.selected_plan()
        );
        assert_eq!(allocation.current().homes(), expected_current.homes());
        assert_eq!(allocation.current().evidence(), expected_current.evidence());
        assert_eq!(
            stage_optimized_post_allocation_machine_plan(&allocation.current()).unwrap(),
            machine
        );
    }
}
