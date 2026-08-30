use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
};

#[test]
fn validates_all_native_exact_casts_registers_and_stack_on_every_native_target() {
    let target_cases = [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi, 16),
        (NativeTarget::windows_x64(), MachineRegister::X86Rcx, 64),
        (NativeTarget::uefi_x64(), MachineRegister::X86Rcx, 64),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0), 0),
        (NativeTarget::macos_arm64(), MachineRegister::Aarch64X(0), 0),
    ];
    for (source_type, target_type) in legal_native_exact_casts() {
        for (target_profile, register, stack_offset) in target_cases {
            for (parameter_count, expected_location) in [
                (1, ScalarParameterLocation::Register(register)),
                (
                    9,
                    ScalarParameterLocation::IncomingStack {
                        byte_offset: stack_offset,
                    },
                ),
            ] {
                let source =
                    uniform_integer_exact_cast_plan(source_type, target_type, parameter_count);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerExactCastParameter(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("exact integer cast must publish its family receipt")
                };
                assert_eq!(row.machine(), source.entry);
                assert_eq!(row.cast_operation(), OperationId::new(4_400).unwrap());
                assert_eq!(row.obligation(), ObligationId::new(4_402).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
                assert_eq!(row.source_value(), ValueId::new(4_401).unwrap());
                assert_eq!(row.source_type(), source_type);
                assert_eq!(row.target_type(), target_type);
                assert_eq!(row.parameter_index(), parameter_count - 1);
                assert_eq!(row.location(), expected_location);
            }
        }
    }
}

#[test]
fn exact_cast_retains_mixed_roster_operand_and_obligation_custody() {
    let source_type = integer_type(IntegerSign::Unsigned, 64);
    let target_type = integer_type(IntegerSign::Signed, 8);
    let source = integer_exact_cast_parameter_plan(
        &[
            ScalarType::Boolean,
            ScalarType::Integer(integer_type(IntegerSign::Signed, 16)),
            ScalarType::Integer(source_type),
            ScalarType::Boolean,
        ],
        2,
        target_type,
    );
    let target_profile = NativeTarget::linux_arm64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    let AbstractToTargetFunctionTranslationDisposition::Validated(
        AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerExactCastParameter(row),
    ) = receipt.function_roster()[0].translation()
    else {
        panic!("mixed roster must retain exact-cast custody")
    };
    assert_eq!(row.obligation(), ObligationId::new(4_402).unwrap());
    assert_eq!(row.operand_value(), ValueId::new(3_102).unwrap());
    assert_eq!(row.parameter_index(), 2);
    assert_eq!(
        row.location(),
        ScalarParameterLocation::Register(MachineRegister::Aarch64X(2))
    );
}
