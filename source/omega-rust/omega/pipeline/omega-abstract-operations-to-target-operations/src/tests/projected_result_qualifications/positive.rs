//! Positive target transport and independent custody.

use super::*;

#[test]
fn exact_projected_structural_call_return_lowers_and_is_independently_receipted() {
    let source = projected_structural_call_return_plan();
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target = lower_to_target_operations(&source, target_profile)
            .expect("the target ABI admits the exact two-function closure");
        let receipt =
            crate::validate_abstract_to_target_translation(&source, target_profile, &target)
                .expect("independent plan and local-family replay accepts the closure");
        let closure = receipt
            .structural_call_return()
            .expect("whole-plan custody");
        assert_eq!(closure.caller(), MachineId::new(900).unwrap());
        assert_eq!(closure.callee(), MachineId::new(901).unwrap());
        assert_eq!(closure.projected_qualifications().len(), 2);
        assert!(matches!(
            receipt.function_roster()[0].translation(),
            crate::AbstractToTargetFunctionTranslationDisposition::Validated(
                crate::AbstractToTargetFunctionTranslationReceipt::StructuralCallReturnCaller(_)
            )
        ));
        assert!(matches!(
            receipt.function_roster()[1].translation(),
            crate::AbstractToTargetFunctionTranslationDisposition::Validated(
                crate::AbstractToTargetFunctionTranslationReceipt::StructuralParameterReturnCallee(
                    _
                )
            )
        ));
    }
}
