//! Public optimized-target custody through exact structural selection.

use crate::tests::*;

#[test]
fn projected_structural_call_return_reaches_selection_on_all_targets_only() {
    let (semantic, proof) = projected_structural_call_return_artifact();
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
        assert!(
            target
                .translation_validation()
                .structural_call_return()
                .is_some()
        );
        let legalized = legalize_target_operations(
            target.target_operations(),
            target.optimized().plan(),
            target.optimized().unit(),
        )
        .expect("validated target custody reaches identity legalization");
        let receipt = legalized
            .receipt()
            .projected_structural_call_return()
            .expect("legalization retains its own independent receipt");
        assert_eq!(receipt.projected_qualification_count(), 1);
        assert_eq!(legalized.plan().projected_structural_call_returns.len(), 1);
        let selected = stage_optimized_instruction_selection(target)
            .expect("exact projected structural closure reaches selection");
        let plan = selected.selected().plan();
        let [closure] = plan.projected_structural_call_returns.as_slice() else {
            panic!("selection must retain one atomic caller/callee closure")
        };
        assert_eq!(closure.fragments.len(), 8);
        assert_eq!(
            selected
                .selected()
                .receipt()
                .projected_structural_call_return_count(),
            1
        );
        assert_eq!(selected.custody().function_count(), 2);
        match target_profile.architecture {
            omega_target::Architecture::X86_64 => assert!(matches!(
                closure.callee_return_transfer,
                omega_selected_instructions::SelectedStructuralTransfer::FixedViewCopy { .. }
            )),
            omega_target::Architecture::Aarch64 => assert!(matches!(
                closure.callee_return_transfer,
                omega_selected_instructions::SelectedStructuralTransfer::SameViewNoCopy { .. }
            )),
        }
        assert!(matches!(
            analyze_machine_effects(selected.selected(), selected.register_environment()),
            Err(MachineEffectStageError::Analysis(
                omega_selected_instructions_to_machine_effects::MachineEffectError::ProjectedStructuralCallReturnUnsupported
            ))
        ));
        assert!(matches!(
            stage_optimized_liveness(selected),
            Err(OptimizedLivenessCustodyError::Analysis(
                omega_selected_instructions_to_register_homes::LivenessError::ProjectedStructuralCallReturnUnsupported
            ))
        ));
    }
}
