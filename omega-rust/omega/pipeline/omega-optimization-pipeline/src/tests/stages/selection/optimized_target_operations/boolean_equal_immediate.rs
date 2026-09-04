//! Public optimized-target custody for constant Boolean equality materialization.

use super::*;

#[test]
fn optimized_target_lowering_retains_constant_boolean_equality_immediate_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (left_value, right_value) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let (semantic, proof) =
                boolean_equal_immediate_return_artifact(left_value, right_value);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanEqualImmediate(row),
            ) = target.translation_validation().function_roster()[0].translation()
            else {
                panic!("optimized constant Boolean equality must retain its exact immediate family")
            };
            assert_eq!(
                row.left_constant_operation(),
                OperationId::new(69_003).unwrap()
            );
            assert_eq!(
                row.right_constant_operation(),
                OperationId::new(69_005).unwrap()
            );
            assert_eq!(row.equal_operation(), OperationId::new(69_007).unwrap());
            assert_eq!(row.left_constant_result(), ValueId::new(69_004).unwrap());
            assert_eq!(row.right_constant_result(), ValueId::new(69_006).unwrap());
            assert_eq!(row.equal_result(), ValueId::new(69_008).unwrap());
            assert_eq!(row.left_value(), left_value);
            assert_eq!(row.right_value(), right_value);
            assert_eq!(row.materialized_value(), left_value == right_value);
            assert!(matches!(
                target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanImmediate {
                    source_value,
                    value,
                    ..
                } if source_value == ValueId::new(69_008).unwrap()
                    && value == (left_value == right_value)
            ));
        }
    }
}
