//! Public optimized-target custody for proof-bearing wrapping divide over constant operands.

use super::super::*;

#[test]
fn optimized_target_lowering_retains_wrapping_divide_immediate_operand_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_fixed_types() {
            for (left, right) in boundary_pairs(scalar_type) {
                let expected = scalar_type.wrapping_div(left, right).unwrap();
                let (semantic, proof) =
                    wrapping_integer_divide_immediate_operands_return_artifact(
                        scalar_type,
                        left,
                        right,
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
                    AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerDivideImmediateOperands(row),
                ) = target.translation_validation().function_roster()[0].translation()
                else {
                    panic!("optimized constant-operand wrapping divide must retain its proof-bearing family")
                };
                assert_eq!(row.machine(), MachineId::new(84_001).unwrap());
                assert_eq!(row.left_constant_operation(), OperationId::new(84_003).unwrap());
                assert_eq!(row.right_constant_operation(), OperationId::new(84_005).unwrap());
                assert_eq!(row.divide_operation(), OperationId::new(84_007).unwrap());
                assert_eq!(row.obligation(), ObligationId::new(84_011).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(84_009).unwrap());
                assert_eq!(row.left_constant_result(), ValueId::new(84_004).unwrap());
                assert_eq!(row.right_constant_result(), ValueId::new(84_006).unwrap());
                assert_eq!(row.divide_result(), ValueId::new(84_008).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left(), left);
                assert_eq!(row.right(), right);
                assert_eq!(row.quotient(), expected);
                assert!(matches!(
                    &target.target_operations().functions[0].operation,
                    TargetOperation::ReturnIntegerExpression {
                        source_value,
                        scalar_type: target_type,
                        expression: TargetIntegerExpression::WrappingDivide {
                            psi_operation,
                            obligation,
                            left: target_left,
                            right: target_right,
                        },
                        ..
                    } if *source_value == ValueId::new(84_008).unwrap()
                        && *target_type == scalar_type
                        && *psi_operation == OperationId::new(84_007).unwrap()
                        && *obligation == ObligationId::new(84_011).unwrap()
                        && matches!(
                            target_left.as_ref(),
                            TargetIntegerExpression::Immediate { source_value, value }
                                if *source_value == ValueId::new(84_004).unwrap() && *value == left
                        )
                        && matches!(
                            target_right.as_ref(),
                            TargetIntegerExpression::Immediate { source_value, value }
                                if *source_value == ValueId::new(84_006).unwrap() && *value == right
                        )
                ));
            }
        }
    }
}

fn native_fixed_types() -> Vec<IntegerType> {
    [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).unwrap()))
        .collect()
}

fn boundary_pairs(scalar_type: IntegerType) -> [(IntegerValue, IntegerValue); 4] {
    let one = typed_value(scalar_type, 1);
    let two = typed_value(scalar_type, 2);
    let negative_one_or_maximum = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => scalar_type.maximum_value(),
    };
    [
        (scalar_type.minimum_value(), negative_one_or_maximum),
        (scalar_type.maximum_value(), two),
        (scalar_type.maximum_value(), one),
        (one, scalar_type.maximum_value()),
    ]
}

fn typed_value(scalar_type: IntegerType, value: u128) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value as i128),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    }
}
