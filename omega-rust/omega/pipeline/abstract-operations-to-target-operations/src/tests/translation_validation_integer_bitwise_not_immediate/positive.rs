use super::*;

#[test]
fn validates_every_native_integer_type_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_types() {
            for source_value in [scalar_type.minimum_value(), scalar_type.maximum_value()] {
                let source = base_plan(scalar_type, source_value);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotImmediate(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("constant bitwise-not must publish only its exact immediate family")
                };
                let materialized = scalar_type.bitwise_not(source_value).unwrap();
                assert_eq!(row.machine(), machine());
                assert_eq!(row.constant_operation(), OperationId::new(67_003).unwrap());
                assert_eq!(
                    row.bitwise_not_operation(),
                    OperationId::new(67_005).unwrap()
                );
                assert_eq!(row.return_edge(), EdgeId::new(67_007).unwrap());
                assert_eq!(row.constant_result(), ValueId::new(67_004).unwrap());
                assert_eq!(row.bitwise_not_result(), ValueId::new(67_006).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.source_value(), source_value);
                assert_eq!(row.materialized_value(), materialized);
                assert!(matches!(
                    target.functions[0].operation,
                    TargetOperation::ReturnIntegerImmediate {
                        source_value,
                        scalar_type: target_type,
                        value,
                        ..
                    } if source_value == ValueId::new(67_006).unwrap()
                        && target_type == scalar_type
                        && value == materialized
                ));
            }
        }
    }
}

#[test]
fn classifier_is_disjoint_from_plain_immediate_and_parameter_bitwise_not() {
    let target_profile = NativeTarget::linux_x64();
    let mut plain = default_plan();
    plain.functions[0].operations.remove(1);
    let AbstractOperation::Return { value, .. } = &mut plain.functions[0].operations[1] else {
        unreachable!()
    };
    *value = ValueId::new(67_004).unwrap();
    let target = lower_to_target_operations(&plain, target_profile).unwrap();
    let receipt = validate_abstract_to_target_translation(&plain, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(_)
        )
    ));

    assert!(
        !crate::validation::straight_line_integer_bitwise_not_immediate::is_candidate(
            &super::super::parameter_translation_fixture::uniform_integer_bitwise_not_plan(
                scalar_type(),
                1,
            )
            .functions[0]
        )
    );
}

fn native_types() -> Vec<IntegerType> {
    let mut types = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).unwrap()))
        .collect::<Vec<_>>();
    types.push(IntegerType::address(64).unwrap());
    types
}
