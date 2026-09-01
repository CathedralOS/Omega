use super::*;

#[test]
fn validates_native_boundary_pairs_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_types() {
            for (left_value, right_value) in boundary_pairs(scalar_type) {
                let source = base_plan(scalar_type, left_value, right_value);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerEqualImmediate(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("constant integer equality must publish only its exact immediate family")
                };
                assert_eq!(row.machine(), machine());
                assert_eq!(
                    row.left_constant_operation(),
                    OperationId::new(70_003).unwrap()
                );
                assert_eq!(
                    row.right_constant_operation(),
                    OperationId::new(70_005).unwrap()
                );
                assert_eq!(row.equal_operation(), OperationId::new(70_007).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(70_009).unwrap());
                assert_eq!(row.left_constant_result(), ValueId::new(70_004).unwrap());
                assert_eq!(row.right_constant_result(), ValueId::new(70_006).unwrap());
                assert_eq!(row.equal_result(), ValueId::new(70_008).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left_value(), left_value);
                assert_eq!(row.right_value(), right_value);
                assert_eq!(row.materialized_value(), left_value == right_value);
                assert!(matches!(
                    target.functions[0].operation,
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

#[test]
fn classifier_is_disjoint_from_boolean_immediates_and_parameter_integer_equality() {
    let target_profile = NativeTarget::linux_x64();
    let mut plain_boolean = default_plan();
    plain_boolean.functions[0].operations = vec![
        AbstractOperation::BooleanConstant {
            psi_operation: OperationId::new(70_003).unwrap(),
            result: ValueId::new(70_008).unwrap(),
            value: false,
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(70_009).unwrap(),
            result: ValueId::new(70_010).unwrap(),
            value: ValueId::new(70_008).unwrap(),
            scalar_type: ScalarType::Boolean,
            cleanup_actions: Vec::new(),
        },
    ];
    let target = lower_to_target_operations(&plain_boolean, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&plain_boolean, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate(_)
        )
    ));

    let mut boolean_equality = default_plan();
    boolean_equality.functions[0].operations[0] = AbstractOperation::BooleanConstant {
        psi_operation: OperationId::new(70_003).unwrap(),
        result: ValueId::new(70_004).unwrap(),
        value: true,
    };
    boolean_equality.functions[0].operations[1] = AbstractOperation::BooleanConstant {
        psi_operation: OperationId::new(70_005).unwrap(),
        result: ValueId::new(70_006).unwrap(),
        value: false,
    };
    boolean_equality.functions[0].operations[2] = AbstractOperation::BooleanEqual {
        psi_operation: OperationId::new(70_007).unwrap(),
        result: ValueId::new(70_008).unwrap(),
        left: ValueId::new(70_004).unwrap(),
        right: ValueId::new(70_006).unwrap(),
    };
    let target = lower_to_target_operations(&boolean_equality, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&boolean_equality, target_profile, &target)
            .unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanEqualImmediate(_)
        )
    ));

    assert!(
        !crate::validation::straight_line_integer_equal_immediate::is_candidate(
            &super::super::super::parameter_translation_fixture::uniform_integer_equal_plan(
                scalar_type(),
                2,
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
