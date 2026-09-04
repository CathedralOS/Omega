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
