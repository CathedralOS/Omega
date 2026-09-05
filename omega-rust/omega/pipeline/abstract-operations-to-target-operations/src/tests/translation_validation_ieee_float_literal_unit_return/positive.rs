use super::*;

#[test]
fn validates_exact_ieee_literal_unit_return_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for value in [
            IeeeFloatValue::Binary32(0x8000_0000),
            IeeeFloatValue::Binary32(0x7fc1_2345),
            IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc),
        ] {
            let mut source = base_plan();
            let AbstractOperation::IeeeFloatConstant {
                value: source_value,
                ..
            } = &mut source.functions[0].operations[0]
            else {
                unreachable!()
            };
            *source_value = value;
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIeeeFloatLiteralUnitReturn(
                    row,
                ),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("exact IEEE-literal Unit body must publish its validated family row")
            };
            assert_eq!(row.machine(), machine());
            assert_eq!(row.literal_operation(), literal_operation());
            assert_eq!(row.literal_result(), literal_result());
            assert_eq!(row.value(), value);
            assert_eq!(row.return_edge(), return_edge());

            let TargetOperation::UnitBody(body) = &target.functions[0].operation else {
                panic!("fixture must lower through the Unit body carrier")
            };
            assert_eq!(
                body.structural_types
                    .iter()
                    .map(|declaration| declaration.id)
                    .collect::<Vec<_>>(),
                vec![
                    StructuralTypeId::new(59_009).unwrap(),
                    StructuralTypeId::new(59_010).unwrap(),
                ]
            );
            assert_eq!(
                body.call_plan,
                evaluate_call_plan(
                    CallingPolicy::native_for_target(target_profile),
                    &CallSignature::default(),
                )
                .unwrap()
            );
        }
    }
}

#[test]
fn ieee_literal_classifier_is_disjoint_from_return_only_and_integer_literal_families() {
    let target_profile = NativeTarget::linux_x64();
    let mut return_only = base_plan();
    return_only.functions[0].operations.remove(0);
    let target = lower_to_target_operations(&return_only, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&return_only, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn(_)
        )
    ));

    let mut integer = base_plan();
    integer.functions[0].operations[0] = AbstractOperation::IntegerConstant {
        psi_operation: literal_operation(),
        result: literal_result(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        value: semantic_vocabulary::IntegerValue::Signed(-17),
    };
    let target = lower_to_target_operations(&integer, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&integer, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLiteralUnitReturn(_)
        )
    ));
}
