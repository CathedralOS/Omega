use super::*;

#[test]
fn validates_integer_bitwise_not_types_registers_and_stack_on_every_native_target() {
    let target_cases = [
        (
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
        ),
        (
            NativeTarget::windows_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::uefi_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
        (
            NativeTarget::macos_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ];
    let integers = [
        integer_type(IntegerSign::Signed, 8),
        integer_type(IntegerSign::Unsigned, 8),
        integer_type(IntegerSign::Signed, 16),
        integer_type(IntegerSign::Unsigned, 16),
        integer_type(IntegerSign::Signed, 32),
        integer_type(IntegerSign::Unsigned, 32),
        integer_type(IntegerSign::Signed, 64),
        integer_type(IntegerSign::Unsigned, 64),
    ];
    for (target_profile, register, stack) in target_cases {
        for scalar_type in integers {
            for (parameter_count, expected_location) in [(1, register), (9, stack)] {
                let source = uniform_integer_bitwise_not_plan(scalar_type, parameter_count);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotParameter(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("exact integer bitwise-not must publish its family receipt")
                };
                assert_eq!(row.machine(), source.entry);
                assert_eq!(
                    row.bitwise_not_operation(),
                    OperationId::new(4_200).unwrap()
                );
                assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
                assert_eq!(row.source_value(), ValueId::new(4_201).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(
                    row.operand_value(),
                    ValueId::new(3_100 + parameter_count as u64 - 1).unwrap()
                );
                assert_eq!(row.parameter_index(), parameter_count - 1);
                assert_eq!(row.location(), expected_location);
            }
        }
    }
}

#[test]
fn integer_bitwise_not_retains_exact_mixed_roster_operand_custody() {
    let integer = integer_type(IntegerSign::Signed, 32);
    let source = integer_bitwise_not_parameter_plan(
        &[
            ScalarType::Boolean,
            ScalarType::Integer(integer_type(IntegerSign::Unsigned, 8)),
            ScalarType::Integer(integer),
            ScalarType::Boolean,
        ],
        2,
    );
    let target_profile = NativeTarget::linux_arm64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    let AbstractToTargetFunctionTranslationDisposition::Validated(
        AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotParameter(row),
    ) = receipt.function_roster()[0].translation()
    else {
        panic!("mixed roster must retain the exact integer bitwise-not operand")
    };
    assert_eq!(row.scalar_type(), integer);
    assert_eq!(row.operand_value(), ValueId::new(3_102).unwrap());
    assert_eq!(row.parameter_index(), 2);
    assert_eq!(
        row.location(),
        ScalarParameterLocation::Register(MachineRegister::Aarch64X(2))
    );
}
