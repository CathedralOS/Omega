use super::*;

#[test]
fn validates_finite_ieee_literal_sequences_on_every_native_target() {
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
                .drain(literal_count..LITERALS.len());
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIeeeFloatLiteralSequenceUnitReturn(
                    row,
                ),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("finite IEEE sequence must publish its exact validated family")
            };
            assert_eq!(row.machine(), machine());
            assert_eq!(row.return_edge(), return_edge());
            assert_eq!(row.literals().len(), literal_count);
            for (member, (operation, result, value)) in row.literals().iter().zip(LITERALS) {
                assert_eq!(member.operation(), OperationId::new(*operation).unwrap());
                assert_eq!(member.result(), ValueId::new(*result).unwrap());
                assert_eq!(member.value(), *value);
            }

            let TargetOperation::UnitBody(body) = &target.functions[0].operation else {
                panic!("IEEE sequence must lower through the Unit-body carrier")
            };
            assert_eq!(body.operations.len(), literal_count + 1);
            assert_eq!(
                body.structural_types
                    .iter()
                    .map(|declaration| declaration.id)
                    .collect::<Vec<_>>(),
                vec![
                    StructuralTypeId::new(60_010).unwrap(),
                    StructuralTypeId::new(60_011).unwrap(),
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
fn sequence_classifier_is_disjoint_from_single_literal_and_return_only_families() {
    let target_profile = NativeTarget::linux_x64();
    let mut single = base_plan();
    single.functions[0].operations.drain(1..LITERALS.len());
    let target = lower_to_target_operations(&single, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&single, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIeeeFloatLiteralUnitReturn(_)
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
