use super::*;

#[test]
fn validates_integer_literal_unit_return_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = base_plan();
        let target = lower_to_target_operations(&source, target_profile).unwrap();
        let receipt =
            validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
        assert_eq!(
            receipt.function_roster()[0]
                .translation()
                .validated()
                .unwrap()
                .family(),
            AbstractToTargetTranslationFamily::StraightLineIntegerLiteralUnitReturn
        );
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLiteralUnitReturn(row),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("exact integer-literal Unit body must publish its validated family row")
        };
        assert_eq!(row.machine(), machine());
        assert_eq!(row.literal_operation(), literal_operation());
        assert_eq!(row.literal_result(), literal_result());
        assert_eq!(row.scalar_type(), literal_type());
        assert_eq!(row.value(), literal_value());
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
                StructuralTypeId::new(58_009).unwrap(),
                StructuralTypeId::new(58_010).unwrap(),
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

#[test]
fn integer_literal_classifier_is_disjoint_from_return_only_family() {
    let target_profile = NativeTarget::linux_x64();
    let mut source = base_plan();
    source.functions[0].operations.remove(0);
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn(_)
        )
    ));
}
