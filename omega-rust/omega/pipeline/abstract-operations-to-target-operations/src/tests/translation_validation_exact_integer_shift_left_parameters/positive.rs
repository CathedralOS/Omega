use super::exact_integer_shift_left_parameters_plan;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    lower_to_target_operations, validate_abstract_to_target_translation,
};
use semantic_vocabulary::{EdgeId, IntegerSign, IntegerType, OperationId, ScalarType, ValueId};
use target::NativeTarget;

fn types() -> Vec<IntegerType> {
    [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| super::integer_type(sign, bits)))
        .collect()
}

#[test]
fn validates_independent_value_and_count_types_and_abi_placements_on_every_target() {
    let targets = [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ];
    let placements = [(0, 1), (8, 9), (0, 9), (9, 0)];

    for target_profile in targets {
        for value_type in types() {
            for count_type in types() {
                for (value_index, count_index) in placements {
                    let mut parameter_types = vec![ScalarType::Boolean; 10];
                    parameter_types[value_index] = ScalarType::Integer(value_type);
                    parameter_types[count_index] = ScalarType::Integer(count_type);
                    let source = exact_integer_shift_left_parameters_plan(
                        &parameter_types,
                        value_index,
                        count_index,
                    );
                    let target = lower_to_target_operations(&source, target_profile).unwrap();
                    let receipt =
                        validate_abstract_to_target_translation(&source, target_profile, &target)
                            .unwrap();
                    let AbstractToTargetFunctionTranslationDisposition::Validated(
                        AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerShiftLeftParameters(row),
                    ) = receipt.function_roster()[0].translation()
                    else {
                        panic!("exact shift-left must publish its independent receipt")
                    };
                    assert_eq!(row.machine(), source.entry);
                    assert_eq!(row.shift_operation(), OperationId::new(7_200).unwrap());
                    assert_eq!(
                        row.obligation(),
                        semantic_vocabulary::ObligationId::new(7_202).unwrap()
                    );
                    assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
                    assert_eq!(row.source_value(), ValueId::new(7_201).unwrap());
                    assert_eq!(row.value_type(), value_type);
                    assert_eq!(row.count_type(), count_type);
                    assert_eq!(row.value_parameter_index(), value_index);
                    assert_eq!(row.count_parameter_index(), count_index);
                    assert_eq!(
                        row.value(),
                        source.functions[0].parameters[value_index].value
                    );
                    assert_eq!(
                        row.count(),
                        source.functions[0].parameters[count_index].value
                    );
                }
            }
        }
    }
}

#[test]
fn exact_shift_left_retains_reversed_same_operand_and_mixed_roster_custody() {
    let integer_types = types();
    for scalar_type in integer_types {
        let parameter_types = [
            ScalarType::Boolean,
            ScalarType::Integer(scalar_type),
            ScalarType::Integer(scalar_type),
        ];
        for (value_index, count_index) in [(2, 1), (1, 1)] {
            let source = exact_integer_shift_left_parameters_plan(
                &parameter_types,
                value_index,
                count_index,
            );
            let target_profile = NativeTarget::linux_x64();
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerShiftLeftParameters(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("exact shift-left must retain value/count order")
            };
            assert_eq!(row.value_parameter_index(), value_index);
            assert_eq!(row.count_parameter_index(), count_index);
            assert_eq!(
                row.value_location() == row.count_location(),
                value_index == count_index
            );
        }
    }
}
