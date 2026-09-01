use super::*;

#[test]
fn validates_every_native_widening_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (source_type, target_type) in legal_native_widenings() {
            for value in boundary_values(source_type) {
                let source = base_plan(source_type, target_type, value);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerWidenImmediate(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("constant widening must publish only its exact immediate family")
                };
                let materialized = source_type.widen_value_to(target_type, value).unwrap();
                assert_eq!(row.machine(), machine());
                assert_eq!(row.constant_operation(), OperationId::new(64_003).unwrap());
                assert_eq!(row.widen_operation(), OperationId::new(64_005).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(64_007).unwrap());
                assert_eq!(row.constant_result(), ValueId::new(64_004).unwrap());
                assert_eq!(row.widened_result(), ValueId::new(64_006).unwrap());
                assert_eq!(row.source_type(), source_type);
                assert_eq!(row.target_type(), target_type);
                assert_eq!(row.source_value(), value);
                assert_eq!(row.materialized_value(), materialized);
                assert!(matches!(
                    target.functions[0].operation,
                    TargetOperation::ReturnIntegerImmediate {
                        source_value,
                        scalar_type,
                        value: target_value,
                        ..
                    } if source_value == ValueId::new(64_006).unwrap()
                        && scalar_type == target_type
                        && target_value == materialized
                ));
            }
        }
    }
}

#[test]
fn classifier_is_disjoint_from_plain_integer_immediate_and_parameter_widening() {
    let target_profile = NativeTarget::linux_x64();
    let mut plain = default_plan();
    plain.functions[0].operations.remove(1);
    let AbstractOperation::Return {
        value, scalar_type, ..
    } = &mut plain.functions[0].operations[1]
    else {
        unreachable!()
    };
    *value = ValueId::new(64_004).unwrap();
    *scalar_type = ScalarType::Integer(source_type());
    let AbstractFunctionResult::Scalar(result) = &mut plain.functions[0].result else {
        unreachable!()
    };
    result.scalar_type = ScalarType::Integer(source_type());
    let target = lower_to_target_operations(&plain, target_profile).unwrap();
    let receipt = validate_abstract_to_target_translation(&plain, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(_)
        )
    ));

    assert!(
        !crate::validation::straight_line_integer_widen_immediate::is_candidate(
            &super::super::parameter_translation_fixture::uniform_integer_widen_plan(
                source_type(),
                target_type(),
                1,
            )
            .functions[0]
        )
    );
}

fn legal_native_widenings() -> Vec<(IntegerType, IntegerType)> {
    let integer = |sign, bits| IntegerType::new(sign, bits).unwrap();
    let mut pairs = Vec::new();
    for source_sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for source_bits in [8, 16, 32] {
            for target_bits in [16, 32, 64] {
                if source_bits < target_bits {
                    pairs.push((
                        integer(source_sign, source_bits),
                        integer(source_sign, target_bits),
                    ));
                }
            }
        }
    }
    for source_bits in [8, 16, 32] {
        for target_bits in [16, 32, 64] {
            if source_bits < target_bits {
                pairs.push((
                    integer(IntegerSign::Unsigned, source_bits),
                    integer(IntegerSign::Signed, target_bits),
                ));
            }
        }
    }
    assert_eq!(pairs.len(), 18);
    pairs
}

fn boundary_values(integer: IntegerType) -> [IntegerValue; 2] {
    let bits = integer.bits();
    match integer.sign() {
        IntegerSign::Signed => [
            IntegerValue::Signed(-(1_i128 << (bits - 1))),
            IntegerValue::Signed((1_i128 << (bits - 1)) - 1),
        ],
        IntegerSign::Unsigned => [
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned((1_u128 << bits) - 1),
        ],
    }
}
