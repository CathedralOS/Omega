use super::*;

#[test]
fn optimized_target_lowering_retains_exact_integer_less_or_equal_parameter_custody() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    for (target_profile, registers, stack) in boolean_equal_location_cases() {
        for (parameter_count, expected) in [(2, registers), (10, stack)] {
            let (semantic, proof) =
                integer_less_or_equal_parameters_return_artifact(scalar_type, parameter_count);
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
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLessOrEqualParameters(
                    row,
                ),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized integer less-or-equal must retain its family row")
            };
            assert_eq!(
                row.less_or_equal_operation(),
                OperationId::new(30_005).unwrap()
            );
            assert_eq!(row.scalar_type(), scalar_type);
            assert_eq!(row.left_parameter_index(), parameter_count - 2);
            assert_eq!(row.right_parameter_index(), parameter_count - 1);
            assert_eq!(row.left_location(), expected[0]);
            assert_eq!(row.right_location(), expected[1]);
            assert!(matches!(
                &target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanExpression {
                    expression: TargetBooleanExpression::IntegerLessOrEqual {
                        scalar_type: target_type,
                        left,
                        right,
                        ..
                    },
                    ..
                } if *target_type == scalar_type && matches!(
                    left.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected[0]
                ) && matches!(
                    right.as_ref(),
                    TargetIntegerExpression::Parameter { location, .. }
                        if *location == expected[1]
                )
            ));
        }
    }
}
