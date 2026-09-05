//! Public optimized-target custody for constant wrapping integer shift-left materialization.

use super::super::*;

#[test]
fn optimized_target_lowering_retains_constant_wrapping_integer_shift_left_immediate_custody() {
    for target_profile in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for value_type in native_types() {
            for count_type in native_types() {
                for (value, count) in boundary_pairs(value_type, count_type) {
                    let expected = value_type
                        .wrapping_shift_left(value, count_type, count)
                        .unwrap();
                    let (semantic, proof) = wrapping_integer_shift_left_immediate_return_artifact(
                        value_type, count_type, value, count,
                    );
                    let optimized = optimize_artifact_sections(
                        &semantic,
                        &proof,
                        &AdmissionProfile::default(),
                        request(
                            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
                        ),
                    )
                    .unwrap();
                    let target =
                        lower_optimized_to_target_operations(optimized, target_profile).unwrap();
                    let AbstractToTargetFunctionTranslationDisposition::Validated(
                        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftLeftImmediate(row),
                    ) = target.translation_validation().function_roster()[0].translation()
                    else {
                        panic!("optimized wrapping shift-left must retain its exact immediate family")
                    };
                    assert_eq!(
                        row.value_constant_operation(),
                        OperationId::new(83_003).unwrap()
                    );
                    assert_eq!(
                        row.count_constant_operation(),
                        OperationId::new(83_005).unwrap()
                    );
                    assert_eq!(
                        row.wrapping_shift_operation(),
                        OperationId::new(83_007).unwrap()
                    );
                    assert_eq!(row.value_constant_result(), ValueId::new(83_004).unwrap());
                    assert_eq!(row.count_constant_result(), ValueId::new(83_006).unwrap());
                    assert_eq!(row.wrapping_shift_result(), ValueId::new(83_008).unwrap());
                    assert_eq!(row.value_type(), value_type);
                    assert_eq!(row.count_type(), count_type);
                    assert_eq!(row.value(), value);
                    assert_eq!(row.count(), count);
                    assert_eq!(row.materialized_value(), expected);
                    assert!(matches!(
                        target.target_operations().functions[0].operation,
                        TargetOperation::ReturnIntegerImmediate {
                            source_value,
                            scalar_type,
                            value: target_value,
                            ..
                        } if source_value == ValueId::new(83_008).unwrap()
                            && scalar_type == value_type
                            && target_value == expected
                    ));
                }
            }
        }
    }
}

fn native_types() -> Vec<IntegerType> {
    let mut types = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).unwrap()))
        .collect::<Vec<_>>();
    types.push(IntegerType::address(64).unwrap());
    types
}

fn boundary_pairs(
    value_type: IntegerType,
    count_type: IntegerType,
) -> [(IntegerValue, IntegerValue); 4] {
    let zero = count_value(count_type, 0);
    let width = count_value(count_type, u128::from(value_type.bits()));
    let width_plus_one = count_value(count_type, u128::from(value_type.bits()) + 1);
    let minus_one_or_maximum = match count_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => count_type.maximum_value(),
    };
    let one = match value_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    [
        (value_type.minimum_value(), zero),
        (value_type.maximum_value(), width),
        (value_type.maximum_value(), width_plus_one),
        (one, minus_one_or_maximum),
    ]
}

fn count_value(count_type: IntegerType, value: u128) -> IntegerValue {
    match count_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value as i128),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    }
}
