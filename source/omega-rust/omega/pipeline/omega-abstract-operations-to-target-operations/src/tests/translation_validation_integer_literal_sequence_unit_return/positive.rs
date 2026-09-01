use super::*;

#[test]
fn validates_finite_typed_integer_sequences_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for literal_count in [2, 3] {
            let mut source = base_plan();
            source.functions[0]
                .operations
                .drain(literal_count..literals().len());
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLiteralSequenceUnitReturn(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("finite integer sequence must publish only its exact family")
            };
            assert_eq!(row.machine(), machine());
            assert_eq!(row.return_edge(), return_edge());
            assert_eq!(row.literals().len(), literal_count);
            for (member, (operation, result, scalar_type, value)) in
                row.literals().iter().zip(literals())
            {
                assert_eq!(member.operation(), OperationId::new(operation).unwrap());
                assert_eq!(member.result(), ValueId::new(result).unwrap());
                assert_eq!(member.scalar_type(), scalar_type);
                assert_eq!(member.value(), value);
            }
        }
    }
}

#[test]
fn classifier_is_disjoint_from_single_integer_and_unit_return_families() {
    let target_profile = NativeTarget::linux_x64();
    let mut single = base_plan();
    single.functions[0].operations.drain(1..literals().len());
    let target = lower_to_target_operations(&single, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&single, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLiteralUnitReturn(_)
        )
    ));

    single.functions[0].operations.remove(0);
    let target = lower_to_target_operations(&single, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&single, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn(_)
        )
    ));
}
