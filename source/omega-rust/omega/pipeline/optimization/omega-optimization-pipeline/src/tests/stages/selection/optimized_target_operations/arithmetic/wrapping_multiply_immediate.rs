//! Public optimized-target custody for constant integer wrapping integer multiplication materialization.

use super::super::*;

#[test]
fn optimized_target_lowering_retains_constant_wrapping_integer_multiply_immediate_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_types() {
            for (left_value, right_value) in boundary_pairs(scalar_type) {
                let expected = scalar_type.wrapping_mul(left_value, right_value).unwrap();
                let (semantic, proof) = wrapping_integer_multiply_immediate_return_artifact(
                    scalar_type,
                    left_value,
                    right_value,
                );
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
                )
                .unwrap();
                let target =
                    lower_optimized_to_target_operations(optimized, target_profile).unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerMultiplyImmediate(
                        row,
                    ),
                ) = target.translation_validation().function_roster()[0].translation()
                else {
                    panic!("optimized integer wrapping integer multiplication must retain its exact immediate family")
                };
                assert_eq!(
                    row.left_constant_operation(),
                    OperationId::new(73_003).unwrap()
                );
                assert_eq!(
                    row.right_constant_operation(),
                    OperationId::new(73_005).unwrap()
                );
                assert_eq!(
                    row.wrapping_multiply_operation(),
                    OperationId::new(73_007).unwrap()
                );
                assert_eq!(row.left_constant_result(), ValueId::new(73_004).unwrap());
                assert_eq!(row.right_constant_result(), ValueId::new(73_006).unwrap());
                assert_eq!(
                    row.wrapping_multiply_result(),
                    ValueId::new(73_008).unwrap()
                );
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left_value(), left_value);
                assert_eq!(row.right_value(), right_value);
                assert_eq!(row.materialized_value(), expected);
                assert!(matches!(
                    target.target_operations().functions[0].operation,
                    TargetOperation::ReturnIntegerImmediate {
                        source_value,
                        scalar_type: target_type,
                        value,
                        ..
                    } if source_value == ValueId::new(73_008).unwrap()
                        && target_type == scalar_type
                        && value == expected
                ));
            }
        }
    }
}

fn native_types() -> Vec<IntegerType> {
    let mut types = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).unwrap()))
        .collect::<Vec<_>>();
    types.push(IntegerType::address(64).unwrap());
    types
}

fn boundary_pairs(scalar_type: IntegerType) -> [(IntegerValue, IntegerValue); 4] {
    let minimum = scalar_type.minimum_value();
    let maximum = scalar_type.maximum_value();
    let two = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(2),
        IntegerSign::Unsigned => IntegerValue::Unsigned(2),
    };
    [
        (minimum, minimum),
        (maximum, maximum),
        (maximum, two),
        (two, maximum),
    ]
}
