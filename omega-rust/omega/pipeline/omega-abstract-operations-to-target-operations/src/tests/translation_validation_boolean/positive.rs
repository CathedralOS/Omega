use super::*;

#[test]
fn validates_exact_boolean_identity_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for value in [false, true] {
            let source = boolean_plan(value);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("exact Boolean return must publish one validated family row")
            };
            assert_eq!(row.machine(), MachineId::new(1_001).unwrap());
            assert_eq!(row.constant_operation(), OperationId::new(1_003).unwrap());
            assert_eq!(row.return_edge(), EdgeId::new(1_006).unwrap());
            assert_eq!(row.source_value(), ValueId::new(1_004).unwrap());
            assert_eq!(row.value(), value);
        }
    }
}

#[test]
fn receipt_does_not_claim_unimplemented_parameterized_boolean_family() {
    let mut source = boolean_plan(true);
    source.functions[0].parameters.push(AbstractParameter {
        value: ValueId::new(1_100).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    assert_eq!(
        receipt.function_roster()[0].translation(),
        &AbstractToTargetFunctionTranslationDisposition::Uncovered
    );
}
