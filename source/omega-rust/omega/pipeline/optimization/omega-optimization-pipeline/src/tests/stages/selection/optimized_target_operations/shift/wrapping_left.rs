use super::*;

#[test]
fn optimized_target_lowering_retains_wrapping_shift_left_parameter_custody() {
    let mut integers = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| {
            [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).expect("native integer"))
        })
        .collect::<Vec<_>>();
    integers.push(IntegerType::address(64).expect("native address integer"));

    for value_type in integers.iter().copied() {
        for count_type in integers.iter().copied() {
            for (target_profile, registers, stack) in boolean_equal_location_cases() {
                for (parameter_count, expected) in [(2, registers), (10, stack)] {
                    let value_index = parameter_count - 2;
                    let count_index = parameter_count - 1;
                    let value_id = ValueId::new(30_100 + value_index as u64).unwrap();
                    let count_id = ValueId::new(30_100 + count_index as u64).unwrap();
                    let (semantic, proof) = wrapping_integer_shift_left_parameters_return_artifact(
                        value_type,
                        count_type,
                        parameter_count,
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
                    let receipt = target.translation_validation();
                    let AbstractToTargetFunctionTranslationDisposition::Validated(
                        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftLeftParameters(row),
                    ) = receipt.function_roster()[0].translation()
                    else {
                        panic!("optimized wrapping shift-left must retain its family row")
                    };
                    assert_eq!(row.machine(), MachineId::new(30_001).unwrap());
                    assert_eq!(row.shift_operation(), OperationId::new(30_005).unwrap());
                    assert_eq!(row.return_edge(), EdgeId::new(30_006).unwrap());
                    assert_eq!(row.source_value(), ValueId::new(30_003).unwrap());
                    assert_eq!(row.value_type(), value_type);
                    assert_eq!(row.count_type(), count_type);
                    assert_eq!(row.value(), value_id);
                    assert_eq!(row.count(), count_id);
                    assert_eq!(row.value_parameter_index(), value_index);
                    assert_eq!(row.count_parameter_index(), count_index);
                    assert_eq!(row.value_location(), expected[0]);
                    assert_eq!(row.count_location(), expected[1]);
                    assert!(matches!(
                        &target.target_operations().functions[0].operation,
                        TargetOperation::ReturnIntegerExpression {
                            scalar_type: result_type,
                            expression: TargetIntegerExpression::WrappingShiftLeft {
                                psi_operation,
                                count_type: target_count_type,
                                value,
                                count,
                            },
                            ..
                        } if *result_type == value_type
                            && *psi_operation == OperationId::new(30_005).unwrap()
                            && *target_count_type == count_type
                            && matches!(
                                value.as_ref(),
                                TargetIntegerExpression::Parameter {
                                    source_value,
                                    parameter_index,
                                    location,
                                } if *source_value == value_id
                                    && *parameter_index == value_index
                                    && *location == expected[0]
                            )
                            && matches!(
                                count.as_ref(),
                                TargetIntegerExpression::Parameter {
                                    source_value,
                                    parameter_index,
                                    location,
                                } if *source_value == count_id
                                    && *parameter_index == count_index
                                    && *location == expected[1]
                            )
                    ));
                }
            }
        }
    }
}
