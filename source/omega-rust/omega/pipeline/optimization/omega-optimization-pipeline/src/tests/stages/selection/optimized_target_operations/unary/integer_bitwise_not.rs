use super::*;

#[test]
fn optimized_target_lowering_retains_exact_integer_bitwise_not_parameter_custody() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) =
                integer_bitwise_not_parameter_return_artifact(scalar_type, parameter_count);
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target = lower_optimized_to_target_operations(optimized, target_profile).unwrap();
            let receipt = target.translation_validation();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotParameter(
                    row,
                ),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized integer bitwise-not must retain its family row")
            };
            assert_eq!(
                row.bitwise_not_operation(),
                OperationId::new(30_005).unwrap()
            );
            assert_eq!(row.scalar_type(), scalar_type);
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                &target.target_operations().functions[0].operation,
                TargetOperation::ReturnIntegerExpression {
                    scalar_type: target_type,
                    expression: TargetIntegerExpression::BitwiseNot { operand, .. },
                    ..
                } if *target_type == scalar_type && matches!(
                    operand.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected_location
                )
            ));
        }
    }
}
