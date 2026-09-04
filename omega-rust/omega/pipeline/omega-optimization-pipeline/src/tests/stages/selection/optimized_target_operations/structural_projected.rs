//! Public optimized target carrier retains independently validated projected-roster custody.

use super::*;

#[test]
fn optimized_target_carrier_retains_projected_structural_call_return_receipts() {
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
        let target = lower_optimized_to_target_operations(optimized, target_profile)
            .expect("public optimizer target custody admits the exact closure");
        let closure = target
            .translation_validation()
            .structural_call_return()
            .expect("the public carrier retains the whole-plan receipt");
        assert_eq!(closure.callee(), MachineId::new(3_801).unwrap());
        assert_eq!(closure.projected_qualifications().len(), 1);
        assert!(matches!(
            target.translation_validation().function_roster()[0].translation(),
            AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StructuralCallReturnCaller(_)
            )
        ));
        assert!(matches!(
            target.translation_validation().function_roster()[1].translation(),
            AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StructuralParameterReturnCallee(_)
            )
        ));
    }
}
