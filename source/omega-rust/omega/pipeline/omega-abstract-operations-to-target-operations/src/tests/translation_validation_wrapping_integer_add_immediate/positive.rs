use super::*;

#[test]
fn validates_native_wrapping_add_boundaries_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_types() {
            for (left_value, right_value) in boundary_pairs(scalar_type) {
                let expected = scalar_type.wrapping_add(left_value, right_value).unwrap();
                let source = base_plan(scalar_type, left_value, right_value);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerAddImmediate(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("constant integer wrapping integer addition must publish only its exact immediate family")
                };
                assert_eq!(row.machine(), machine());
                assert_eq!(
                    row.left_constant_operation(),
                    OperationId::new(73_003).unwrap()
                );
                assert_eq!(
                    row.right_constant_operation(),
                    OperationId::new(73_005).unwrap()
                );
                assert_eq!(
                    row.wrapping_add_operation(),
                    OperationId::new(73_007).unwrap()
                );
                assert_eq!(row.return_edge(), EdgeId::new(73_009).unwrap());
                assert_eq!(row.left_constant_result(), ValueId::new(73_004).unwrap());
                assert_eq!(row.right_constant_result(), ValueId::new(73_006).unwrap());
                assert_eq!(row.wrapping_add_result(), ValueId::new(73_008).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left_value(), left_value);
                assert_eq!(row.right_value(), right_value);
                assert_eq!(row.materialized_value(), expected);
                assert!(matches!(
                    target.functions[0].operation,
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

#[test]
fn classifier_is_disjoint_from_plain_adjacent_arithmetic_and_parameter_add() {
    let target_profile = NativeTarget::linux_x64();
    let mut plain = default_plan();
    plain.functions[0].operations.drain(1..3);
    let AbstractOperation::Return { value, .. } = &mut plain.functions[0].operations[1] else {
        unreachable!()
    };
    *value = ValueId::new(73_004).unwrap();
    let target = lower_to_target_operations(&plain, target_profile).unwrap();
    let receipt = validate_abstract_to_target_translation(&plain, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(_)
        )
    ));

    for replacement in [
        AbstractOperation::ExactIntegerAdd {
            psi_operation: OperationId::new(73_007).unwrap(),
            result: ValueId::new(73_008).unwrap(),
            scalar_type: scalar_type(),
            left: ValueId::new(73_004).unwrap(),
            right: ValueId::new(73_006).unwrap(),
            obligation: ObligationId::new(73_011).unwrap(),
        },
        AbstractOperation::SaturatingIntegerAdd {
            psi_operation: OperationId::new(73_007).unwrap(),
            result: ValueId::new(73_008).unwrap(),
            scalar_type: scalar_type(),
            left: ValueId::new(73_004).unwrap(),
            right: ValueId::new(73_006).unwrap(),
        },
        AbstractOperation::WrappingIntegerSubtract {
            psi_operation: OperationId::new(73_007).unwrap(),
            result: ValueId::new(73_008).unwrap(),
            scalar_type: scalar_type(),
            left: ValueId::new(73_004).unwrap(),
            right: ValueId::new(73_006).unwrap(),
        },
        AbstractOperation::WrappingIntegerMultiply {
            psi_operation: OperationId::new(73_007).unwrap(),
            result: ValueId::new(73_008).unwrap(),
            scalar_type: scalar_type(),
            left: ValueId::new(73_004).unwrap(),
            right: ValueId::new(73_006).unwrap(),
        },
    ] {
        let mut adjacent = default_plan();
        adjacent.functions[0].operations[2] = replacement;
        assert!(
            !crate::validation::straight_line_wrapping_integer_add_immediate::is_candidate(
                &adjacent.functions[0]
            )
        );
    }

    assert!(
        !crate::validation::straight_line_wrapping_integer_add_immediate::is_candidate(
            &super::super::parameter_translation_fixture::uniform_wrapping_integer_add_plan(
                scalar_type(),
                2,
            )
            .functions[0]
        )
    );
}

pub(super) fn native_types() -> Vec<IntegerType> {
    let mut types = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).unwrap()))
        .collect::<Vec<_>>();
    types.push(IntegerType::address(64).unwrap());
    types
}

pub(super) fn boundary_pairs(scalar_type: IntegerType) -> [(IntegerValue, IntegerValue); 4] {
    let minimum = scalar_type.minimum_value();
    let maximum = scalar_type.maximum_value();
    let one = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    [
        (minimum, minimum),
        (maximum, one),
        (one, maximum),
        (maximum, maximum),
    ]
}
