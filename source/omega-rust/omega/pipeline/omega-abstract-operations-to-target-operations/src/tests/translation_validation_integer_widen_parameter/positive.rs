use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
};

#[test]
fn validates_all_native_widenings_registers_and_stack_on_every_native_target() {
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
    for (source_type, target_type) in legal_native_widenings() {
        for (target_profile, register, stack) in target_cases {
            for (parameter_count, expected_location) in [(1, register), (9, stack)] {
                let source = uniform_integer_widen_plan(source_type, target_type, parameter_count);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerWidenParameter(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("exact integer widening must publish its family receipt")
                };
                assert_eq!(row.machine(), source.entry);
                assert_eq!(row.widen_operation(), OperationId::new(4_300).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
                assert_eq!(row.source_value(), ValueId::new(4_301).unwrap());
                assert_eq!(row.source_type(), source_type);
                assert_eq!(row.target_type(), target_type);
                assert_eq!(
                    row.operand_value(),
                    ValueId::new(3_100 + parameter_count as u64 - 1).unwrap()
                );
                assert_eq!(row.parameter_index(), parameter_count - 1);
                assert_eq!(row.location(), expected_location);
                assert!(matches!(
                    &target.functions[0].operation,
                    TargetOperation::ReturnIntegerExpression {
                        scalar_type,
                        expression: TargetIntegerExpression::IntegerWiden {
                            source_type: nested_source,
                            operand,
                            ..
                        },
                        ..
                    } if *scalar_type == target_type
                        && *nested_source == source_type
                        && matches!(
                            operand.as_ref(),
                            TargetIntegerExpression::Parameter { location, .. }
                                if *location == expected_location
                        )
                ));
            }
        }
    }
}

#[test]
fn integer_widen_retains_mixed_roster_operand_and_distinct_type_custody() {
    let source_type = integer_type(IntegerSign::Unsigned, 16);
    let target_type = integer_type(IntegerSign::Signed, 64);
    let source = integer_widen_parameter_plan(
        &[
            ScalarType::Boolean,
            ScalarType::Integer(integer_type(IntegerSign::Signed, 8)),
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
        AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerWidenParameter(row),
    ) = receipt.function_roster()[0].translation()
    else {
        panic!("mixed roster must retain exact integer-widen custody")
    };
    assert_eq!(row.source_type(), source_type);
    assert_eq!(row.target_type(), target_type);
    assert_eq!(row.operand_value(), ValueId::new(3_102).unwrap());
    assert_eq!(row.parameter_index(), 2);
    assert_eq!(
        row.location(),
        ScalarParameterLocation::Register(MachineRegister::Aarch64X(2))
    );
}
