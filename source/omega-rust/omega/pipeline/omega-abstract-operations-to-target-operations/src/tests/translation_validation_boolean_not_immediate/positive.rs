use super::*;

#[test]
fn validates_both_boolean_values_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for source_value in [false, true] {
            let source = base_plan(source_value);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanNotImmediate(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("constant Boolean-not must publish only its exact immediate family")
            };
            assert_eq!(row.machine(), machine());
            assert_eq!(row.constant_operation(), OperationId::new(68_003).unwrap());
            assert_eq!(
                row.boolean_not_operation(),
                OperationId::new(68_005).unwrap()
            );
            assert_eq!(row.return_edge(), EdgeId::new(68_007).unwrap());
            assert_eq!(row.constant_result(), ValueId::new(68_004).unwrap());
            assert_eq!(row.boolean_not_result(), ValueId::new(68_006).unwrap());
            assert_eq!(row.source_value(), source_value);
            assert_eq!(row.materialized_value(), !source_value);
            assert!(matches!(
                target.functions[0].operation,
                TargetOperation::ReturnBooleanImmediate {
                    source_value: result,
                    value,
                    ..
                } if result == ValueId::new(68_006).unwrap() && value != source_value
            ));
        }
    }
}

#[test]
fn classifier_is_disjoint_from_plain_immediate_and_parameter_boolean_not() {
    let target_profile = NativeTarget::linux_x64();
    let mut plain = default_plan();
    plain.functions[0].operations.remove(1);
    let AbstractOperation::Return { value, .. } = &mut plain.functions[0].operations[1] else {
        unreachable!()
    };
    *value = ValueId::new(68_004).unwrap();
    let target = lower_to_target_operations(&plain, target_profile).unwrap();
    let receipt = validate_abstract_to_target_translation(&plain, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate(_)
        )
    ));

    assert!(
        !crate::validation::straight_line_boolean_not_immediate::is_candidate(
            &super::super::super::parameter_translation_fixture::uniform_boolean_not_plan(1)
                .functions[0]
        )
    );
}
