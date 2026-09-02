//! Public optimized-target custody for constant saturating integer-add materialization.

use super::super::*;

#[test]
fn optimized_target_lowering_retains_constant_saturating_integer_add_immediate_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_types() {
            for (left_value, right_value) in boundary_pairs(scalar_type) {
                let expected = scalar_type.saturating_add(left_value, right_value).unwrap();
                let (semantic, proof) = saturating_integer_add_immediate_return_artifact(
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
                    AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerAddImmediate(row),
                ) = target.translation_validation().function_roster()[0].translation()
                else {
                    panic!("optimized saturating integer-add must retain its exact immediate family")
                };
                assert_eq!(
                    row.left_constant_operation(),
                    OperationId::new(80_003).unwrap()
                );
                assert_eq!(
                    row.right_constant_operation(),
                    OperationId::new(80_005).unwrap()
                );
                assert_eq!(
                    row.saturating_add_operation(),
                    OperationId::new(80_007).unwrap()
                );
                assert_eq!(row.left_constant_result(), ValueId::new(80_004).unwrap());
                assert_eq!(row.right_constant_result(), ValueId::new(80_006).unwrap());
                assert_eq!(row.saturating_add_result(), ValueId::new(80_008).unwrap());
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
                    } if source_value == ValueId::new(80_008).unwrap()
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
    let one = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    let negative_or_zero = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => minimum,
    };
    [
        (maximum, one),
        (one, maximum),
        (minimum, negative_or_zero),
        (minimum, maximum),
    ]
}
