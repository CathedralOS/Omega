use super::*;

#[test]
fn validates_native_bitwise_and_boundaries_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_types() {
            for (left_value, right_value) in boundary_pairs(scalar_type) {
                let expected = scalar_type.bitwise_and(left_value, right_value).unwrap();
                let source = base_plan(scalar_type, left_value, right_value);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseAndImmediate(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("constant integer bitwise-AND must publish only its exact immediate family")
                };
                assert_eq!(row.machine(), machine());
                assert_eq!(row.left_constant_operation(), OperationId::new(73_003).unwrap());
                assert_eq!(row.right_constant_operation(), OperationId::new(73_005).unwrap());
                assert_eq!(row.bitwise_and_operation(), OperationId::new(73_007).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(73_009).unwrap());
                assert_eq!(row.left_constant_result(), ValueId::new(73_004).unwrap());
                assert_eq!(row.right_constant_result(), ValueId::new(73_006).unwrap());
                assert_eq!(row.bitwise_and_result(), ValueId::new(73_008).unwrap());
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
fn classifier_is_disjoint_from_plain_not_or_xor_and_parameter_and() {
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

    let mut bitwise_not = default_plan();
    bitwise_not.functions[0].operations.remove(1);
    bitwise_not.functions[0].operations[1] = AbstractOperation::IntegerBitwiseNot {
        psi_operation: OperationId::new(73_007).unwrap(),
        result: ValueId::new(73_008).unwrap(),
        scalar_type: scalar_type(),
        operand: ValueId::new(73_004).unwrap(),
    };
    let target = lower_to_target_operations(&bitwise_not, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&bitwise_not, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotImmediate(_)
        )
    ));

    for replacement in [
        AbstractOperation::IntegerBitwiseOr {
            psi_operation: OperationId::new(73_007).unwrap(),
            result: ValueId::new(73_008).unwrap(),
            scalar_type: scalar_type(),
            left: ValueId::new(73_004).unwrap(),
            right: ValueId::new(73_006).unwrap(),
        },
        AbstractOperation::IntegerBitwiseXor {
            psi_operation: OperationId::new(73_007).unwrap(),
            result: ValueId::new(73_008).unwrap(),
            scalar_type: scalar_type(),
            left: ValueId::new(73_004).unwrap(),
            right: ValueId::new(73_006).unwrap(),
        },
    ] {
        let mut adjacent = default_plan();
        adjacent.functions[0].operations[2] = replacement;
        assert!(!crate::validation::straight_line_integer_bitwise_and_immediate::is_candidate(
            &adjacent.functions[0]
        ));
    }

    assert!(!crate::validation::straight_line_integer_bitwise_and_immediate::is_candidate(
        &super::super::parameter_translation_fixture::uniform_integer_bitwise_and_plan(
            scalar_type(),
            2,
        )
        .functions[0]
    ));
}

pub(super) fn native_types() -> Vec<IntegerType> {
    let mut types = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).unwrap()))
        .collect::<Vec<_>>();
    types.push(IntegerType::address(64).unwrap());
    types
}

pub(super) fn boundary_pairs(
    scalar_type: IntegerType,
) -> [(IntegerValue, IntegerValue); 4] {
    let minimum = scalar_type.minimum_value();
    let maximum = scalar_type.maximum_value();
    [
        (minimum, minimum),
        (minimum, maximum),
        (maximum, minimum),
        (maximum, maximum),
    ]
}
