use super::super::*;

#[test]
fn optimized_target_lowering_retains_exact_integer_subtract_parameter_custody() {
    let integers = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| {
            [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).expect("native integer"))
        })
        .collect::<Vec<_>>();
    for scalar_type in integers {
        for (target_profile, registers, stack) in boolean_equal_location_cases() {
            for (parameter_count, expected) in [(2, registers), (10, stack)] {
                let left_index = parameter_count - 2;
                let right_index = parameter_count - 1;
                let left_value = ValueId::new(30_100 + left_index as u64).unwrap();
                let right_value = ValueId::new(30_100 + right_index as u64).unwrap();
                let subtract_operation = OperationId::new(30_005).unwrap();
                let obligation = ObligationId::new(30_009).unwrap();
                let (semantic, proof) =
                    exact_integer_subtract_parameters_return_artifact(scalar_type, parameter_count);
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
                )
                .unwrap();
                let target =
                    lower_optimized_to_target_operations(optimized, target_profile).unwrap();
                let receipt = target.translation_validation();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerSubtractParameters(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("optimized exact integer subtract must retain its family row")
                };
                assert_eq!(row.machine(), MachineId::new(30_001).unwrap());
                assert_eq!(row.subtract_operation(), subtract_operation);
                assert_eq!(row.obligation(), obligation);
                assert_eq!(row.return_edge(), EdgeId::new(30_006).unwrap());
                assert_eq!(row.source_value(), ValueId::new(30_003).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left_value(), left_value);
                assert_eq!(row.right_value(), right_value);
                assert_eq!(row.left_parameter_index(), left_index);
                assert_eq!(row.right_parameter_index(), right_index);
                assert_eq!(row.left_location(), expected[0]);
                assert_eq!(row.right_location(), expected[1]);
                assert!(matches!(
                    &target.target_operations().functions[0].operation,
                    TargetOperation::ReturnIntegerExpression {
                        scalar_type: target_type,
                        expression: TargetIntegerExpression::ExactSubtract {
                            psi_operation,
                            obligation: target_obligation,
                            left,
                            right,
                        },
                        ..
                    } if *target_type == scalar_type
                        && *psi_operation == subtract_operation
                        && *target_obligation == obligation
                        && matches!(
                            left.as_ref(),
                            TargetIntegerExpression::Parameter {
                                source_value,
                                parameter_index,
                                location,
                            } if *source_value == left_value
                                && *parameter_index == left_index
                                && *location == expected[0]
                        )
                        && matches!(
                            right.as_ref(),
                            TargetIntegerExpression::Parameter {
                                source_value,
                                parameter_index,
                                location,
                            } if *source_value == right_value
                                && *parameter_index == right_index
                                && *location == expected[1]
                        )
                ));
            }
        }
    }
}
