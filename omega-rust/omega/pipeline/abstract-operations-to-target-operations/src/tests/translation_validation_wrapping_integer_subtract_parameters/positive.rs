use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
};

#[test]
fn validates_all_wrapping_integer_subtract_types_and_operand_placements_on_every_target() {
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
        .chain([IntegerType::address(64).unwrap()])
        .collect::<Vec<_>>();
    let operand_pairs = [
        (0, 1), // register/register
        (8, 9), // stack/stack
        (0, 9), // register/stack
        (9, 0), // reversed stack/register
        (7, 7), // identical operands
    ];

    for target_profile in targets {
        for scalar_type in &integers {
            for (left, right) in operand_pairs {
                let source = wrapping_integer_subtract_parameters_plan(
                    &[ScalarType::Integer(*scalar_type); 10],
                    left,
                    right,
                );
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerSubtractParameters(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("wrapping-integer-subtract must publish its family receipt")
                };
                assert_eq!(row.machine(), source.entry);
                assert_eq!(row.subtract_operation(), OperationId::new(4_900).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
                assert_eq!(row.source_value(), ValueId::new(4_901).unwrap());
                assert_eq!(row.scalar_type(), *scalar_type);
                assert_eq!(row.left_parameter_index(), left);
                assert_eq!(row.right_parameter_index(), right);
                assert_eq!(row.left_value(), source.functions[0].parameters[left].value);
                assert_eq!(
                    row.right_value(),
                    source.functions[0].parameters[right].value
                );
                assert_eq!(row.left_location() == row.right_location(), left == right);
            }
        }
    }
}

#[test]
fn wrapping_integer_subtract_retains_order_and_mixed_roster_custody() {
    let integer = integer_type(IntegerSign::Unsigned, 32);
    for (left, right) in [(2, 1), (1, 1)] {
        let source = wrapping_integer_subtract_parameters_plan(
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
            AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerSubtractParameters(
                row,
            ),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("wrapping-integer-subtract must retain ordered operand custody")
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
