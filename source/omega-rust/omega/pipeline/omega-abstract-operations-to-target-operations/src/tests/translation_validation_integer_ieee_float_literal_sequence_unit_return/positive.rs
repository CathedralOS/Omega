use super::*;

#[test]
fn validates_both_mixed_orders_on_every_native_target() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for reverse_first_pair in [false, true] {
            let mut source = base_plan();
            if reverse_first_pair {
                source.functions[0].operations.swap(0, 1);
            }
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("mixed sequence must publish only its exact family")
            };
            assert_eq!(row.machine(), machine());
            assert_eq!(row.return_edge(), return_edge());
            assert_eq!(row.literals().len(), 3);
            assert_eq!(
                row.literals()
                    .iter()
                    .map(IntegerIeeeFloatLiteralSequenceMember::operation)
                    .collect::<Vec<_>>(),
                target.functions[0].provenance.operations
            );
            assert!(row.literals().iter().any(|literal| matches!(
                literal,
                IntegerIeeeFloatLiteralSequenceMember::Integer { .. }
            )));
            assert!(row.literals().iter().any(|literal| matches!(
                literal,
                IntegerIeeeFloatLiteralSequenceMember::IeeeFloat { .. }
            )));
        }
    }
}

#[test]
fn classifier_is_disjoint_from_both_homogeneous_sequence_families() {
    let target_profile = NativeTarget::linux_x64();

    let mut integers = base_plan();
    integers.functions[0].operations[1] = AbstractOperation::IntegerConstant {
        psi_operation: OperationId::new(63_005).unwrap(),
        result: ValueId::new(63_006).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        value: IntegerValue::Signed(-1),
    };
    let target = lower_to_target_operations(&integers, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&integers, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLiteralSequenceUnitReturn(_)
        )
    ));

    let mut floats = base_plan();
    for (index, value) in [
        IeeeFloatValue::Binary32(0x8000_0000),
        IeeeFloatValue::Binary64(0x7ff8_1234_5678_9abc),
    ]
    .into_iter()
    .enumerate()
    {
        let operation = match &floats.functions[0].operations[index * 2] {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                ..
            } => (*psi_operation, *result),
            _ => unreachable!(),
        };
        floats.functions[0].operations[index * 2] = AbstractOperation::IeeeFloatConstant {
            psi_operation: operation.0,
            result: operation.1,
            value,
        };
    }
    let target = lower_to_target_operations(&floats, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&floats, target_profile, &target).unwrap();
    assert!(matches!(
        receipt.function_roster()[0].translation(),
        AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIeeeFloatLiteralSequenceUnitReturn(_)
        )
    ));
}
