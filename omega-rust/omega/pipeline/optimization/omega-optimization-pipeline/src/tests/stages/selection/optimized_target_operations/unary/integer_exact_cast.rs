use super::*;

#[test]
fn optimized_target_lowering_retains_proof_bearing_integer_exact_cast_custody() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    for (target_profile, register, stack) in parameter_location_cases() {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let (semantic, proof) = integer_exact_cast_parameter_return_artifact(
                source_type,
                target_type,
                parameter_count,
            );
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
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerExactCastParameter(
                    row,
                ),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("optimized integer exact cast must retain its family row")
            };
            assert_eq!(row.cast_operation(), OperationId::new(30_005).unwrap());
            assert_eq!(row.obligation(), ObligationId::new(30_009).unwrap());
            assert_eq!(row.source_type(), source_type);
            assert_eq!(row.target_type(), target_type);
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                &target.target_operations().functions[0].operation,
                TargetOperation::ReturnIntegerExpression {
                    scalar_type,
                    expression: TargetIntegerExpression::IntegerExactCast {
                        obligation,
                        source_type: target_source_type,
                        operand,
                        ..
                    },
                    ..
                } if *scalar_type == target_type
                    && *obligation == ObligationId::new(30_009).unwrap()
                    && *target_source_type == source_type
                    && matches!(
                        operand.as_ref(),
                        TargetIntegerExpression::Parameter { location, .. }
                            if *location == expected_location
                    )
            ));
        }
    }
}
