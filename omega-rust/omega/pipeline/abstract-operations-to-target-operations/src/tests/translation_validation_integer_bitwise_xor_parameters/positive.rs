use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
};

#[test]
fn validates_all_native_integer_bitwise_xor_types_registers_and_stack_on_every_target() {
    let targets = [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ];
    let integers = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| integer_type(sign, bits)))
        .collect::<Vec<_>>();
    for target_profile in targets {
        for scalar_type in &integers {
            for parameter_count in [2, 10] {
                let source = uniform_integer_bitwise_xor_plan(*scalar_type, parameter_count);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseXorParameters(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("integer bitwise-XOR must publish its family receipt")
                };
                assert_eq!(row.machine(), source.entry);
                assert_eq!(row.xor_operation(), OperationId::new(4_700).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
                assert_eq!(row.source_value(), ValueId::new(4_701).unwrap());
                assert_eq!(row.scalar_type(), *scalar_type);
                assert_eq!(row.left_parameter_index(), parameter_count - 2);
                assert_eq!(row.right_parameter_index(), parameter_count - 1);
                assert_ne!(row.left_location(), row.right_location());
            }
        }
    }
}

#[test]
fn integer_bitwise_xor_retains_order_identity_and_mixed_roster_custody() {
    let integer = integer_type(IntegerSign::Unsigned, 32);
    for (left, right) in [(2, 1), (1, 1)] {
        let source = integer_bitwise_xor_parameters_plan(
            &[
                ScalarType::Boolean,
                ScalarType::Integer(integer),
                ScalarType::Integer(integer),
            ],
            left,
            right,
        );
        let target_profile = NativeTarget::linux_x64();
        let target = lower_to_target_operations(&source, target_profile).unwrap();
        let receipt =
            validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseXorParameters(
                row,
            ),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("integer bitwise-XOR must retain ordered operand custody")
        };
        assert_eq!(row.left_parameter_index(), left);
        assert_eq!(row.right_parameter_index(), right);
        assert_eq!(row.left_value(), source.functions[0].parameters[left].value);
        assert_eq!(
            row.right_value(),
            source.functions[0].parameters[right].value
        );
    }
}
