use super::*;

#[test]
fn validates_native_ordered_boundaries_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_fixed_types() {
            for (left, right) in boundary_pairs(scalar_type) {
                let expected = scalar_type.wrapping_rem(left, right).unwrap();
                let source = base_plan(scalar_type, left, right);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerRemainderImmediateOperands(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("constant-operand wrapping remainder must publish only its exact family")
                };
                assert_eq!(row.machine(), machine());
                assert_eq!(
                    row.left_constant_operation(),
                    OperationId::new(84_003).unwrap()
                );
                assert_eq!(
                    row.right_constant_operation(),
                    OperationId::new(84_005).unwrap()
                );
                assert_eq!(row.remainder_operation(), OperationId::new(84_007).unwrap());
                assert_eq!(row.obligation(), ObligationId::new(84_011).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(84_009).unwrap());
                assert_eq!(row.left_constant_result(), ValueId::new(84_004).unwrap());
                assert_eq!(row.right_constant_result(), ValueId::new(84_006).unwrap());
                assert_eq!(row.remainder_result(), ValueId::new(84_008).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left(), left);
                assert_eq!(row.right(), right);
                assert_eq!(row.remainder(), expected);
                assert!(matches!(
                    &target.functions[0].operation,
                    TargetOperation::ReturnIntegerExpression {
                        source_value,
                        scalar_type: target_type,
                        expression: TargetIntegerExpression::WrappingRemainder {
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

#[test]
fn classifier_is_disjoint_from_plain_parameter_and_other_remainder_policies() {
    let mut plain = default_plan();
    plain.functions[0].operations.drain(1..3);
    let AbstractOperation::Return { value, .. } = &mut plain.functions[0].operations[1] else {
        unreachable!()
    };
    *value = ValueId::new(84_004).unwrap();
    assert!(
        !crate::validation::straight_line_wrapping_integer_remainder_immediate_operands::is_candidate(
            &plain.functions[0]
        )
    );

    let scalar_type = scalar_type();
    for replacement in [
        AbstractOperation::WrappingIntegerDivide {
            psi_operation: OperationId::new(84_007).unwrap(),
            obligation: ObligationId::new(84_011).unwrap(),
            result: ValueId::new(84_008).unwrap(),
            scalar_type,
            left: ValueId::new(84_004).unwrap(),
            right: ValueId::new(84_006).unwrap(),
        },
        AbstractOperation::ExactIntegerDivide {
            psi_operation: OperationId::new(84_007).unwrap(),
            obligation: ObligationId::new(84_011).unwrap(),
            result: ValueId::new(84_008).unwrap(),
            scalar_type,
            left: ValueId::new(84_004).unwrap(),
            right: ValueId::new(84_006).unwrap(),
        },
        AbstractOperation::SaturatingIntegerDivide {
            psi_operation: OperationId::new(84_007).unwrap(),
            obligation: ObligationId::new(84_011).unwrap(),
            result: ValueId::new(84_008).unwrap(),
            scalar_type,
            left: ValueId::new(84_004).unwrap(),
            right: ValueId::new(84_006).unwrap(),
        },
        AbstractOperation::ExactIntegerRemainder {
            psi_operation: OperationId::new(84_007).unwrap(),
            obligation: ObligationId::new(84_011).unwrap(),
            result: ValueId::new(84_008).unwrap(),
            scalar_type,
            left: ValueId::new(84_004).unwrap(),
            right: ValueId::new(84_006).unwrap(),
        },
        AbstractOperation::SaturatingIntegerRemainder {
            psi_operation: OperationId::new(84_007).unwrap(),
            obligation: ObligationId::new(84_011).unwrap(),
            result: ValueId::new(84_008).unwrap(),
            scalar_type,
            left: ValueId::new(84_004).unwrap(),
            right: ValueId::new(84_006).unwrap(),
        },
    ] {
        let mut adjacent = default_plan();
        adjacent.functions[0].operations[2] = replacement;
        assert!(!crate::validation::straight_line_wrapping_integer_remainder_immediate_operands::is_candidate(
            &adjacent.functions[0]
        ));
    }

    let parameter =
        super::super::parameter_translation_fixture::wrapping_integer_remainder_parameters_plan(
            &[
                ScalarType::Integer(scalar_type),
                ScalarType::Integer(scalar_type),
            ],
            0,
            1,
        );
    assert!(
        !crate::validation::straight_line_wrapping_integer_remainder_immediate_operands::is_candidate(
            &parameter.functions[0]
        )
    );
}

pub(super) fn native_fixed_types() -> Vec<IntegerType> {
    [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).unwrap()))
        .collect()
}

pub(super) fn boundary_pairs(scalar_type: IntegerType) -> [(IntegerValue, IntegerValue); 4] {
    let one = typed_value(scalar_type, 1);
    let two = typed_value(scalar_type, 2);
    let negative_one_or_maximum = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => scalar_type.maximum_value(),
    };
    let negative_five_or_maximum = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-5),
        IntegerSign::Unsigned => scalar_type.maximum_value(),
    };
    [
        (scalar_type.minimum_value(), negative_one_or_maximum),
        (negative_five_or_maximum, two),
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
