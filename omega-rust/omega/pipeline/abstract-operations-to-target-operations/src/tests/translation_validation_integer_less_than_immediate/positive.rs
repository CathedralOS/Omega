use super::*;

#[test]
fn validates_native_ordering_boundaries_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for scalar_type in native_types() {
            for (left_value, right_value, expected) in boundary_cases(scalar_type) {
                let source = base_plan(scalar_type, left_value, right_value);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLessThanImmediate(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("constant integer ordering must publish only its exact immediate family")
                };
                assert_eq!(row.machine(), machine());
                assert_eq!(
                    row.left_constant_operation(),
                    OperationId::new(71_003).unwrap()
                );
                assert_eq!(
                    row.right_constant_operation(),
                    OperationId::new(71_005).unwrap()
                );
                assert_eq!(row.less_than_operation(), OperationId::new(71_007).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(71_009).unwrap());
                assert_eq!(row.left_constant_result(), ValueId::new(71_004).unwrap());
                assert_eq!(row.right_constant_result(), ValueId::new(71_006).unwrap());
                assert_eq!(row.less_than_result(), ValueId::new(71_008).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left_value(), left_value);
                assert_eq!(row.right_value(), right_value);
                assert_eq!(row.materialized_value(), expected);
                assert!(matches!(
                    target.functions[0].operation,
                    TargetOperation::ReturnBooleanImmediate {
                        source_value,
                        value,
                        ..
                    } if source_value == ValueId::new(71_008).unwrap() && value == expected
                ));
            }
        }
    }
}

#[test]
fn classifier_is_disjoint_from_integer_equality_and_parameter_ordering() {
    let target_profile = NativeTarget::linux_x64();
    let mut equality = default_plan();
    equality.functions[0].operations[2] = AbstractOperation::IntegerEqual {
        psi_operation: OperationId::new(71_007).unwrap(),
        result: ValueId::new(71_008).unwrap(),
        left: ValueId::new(71_004).unwrap(),
        right: ValueId::new(71_006).unwrap(),
    };
    let target = lower_to_target_operations(&equality, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&equality, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerEqualImmediate(_)
        )
    ));

    assert!(
        !crate::validation::straight_line_integer_less_than_immediate::is_candidate(
            &super::super::super::parameter_translation_fixture::uniform_integer_less_than_plan(
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

pub(super) fn boundary_cases(scalar_type: IntegerType) -> [(IntegerValue, IntegerValue, bool); 4] {
    let minimum = scalar_type.minimum_value();
    let maximum = scalar_type.maximum_value();
    [
        (minimum, minimum, false),
        (minimum, maximum, true),
        (maximum, minimum, false),
        (maximum, maximum, false),
    ]
}
