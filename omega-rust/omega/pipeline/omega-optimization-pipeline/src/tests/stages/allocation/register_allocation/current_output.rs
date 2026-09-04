use crate::tests::*;
use omega_regalloc::ValidatedSelectedAnalysis;

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
        let expected_machine = stage_optimized_post_allocation_machine_plan(&source).unwrap();
        let retained = RetainedAllocation::try_from(source).unwrap();
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
    }
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
