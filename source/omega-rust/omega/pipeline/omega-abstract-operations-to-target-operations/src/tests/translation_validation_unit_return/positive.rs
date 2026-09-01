use super::*;

#[test]
fn validates_parameterless_unit_return_on_every_native_target() {
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
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn(row),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("exact parameterless Unit return must publish one validated family row")
        };
        assert_eq!(row.machine(), source.entry);
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
                StructuralTypeId::new(53_009).unwrap(),
                StructuralTypeId::new(53_010).unwrap(),
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
fn nearby_nonempty_unit_body_remains_explicitly_uncovered() {
    let mut source = base_plan();
    source.functions[0].operations.insert(
        0,
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(53_060).unwrap(),
            result: ValueId::new(53_061).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
            value: IntegerValue::Signed(7),
        },
    );
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert_eq!(
        receipt.function_roster()[0].translation(),
        &AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
}
