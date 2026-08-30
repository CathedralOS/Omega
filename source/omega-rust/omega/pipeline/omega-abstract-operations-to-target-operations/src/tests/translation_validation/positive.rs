use super::*;

#[test]
fn validates_exact_integer_identity_on_every_native_target() {
    let cases = [
        (
            integer_type(IntegerSign::Unsigned, 8),
            IntegerValue::Unsigned(u8::MAX.into()),
        ),
        (
            integer_type(IntegerSign::Signed, 16),
            IntegerValue::Signed(i16::MIN.into()),
        ),
        (
            integer_type(IntegerSign::Unsigned, 64),
            IntegerValue::Unsigned(37),
        ),
    ];
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (scalar_type, value) in cases {
            let source = literal_plan(vec![literal_function(100, scalar_type, value)]);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            assert_eq!(receipt.psi(), source.psi);
            assert_eq!(receipt.target(), target_profile);
            assert_eq!(receipt.entry(), source.entry);
            assert_eq!(receipt.function_count(), 1);
            assert_eq!(
                receipt.function_roster()[0].machine(),
                source.functions[0].machine
            );
            assert_eq!(receipt.function_roster()[0].attachment(), None);
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("exact literal return must publish one validated family row")
            };
            assert_eq!(row.machine(), source.functions[0].machine);
            assert_eq!(row.constant_operation(), OperationId::new(103).unwrap());
            assert_eq!(row.return_edge(), EdgeId::new(106).unwrap());
            assert_eq!(row.source_value(), ValueId::new(104).unwrap());
            assert_eq!(row.scalar_type(), scalar_type);
            assert_eq!(row.value(), value);
        }
    }
}

#[test]
fn receipt_does_not_claim_unimplemented_parameterized_literal_family() {
    let mut source = base_plan();
    source.functions[0].parameters.push(AbstractParameter {
        value: ValueId::new(920).unwrap(),
        scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 64)),
    });
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert_eq!(
        receipt.function_roster()[0].translation(),
        &AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
    assert_eq!(receipt.function_count(), 1);
}

#[test]
fn receipt_retains_an_exact_attached_literal_function_roster() {
    let mut source = base_plan();
    let attachment = StructuralTypeId::new(921).unwrap();
    source.functions[0].attachment = Some(attachment);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert_eq!(receipt.function_roster()[0].attachment(), Some(attachment));
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate(_)
        )
    ));
}
