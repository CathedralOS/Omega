use super::*;

#[test]
fn optimized_target_lowering_retains_exact_scalar_crash_custody() {
    let integer = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 32).expect("native integer type"),
    );
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for cause in [CrashCause::Trap, CrashCause::Abort] {
            for result_type in [ScalarType::Boolean, integer] {
                let (semantic, proof) = scalar_crash_artifact(result_type, cause);
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
                )
                .unwrap();
                let target =
                    lower_optimized_to_target_operations(optimized, target_profile).unwrap();
                let receipt = target.translation_validation();
                assert_eq!(receipt.target(), target_profile);
                assert_eq!(receipt.function_count(), 1);
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineScalarCrash(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("optimized scalar Crash must retain its validated family row")
                };
                assert_eq!(row.result_type(), result_type);
                assert_eq!(row.cause(), cause);
                assert!(row.site_guard().is_empty());
                assert!(row.frontier_lower_bound().is_empty());
                assert!(matches!(
                    &target.target_operations().functions[0].operation,
                    TargetOperation::Crash {
                        cause: target_cause,
                        site_guard,
                        frontier_lower_bound,
                        ..
                    } if *target_cause == cause
                        && site_guard.is_empty()
                        && frontier_lower_bound.is_empty()
                ));
            }
        }
    }
}
