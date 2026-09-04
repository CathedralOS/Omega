use super::*;

#[test]
fn validates_exact_scalar_crash_on_every_native_target() {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for cause in [CrashCause::Trap, CrashCause::Abort] {
            for result_type in [ScalarType::Boolean, integer] {
                let source = crash_plan(cause, result_type);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineScalarCrash(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("exact scalar Crash must publish one validated family row")
                };
                assert_eq!(row.machine(), source.entry);
                assert_eq!(row.result_type(), result_type);
                assert_eq!(row.crash_edge(), EdgeId::new(2_004).unwrap());
                assert_eq!(row.cause(), cause);
                assert_eq!(row.site_guard(), guard_terms());
                assert_eq!(
                    row.frontier_lower_bound(),
                    [ClaimId::new(2_005).unwrap(), ClaimId::new(2_006).unwrap()]
                );
            }
        }
    }
}

#[test]
fn parameterized_scalar_crash_remains_explicitly_uncovered() {
    let mut source = base_plan();
    source.functions[0].parameters.push(AbstractParameter {
        value: ValueId::new(2_100).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert_eq!(
        receipt.function_roster()[0].translation(),
        &AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
}
