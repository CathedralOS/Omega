use super::*;

#[test]
fn validates_ordered_truth_table_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (left_value, right_value) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let source = base_plan(left_value, right_value);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanEqualImmediate(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("constant Boolean equality must publish only its exact immediate family")
            };
            assert_eq!(row.machine(), machine());
            assert_eq!(
                row.left_constant_operation(),
                OperationId::new(69_003).unwrap()
            );
            assert_eq!(
                row.right_constant_operation(),
                OperationId::new(69_005).unwrap()
            );
            assert_eq!(row.equal_operation(), OperationId::new(69_007).unwrap());
            assert_eq!(row.return_edge(), EdgeId::new(69_009).unwrap());
            assert_eq!(row.left_constant_result(), ValueId::new(69_004).unwrap());
            assert_eq!(row.right_constant_result(), ValueId::new(69_006).unwrap());
            assert_eq!(row.equal_result(), ValueId::new(69_008).unwrap());
            assert_eq!(row.left_value(), left_value);
            assert_eq!(row.right_value(), right_value);
            assert_eq!(row.materialized_value(), left_value == right_value);
            assert!(matches!(
                target.functions[0].operation,
                TargetOperation::ReturnBooleanImmediate {
                    source_value,
                    value,
                    ..
                } if source_value == ValueId::new(69_008).unwrap()
                    && value == (left_value == right_value)
            ));
        }
    }
}

#[test]
fn classifier_is_disjoint_from_boolean_immediate_not_and_parameter_equality() {
    let target_profile = NativeTarget::linux_x64();
    let mut plain = default_plan();
    plain.functions[0].operations.drain(1..3);
    let AbstractOperation::Return { value, .. } = &mut plain.functions[0].operations[1] else {
        unreachable!()
    };
    *value = ValueId::new(69_004).unwrap();
    let target = lower_to_target_operations(&plain, target_profile).unwrap();
    let receipt = validate_abstract_to_target_translation(&plain, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate(_)
        )
    ));

    let mut boolean_not = default_plan();
    boolean_not.functions[0].operations.remove(1);
    boolean_not.functions[0].operations[1] = AbstractOperation::BooleanNot {
        psi_operation: OperationId::new(69_007).unwrap(),
        result: ValueId::new(69_008).unwrap(),
        operand: ValueId::new(69_004).unwrap(),
    };
    let target = lower_to_target_operations(&boolean_not, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&boolean_not, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanNotImmediate(_)
        )
    ));

    assert!(
        !crate::validation::straight_line_boolean_equal_immediate::is_candidate(
            &super::super::super::parameter_translation_fixture::uniform_boolean_equal_plan(2)
                .functions[0]
        )
    );
}
