//! Public optimized-target custody through the first legalization-only boundary.

use crate::tests::*;

#[test]
fn projected_structural_call_return_reaches_legalization_on_all_targets_only() {
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
    }
}
