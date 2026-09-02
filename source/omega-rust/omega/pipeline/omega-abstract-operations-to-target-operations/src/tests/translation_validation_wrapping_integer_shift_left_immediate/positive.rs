use super::*;

#[test]
fn validates_independent_native_type_and_count_boundaries_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for value_type in native_types() {
            for count_type in native_types() {
                for (value, count) in boundary_pairs(value_type, count_type) {
                    let expected = value_type
                        .wrapping_shift_left(value, count_type, count)
                        .unwrap();
                    let source = base_plan(value_type, count_type, value, count);
                    let target = lower_to_target_operations(&source, target_profile).unwrap();
                    let receipt =
                        validate_abstract_to_target_translation(&source, target_profile, &target)
                            .unwrap();
                    let AbstractToTargetFunctionTranslationDisposition::Validated(
                        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftLeftImmediate(row),
                    ) = receipt.function_roster()[0].translation()
                    else {
                        panic!("constant wrapping shift-left must publish only its exact immediate family")
                    };
                    assert_eq!(row.machine(), machine());
                    assert_eq!(
                        row.value_constant_operation(),
                        OperationId::new(83_003).unwrap()
                    );
                    assert_eq!(
                        row.count_constant_operation(),
                        OperationId::new(83_005).unwrap()
                    );
                    assert_eq!(
                        row.wrapping_shift_operation(),
                        OperationId::new(83_007).unwrap()
                    );
                    assert_eq!(row.return_edge(), EdgeId::new(83_009).unwrap());
                    assert_eq!(row.value_constant_result(), ValueId::new(83_004).unwrap());
                    assert_eq!(row.count_constant_result(), ValueId::new(83_006).unwrap());
                    assert_eq!(row.wrapping_shift_result(), ValueId::new(83_008).unwrap());
                    assert_eq!(row.value_type(), value_type);
                    assert_eq!(row.count_type(), count_type);
                    assert_eq!(row.value(), value);
                    assert_eq!(row.count(), count);
                    assert_eq!(row.materialized_value(), expected);
                    assert!(matches!(
                        target.functions[0].operation,
                        TargetOperation::ReturnIntegerImmediate {
                            source_value,
                            scalar_type,
                            value: target_value,
                            ..
                        } if source_value == ValueId::new(83_008).unwrap()
                            && scalar_type == value_type
                            && target_value == expected
                    ));
                }
            }
        }
    }
}

#[test]
fn classifier_is_disjoint_from_plain_parameter_and_other_shift_policies() {
    let target_profile = NativeTarget::linux_x64();
    let mut plain = default_plan();
    plain.functions[0].operations.drain(1..3);
    let AbstractOperation::Return { value, .. } = &mut plain.functions[0].operations[1] else {
        unreachable!()
    };
    *value = ValueId::new(83_004).unwrap();
    let target = lower_to_target_operations(&plain, target_profile).unwrap();
    let receipt = validate_abstract_to_target_translation(&plain, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(_)
        )
    ));

    for replacement in [
        AbstractOperation::WrappingIntegerShiftRight {
            psi_operation: OperationId::new(83_007).unwrap(),
            result: ValueId::new(83_008).unwrap(),
            value_type: value_type(),
            count_type: count_type(),
            value: ValueId::new(83_004).unwrap(),
            count: ValueId::new(83_006).unwrap(),
        },
        AbstractOperation::ExactIntegerShiftLeft {
            psi_operation: OperationId::new(83_007).unwrap(),
            obligation: ObligationId::new(83_011).unwrap(),
            result: ValueId::new(83_008).unwrap(),
            value_type: value_type(),
            count_type: count_type(),
            value: ValueId::new(83_004).unwrap(),
            count: ValueId::new(83_006).unwrap(),
        },
        AbstractOperation::ExactIntegerShiftRight {
            psi_operation: OperationId::new(83_007).unwrap(),
            obligation: ObligationId::new(83_011).unwrap(),
            result: ValueId::new(83_008).unwrap(),
            value_type: value_type(),
            count_type: count_type(),
            value: ValueId::new(83_004).unwrap(),
            count: ValueId::new(83_006).unwrap(),
        },
    ] {
        let mut adjacent = default_plan();
        adjacent.functions[0].operations[2] = replacement;
        assert!(
            !crate::validation::straight_line_wrapping_integer_shift_left_immediate::is_candidate(
                &adjacent.functions[0]
            )
        );
    }

    let parameter =
        super::super::parameter_translation_fixture::wrapping_integer_shift_left_parameters_plan(
            &[
                ScalarType::Integer(value_type()),
                ScalarType::Integer(count_type()),
            ],
            0,
            1,
        );
    assert!(
        !crate::validation::straight_line_wrapping_integer_shift_left_immediate::is_candidate(
            &parameter.functions[0]
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

pub(super) fn boundary_pairs(
    value_type: IntegerType,
    count_type: IntegerType,
) -> [(IntegerValue, IntegerValue); 4] {
    let zero = count_value(count_type, 0);
    let width = count_value(count_type, u128::from(value_type.bits()));
    let width_plus_one = count_value(count_type, u128::from(value_type.bits()) + 1);
    let minus_one_or_maximum = match count_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => count_type.maximum_value(),
    };
    let one = match value_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    [
        (value_type.minimum_value(), zero),
        (value_type.maximum_value(), width),
        (value_type.maximum_value(), width_plus_one),
        (one, minus_one_or_maximum),
    ]
}

fn count_value(count_type: IntegerType, value: u128) -> IntegerValue {
    match count_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value as i128),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    }
}
