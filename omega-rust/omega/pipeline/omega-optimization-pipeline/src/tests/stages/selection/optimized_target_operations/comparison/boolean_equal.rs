use super::*;

#[test]
fn optimized_target_lowering_retains_exact_boolean_equality_parameter_custody() {
    for (target_profile, registers, stack) in boolean_equal_location_cases() {
        for (parameter_count, expected) in [(2, registers), (10, stack)] {
            let (semantic, proof) = boolean_equal_parameters_return_artifact(parameter_count);
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
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanEqualParameters(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized Boolean equality must retain its family row")
            };
            assert_eq!(row.equal_operation(), OperationId::new(30_005).unwrap());
            assert_eq!(row.left_parameter_index(), parameter_count - 2);
            assert_eq!(row.right_parameter_index(), parameter_count - 1);
            assert_eq!(row.left_location(), expected[0]);
            assert_eq!(row.right_location(), expected[1]);
            assert!(matches!(
                &target.target_operations().functions[0].operation,
                TargetOperation::ReturnBooleanExpression {
                    expression: TargetBooleanExpression::Equal { left, right, .. },
                    ..
                } if matches!(
                    left.as_ref(),
                    TargetBooleanExpression::Parameter { location, .. }
                        if *location == expected[0]
                ) && matches!(
                    right.as_ref(),
                    TargetBooleanExpression::Parameter { location, .. }
                        if *location == expected[1]
                )
            ));
        }
    }
}
