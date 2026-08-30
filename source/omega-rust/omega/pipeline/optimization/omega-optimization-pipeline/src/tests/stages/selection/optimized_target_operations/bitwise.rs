use super::*;

#[test]
fn optimized_target_lowering_retains_exact_integer_bitwise_and_parameter_custody() {
    let integers = [IntegerSign::Signed, IntegerSign::Unsigned]
        .into_iter()
        .flat_map(|sign| {
            [8, 16, 32, 64].map(|bits| IntegerType::new(sign, bits).expect("native integer"))
        })
        .collect::<Vec<_>>();
    for scalar_type in integers {
        for (target_profile, registers, stack) in boolean_equal_location_cases() {
            for (parameter_count, expected) in [(2, registers), (10, stack)] {
                let (semantic, proof) =
                    integer_bitwise_and_parameters_return_artifact(scalar_type, parameter_count);
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
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseAndParameters(row),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("optimized integer bitwise-AND must retain its family row")
                };
                assert_eq!(row.machine(), MachineId::new(30_001).unwrap());
                assert_eq!(row.and_operation(), OperationId::new(30_005).unwrap());
                assert_eq!(row.return_edge(), EdgeId::new(30_006).unwrap());
                assert_eq!(row.source_value(), ValueId::new(30_003).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left_parameter_index(), parameter_count - 2);
                assert_eq!(row.right_parameter_index(), parameter_count - 1);
                assert_eq!(row.left_location(), expected[0]);
                assert_eq!(row.right_location(), expected[1]);
                assert!(matches!(
                    &target.target_operations().functions[0].operation,
                    TargetOperation::ReturnIntegerExpression {
                        scalar_type: target_type,
                        expression: TargetIntegerExpression::BitwiseAnd { left, right, .. },
                        ..
                    } if *target_type == scalar_type
                        && matches!(
                            left.as_ref(),
                            TargetIntegerExpression::Parameter { location, .. }
                                if *location == expected[0]
                        )
                        && matches!(
                            right.as_ref(),
                            TargetIntegerExpression::Parameter { location, .. }
                                if *location == expected[1]
                        )
                ));
            }
        }
    }
}
