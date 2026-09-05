//! Public optimized-target custody for constant integer equality materialization.

use super::*;

#[test]
fn optimized_target_lowering_retains_constant_integer_equality_immediate_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_types() {
            for (left_value, right_value) in boundary_pairs(scalar_type) {
                let (semantic, proof) =
                    integer_equal_immediate_return_artifact(scalar_type, left_value, right_value);
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
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerEqualImmediate(
                        row,
                    ),
                ) = target.translation_validation().function_roster()[0].translation()
                else {
                    panic!(
                        "optimized constant integer equality must retain its exact immediate family"
                    )
                };
                assert_eq!(
                    row.left_constant_operation(),
                    OperationId::new(70_003).unwrap()
                );
                assert_eq!(
                    row.right_constant_operation(),
                    OperationId::new(70_005).unwrap()
                );
                assert_eq!(row.equal_operation(), OperationId::new(70_007).unwrap());
                assert_eq!(row.left_constant_result(), ValueId::new(70_004).unwrap());
                assert_eq!(row.right_constant_result(), ValueId::new(70_006).unwrap());
                assert_eq!(row.equal_result(), ValueId::new(70_008).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left_value(), left_value);
                assert_eq!(row.right_value(), right_value);
                assert_eq!(row.materialized_value(), left_value == right_value);
                assert!(matches!(
                    target.target_operations().functions[0].operation,
                    TargetOperation::ReturnBooleanImmediate {
                        source_value,
                        value,
                        ..
                    } if source_value == ValueId::new(70_008).unwrap()
                        && value == (left_value == right_value)
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
    [
        (minimum, minimum),
        (minimum, maximum),
        (maximum, minimum),
        (maximum, maximum),
    ]
}
